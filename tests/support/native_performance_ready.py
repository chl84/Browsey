"""Bounded AT-SPI observation of one exact disposable candidate PID."""
import json
import subprocess
import sys
import time
import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi

pid, executable = int(sys.argv[1]), sys.argv[2]
deadline = time.monotonic() + 30
while time.monotonic() < deadline:
    actual = subprocess.check_output(["readlink", "-f", f"/proc/{pid}/exe"], text=True).strip()
    assert actual == executable, "Refuse to observe a different or reused process"
    desktop = Atspi.get_desktop(0)
    app = next((desktop.get_child_at_index(i) for i in range(desktop.get_child_count())
                if desktop.get_child_at_index(i).get_process_id() == pid), None)
    if app is not None:
        stack, count = [(app, 0)], 0
        while stack and count < 2000:
            node, depth = stack.pop()
            if node is None or depth > 35:
                continue
            count += 1
            if node.get_role_name() == "button" and node.get_name().startswith("item-000000-needle"):
                print(json.dumps({"ready": True, "observedNodes": count,
                                  "scope": "accessible first file row, not a paint or thumbnail assertion"}))
                sys.exit(0)
            stack.extend((node.get_child_at_index(i), depth + 1) for i in range(node.get_child_count()))
    time.sleep(0.05)
raise RuntimeError("Candidate's expected fixture row did not become accessible")
