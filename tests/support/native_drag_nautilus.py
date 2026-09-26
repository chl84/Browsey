"""Opt-in native acceptance test against an isolated Nautilus process.

Arguments: source X/Y, fresh temporary fixture root, default|copy|move.
Creates only its own fixtures. Never connects to an existing Nautilus instance.
"""
import ctypes
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

sx, sy = sys.argv[1:3]
root = Path(sys.argv[3])
mode = sys.argv[4]
backend = os.environ.get('BROWSEY_TEST_NAUTILUS_BACKEND', 'x11')
bus = os.environ.get('BROWSEY_TEST_NAUTILUS_BUS', 'private')
assert bus in ('private', 'session')
# Shared-bus coverage exercises GTK's real portal negotiation. Never forward
# the fixture window to a user's already running Nautilus instance.
if bus == 'session':
    owner = subprocess.check_output([
        'gdbus', 'call', '--session', '--dest', 'org.freedesktop.DBus',
        '--object-path', '/org/freedesktop/DBus', '--method',
        'org.freedesktop.DBus.NameHasOwner', 'org.gnome.Nautilus',
    ], text=True)
    assert 'false' in owner, 'Close Nautilus before running the shared-session test'
assert backend in ('x11', 'wayland')
assert mode in ('default', 'copy', 'move')
assert root.parent == Path('/tmp') and root.name.startswith('browsey-native-acceptance-')
root.mkdir()  # Refuse to reuse or remove someone else's existing directory.
dest = root / ('destination-' + root.name.removeprefix('browsey-native-acceptance-'))
dest.mkdir()
runtime = root / 'runtime'
runtime.mkdir(mode=0o700)
names = ['first photo.txt', 'æ #?%+ second.txt']
for name in names:
    (root / name).write_text('Browsey disposable drag fixture: ' + name, encoding='utf-8')
process = None
x11 = ctypes.CDLL('libX11.so.6')
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
x11.XDefaultRootWindow.restype = ctypes.c_ulong
x11.XGetGeometry.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_uint), ctypes.POINTER(ctypes.c_uint), ctypes.POINTER(ctypes.c_uint), ctypes.POINTER(ctypes.c_uint)]
x11.XTranslateCoordinates.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_ulong)]
x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
display = x11.XOpenDisplay(None)
try:
    env = dict(os.environ, GDK_BACKEND=backend, GTK_A11Y='none', NO_AT_BRIDGE='1', XDG_RUNTIME_DIR=str(runtime), XDG_DATA_HOME=str(root / 'data'), XDG_CONFIG_HOME=str(root / 'config'), XDG_CACHE_HOME=str(root / 'cache'))
    # A private D-Bus session must not reuse the real session's AT-SPI socket.
    # Keep compositor access via its absolute socket while isolating runtime data.
    wayland_display = os.environ.get('WAYLAND_DISPLAY', '')
    if wayland_display and not os.path.isabs(wayland_display):
        env['WAYLAND_DISPLAY'] = str(Path(os.environ['XDG_RUNTIME_DIR']) / wayland_display)
    env.pop('AT_SPI_BUS_ADDRESS', None)
    command = ['nautilus', '--new-window', str(dest)]
    if bus == 'private':
        command = ['dbus-run-session', '--', *command]
    process = subprocess.Popen(command, env=env, start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    deadline = time.monotonic() + 12
    window = None
    source = None
    while time.monotonic() < deadline and (window is None or source is None):
        if backend == 'wayland':
            clients = json.loads(subprocess.check_output(['hyprctl', '-j', 'clients']))
            window = next((client for client in clients if dest.name in client['title']), None)
            source = next((client for client in clients if 'Browsey drag source test' in client['title']), None)
            time.sleep(0.15)
            continue
        windows = subprocess.check_output(['xprop', '-root', '_NET_CLIENT_LIST'], text=True)
        for candidate in re.findall(r'0x[0-9a-fA-F]+', windows):
            title = subprocess.run(['xprop', '-id', candidate, '_NET_WM_NAME'], capture_output=True, text=True).stdout
            if dest.name in title:
                window = int(candidate, 16)
            if 'Browsey drag source test' in title:
                source = int(candidate, 16)
        time.sleep(0.15)
    assert window and source, 'native test windows did not appear'
    time.sleep(1)
    root_window = ctypes.c_ulong()
    x, y = ctypes.c_int(), ctypes.c_int()
    width, height, border, depth = (ctypes.c_uint() for _ in range(4))
    if backend == 'wayland':
        # Re-query after window placement/animation has settled.
        clients = json.loads(subprocess.check_output(['hyprctl', '-j', 'clients']))
        window = next(client for client in clients if dest.name in client['title'])
        source = next(client for client in clients if 'Browsey drag source test' in client['title'])
        assert not window['xwayland'], 'Nautilus did not start with the Wayland backend'
        sx, sy = source['at'][0] + 100, source['at'][1] + 100
        dx, dy = window['at'][0] + int(window['size'][0] * 0.75), window['at'][1] + int(window['size'][1] * 0.65)
        pointer_script = 'native_drag_wayland_pointer.py'
    else:
        assert x11.XGetGeometry(display, window, ctypes.byref(root_window), ctypes.byref(x), ctypes.byref(y), ctypes.byref(width), ctypes.byref(height), ctypes.byref(border), ctypes.byref(depth))
        child = ctypes.c_ulong()
        assert x11.XTranslateCoordinates(display, window, x11.XDefaultRootWindow(display), 0, 0, ctypes.byref(x), ctypes.byref(y), ctypes.byref(child))
        dx, dy = x.value + int(width.value * 0.75), y.value + int(height.value * 0.65)
        assert x11.XTranslateCoordinates(display, source, x11.XDefaultRootWindow(display), 0, 0, ctypes.byref(x), ctypes.byref(y), ctypes.byref(child))
        sx, sy = x.value + 100, y.value + 100
        pointer_script = 'native_drag_pointer.py'
    print(f'NATIVE TEST gesture: {backend} {mode} from {sx},{sy} to {dx},{dy}', flush=True)
    # The Rust fixture advertises the explicit action; modifier mapping is
    # independently tested in the frontend. Do not synthesize keyboard state.
    subprocess.run([sys.executable, str(Path(__file__).with_name(pointer_script)), str(sx), str(sy), str(dx), str(dy)], check=True, timeout=12)
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline and (
        not all((dest / name).exists() for name in names)
        or (mode == 'move' and any((root / name).exists() for name in names))
    ):
        time.sleep(0.1)
    for name in names:
        assert (dest / name).read_text(encoding='utf-8') == 'Browsey disposable drag fixture: ' + name
        if mode == 'copy':
            assert (root / name).exists(), 'copy deleted a source'
        elif mode == 'move':
            assert not (root / name).exists(), 'move left the source behind'
    action = 'copy' if (root / names[0]).exists() else 'move'
    print(f'NAUTILUS PASS: {mode} -> {action}; two filenames and contents verified', flush=True)
finally:
    if process is not None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
            process.wait(timeout=3)
        except ProcessLookupError:
            pass
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
    x11.XCloseDisplay(display)
    shutil.rmtree(root)
