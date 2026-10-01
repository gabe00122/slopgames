#!/usr/bin/env python3
"""Virtual gamepads over /dev/uinput, for testing controller support without
hardware (Linux; needs write access to /dev/uinput). Start the game, then:

    python3 tools/vpad.py tap:0:a wait:0.5 tap:0:a axis:0:rt:1 wait:3

Usage: vpad.py SCRIPT...   where each step is one of
  wait:SECS
  tap:PAD:BUTTON          (press and release)
  down:PAD:BUTTON / up:PAD:BUTTON
  axis:PAD:AXIS:VALUE     (AXIS in lx ly rt lt, VALUE -1..1 or 0..1)
PAD is 0-based; pads are created on first use.
"""
import fcntl, os, struct, sys, time

EV_SYN, EV_KEY, EV_ABS = 0, 1, 3
BTN = dict(a=0x130, b=0x131, x=0x133, y=0x134, lb=0x136, rb=0x137, select=0x13a, start=0x13b, mode=0x13c, ls=0x13d, rs=0x13e)
ABS = dict(lx=0, ly=1, lt=2, rx=3, ry=4, rt=5, hx=16, hy=17)
UI_SET_EVBIT, UI_SET_KEYBIT, UI_SET_ABSBIT = 0x40045564, 0x40045565, 0x40045567
UI_DEV_CREATE, UI_DEV_DESTROY, UI_DEV_SETUP, UI_ABS_SETUP = 0x5501, 0x5502, 0x405c5503, 0x401c5504

class Pad:
    def __init__(self, n):
        self.fd = os.open('/dev/uinput', os.O_WRONLY | os.O_NONBLOCK)
        for ev in (EV_KEY, EV_ABS):
            fcntl.ioctl(self.fd, UI_SET_EVBIT, ev)
        for code in BTN.values():
            fcntl.ioctl(self.fd, UI_SET_KEYBIT, code)
        for name, code in ABS.items():
            fcntl.ioctl(self.fd, UI_SET_ABSBIT, code)
            lo, hi = (0, 255) if name in ('lt', 'rt') else ((-1, 1) if name in ('hx', 'hy') else (-32768, 32767))
            flat = 0 if name in ('lt', 'rt', 'hx', 'hy') else 128
            fcntl.ioctl(self.fd, UI_ABS_SETUP, struct.pack('HHiiiiii', code, 0, 0, lo, hi, 16 if flat else 0, flat, 0))
        name = f'Microsoft X-Box 360 pad {n}'.encode()
        fcntl.ioctl(self.fd, UI_DEV_SETUP, struct.pack('HHHH80sI', 3, 0x045e, 0x028e, 0x110, name, 0))
        fcntl.ioctl(self.fd, UI_DEV_CREATE)
    def emit(self, etype, code, value):
        now = time.time()
        os.write(self.fd, struct.pack('llHHi', int(now), int((now % 1) * 1e6), etype, code, value))
        os.write(self.fd, struct.pack('llHHi', int(now), int((now % 1) * 1e6), EV_SYN, 0, 0))
    def close(self):
        fcntl.ioctl(self.fd, UI_DEV_DESTROY)
        os.close(self.fd)

pads = {}
def pad(i):
    if i not in pads:
        pads[i] = Pad(i)
        time.sleep(1.0)
    return pads[i]

for step in sys.argv[1:]:
    parts = step.split(':')
    op = parts[0]
    if op == 'wait':
        time.sleep(float(parts[1]))
    elif op == 'tap':
        p = pad(int(parts[1])); p.emit(EV_KEY, BTN[parts[2]], 1); time.sleep(0.08); p.emit(EV_KEY, BTN[parts[2]], 0); time.sleep(0.12)
    elif op in ('down', 'up'):
        pad(int(parts[1])).emit(EV_KEY, BTN[parts[2]], 1 if op == 'down' else 0)
    elif op == 'axis':
        name, v = parts[2], float(parts[3])
        value = int(v * 255) if name in ('lt', 'rt') else (int(v) if name in ('hx', 'hy') else int(v * 32767))
        pad(int(parts[1])).emit(EV_ABS, ABS[name], value)
    print(step, flush=True)
time.sleep(0.3)
for p in pads.values():
    p.close()
