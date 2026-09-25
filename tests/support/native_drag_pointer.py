"""Drive only the supplied test-window coordinates through X11/XWayland.

Used by the opt-in GTK integration test, never by Browsey itself. Always release
buttons/modifiers, including on exceptions; no files or desktop config are edited.
"""
import ctypes
import sys
import time

x11 = ctypes.CDLL('libX11.so.6')
xtst = ctypes.CDLL('libXtst.so.6')
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XFlush.argtypes = [ctypes.c_void_p]
x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
x11.XDefaultRootWindow.restype = ctypes.c_ulong
x11.XQueryPointer.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_uint)]
x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
x11.XKeysymToKeycode.restype = ctypes.c_uint
xtst.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong]
xtst.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
xtst.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
display = x11.XOpenDisplay(None)
if not display:
    raise RuntimeError('X11 display unavailable')
sx, sy, dx, dy = map(int, sys.argv[1:5])
mode = sys.argv[5] if len(sys.argv) > 5 else 'default'
modifier = {'copy': 0xffe3, 'move': 0xffe1}.get(mode)
keycode = x11.XKeysymToKeycode(display, modifier) if modifier else 0

def motion(x, y):
    xtst.XTestFakeMotionEvent(display, -1, round(x), round(y), 0)
    x11.XFlush(display)

try:
    motion(sx, sy)
    time.sleep(0.15)
    xtst.XTestFakeButtonEvent(display, 1, 1, 0)
    x11.XFlush(display)
    time.sleep(0.15)
    for step in range(1, 31):
        if step == 5 and keycode:
            xtst.XTestFakeKeyEvent(display, keycode, 1, 0)
        motion(sx + (dx - sx) * step / 30, sy + (dy - sy) * step / 30)
        time.sleep(0.04)
    time.sleep(0.4)
    root, child = ctypes.c_ulong(), ctypes.c_ulong()
    rx, ry, wx, wy = (ctypes.c_int() for _ in range(4))
    state = ctypes.c_uint()
    x11.XQueryPointer(display, x11.XDefaultRootWindow(display), ctypes.byref(root), ctypes.byref(child), ctypes.byref(rx), ctypes.byref(ry), ctypes.byref(wx), ctypes.byref(wy), ctypes.byref(state))
    print(f'Pointer at drop: {rx.value},{ry.value}; keycode={keycode}, state={state.value:#x}', flush=True)
finally:
    xtst.XTestFakeButtonEvent(display, 1, 0, 0)
    x11.XFlush(display)
    # Let GTK negotiate/drop while the requested modifier is still physically
    # down; releasing both in one X11 batch can turn a Shift-drop into a copy.
    time.sleep(0.5)
    if keycode:
        xtst.XTestFakeKeyEvent(display, keycode, 0, 0)
    x11.XFlush(display)
    x11.XCloseDisplay(display)
