"""AT-SPI actions/input restricted to one executable/PID/private fixture profile."""
import json
import os
import subprocess
import sys
import time
import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi

pid, executable, data = int(sys.argv[1]), sys.argv[2], sys.argv[3]
command, role, name = sys.argv[4:7]

def validate():
    assert os.path.realpath(f"/proc/{pid}/exe") == executable, "Wrong/reused candidate PID"
    with open(f"/proc/{pid}/environ", "rb") as source:
        assert f"XDG_DATA_HOME={data}".encode() in source.read().split(b"\0"), "Not our private profile"

def targeted_key(mods, key):
    validate()
    clients = json.loads(subprocess.check_output(["hyprctl", "clients", "-j"], text=True))
    client = next(client for client in clients if client["pid"] == pid)
    address = client["address"]
    assert address.startswith("0x") and all(c in "0123456789abcdef" for c in address[2:])
    validate()
    code = f"hl.dsp.send_shortcut({{ mods = {json.dumps(mods)}, key = {json.dumps(key)}, window = {json.dumps('address:' + address)} }})"
    assert subprocess.check_output(["hyprctl", "dispatch", code], text=True, timeout=3).strip() == "ok"

validate()
if command == "key":
    keys = {"enter": ("", "Return"), "context": ("SHIFT", "F10"), "escape": ("", "Escape")}
    targeted_key(*keys[name])
    print(json.dumps({"action": command, "pid": pid, "key": name}))
    sys.exit(0)

deadline = time.monotonic() + min(30, max(1, int(os.environ.get("BROWSEY_TEST_A11Y_TIMEOUT", "30"))))
while time.monotonic() < deadline:
    validate()
    desktop = Atspi.get_desktop(0)
    app = next((desktop.get_child_at_index(i) for i in range(desktop.get_child_count())
                if desktop.get_child_at_index(i).get_process_id() == pid), None)
    nodes, stack = [], [app] if app is not None else []
    while stack and len(nodes) < 2000:
        node = stack.pop()
        if node is None or node in nodes:
            continue
        nodes.append(node)
        stack.extend(node.get_child_at_index(i) for i in range(node.get_child_count()))
    if command == "snapshot":
        print(json.dumps([(node.get_role_name(), node.get_name()) for node in nodes if node.get_name()]))
        sys.exit(0)
    matches = [node for node in nodes if node.get_role_name() in role.split("|") and node.get_name().startswith(name)]
    if len(matches) == 1:
        node = matches[0]
        validate()
        if command in ("click", "focus", "set"):
            assert node.get_component_iface().grab_focus()
        if command == "click":
            action = node.get_action_iface()
            assert action is not None and action.get_n_actions() > 0 and action.do_action(0)
        elif command == "set":
            value = sys.argv[7]
            editable = node.get_editable_text_iface()
            if editable is not None:
                assert editable.set_text_contents(value)
            else:
                # WebKit may expose text without an EditableText interface.
                # Never use global clipboard/input; only fixture ASCII values
                # delivered to the exact candidate window are supported here.
                assert 0 < len(value) <= 64 and all(c in "abcdefghijklmnopqrstuvwxyz0123456789" for c in value)
                targeted_key("CTRL", "a")
                for character in value:
                    targeted_key("", character)
                    time.sleep(0.02)
                if node.get_role_name() != "password text" and node.get_text_iface() is not None:
                    assert Atspi.Text.get_text(node, 0, -1) == value
        else:
            assert command in ("wait", "focus")
        print(json.dumps({"action": command, "pid": pid, "role": role, "namePrefix": name}))
        sys.exit(0)
    time.sleep(0.1)
raise RuntimeError(f"Missing/ambiguous owned accessible target: {role}, {name}")
