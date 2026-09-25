"""Opt-in Hyprland native-input driver for disposable drag test windows.

Requires an already writable /dev/uinput. Creates and destroys its own virtual
input device; never changes permissions, compositor configuration or bindings.
"""
import fcntl
import json
import os
import struct
import subprocess
import sys
import time

sx, sy, dx, dy = map(int, sys.argv[1:5])
device = os.open('/dev/uinput', os.O_WRONLY | os.O_NONBLOCK)
created = False

def event(kind, code, value):
    os.write(device, struct.pack('llHHi', 0, 0, kind, code, value))
    os.write(device, struct.pack('llHHi', 0, 0, 0, 0, 0))

def motion(x, y):
    # Feedback compensates for pointer acceleration without changing settings.
    for _ in range(24):
        current = json.loads(subprocess.check_output(['hyprctl', '-j', 'cursorpos']))
        rx, ry = round(x - current['x']), round(y - current['y'])
        if abs(rx) <= 8 and abs(ry) <= 8:
            return
        event(2, 0, round(rx / 2) or (1 if rx > 0 else -1) if rx else 0)
        event(2, 1, round(ry / 2) or (1 if ry > 0 else -1) if ry else 0)
        time.sleep(0.015)
    raise RuntimeError(f'virtual pointer could not reach {x},{y}; last position {current}')

try:
    for kind in (1, 2):  # EV_KEY, EV_REL
        fcntl.ioctl(device, 0x40045564, kind)
    for button in (272, 273, 274):  # Conventional three-button mouse, no keyboard.
        fcntl.ioctl(device, 0x40045565, button)
    for axis in (0, 1):
        fcntl.ioctl(device, 0x40045566, axis)
    setup = struct.pack('HHHH80sI', 3, 1, 1, 1, b'Browsey disposable DnD test', 0)
    fcntl.ioctl(device, 0x405c5503, setup)
    fcntl.ioctl(device, 0x5501)
    created = True
    time.sleep(0.8)
    motion(sx, sy)
    time.sleep(0.2)
    event(1, 272, 1)
    time.sleep(0.2)
    for step in range(1, 31):
        motion(sx + (dx - sx) * step / 30, sy + (dy - sy) * step / 30)
        time.sleep(0.04)
    time.sleep(0.4)
finally:
    if created:
        event(1, 272, 0)
        time.sleep(0.5)
        fcntl.ioctl(device, 0x5502)
    os.close(device)
