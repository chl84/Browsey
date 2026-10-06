"""Real input on the dedicated authenticated virtual display only."""
import ctypes as C
import json
import os
import sys
import time

assert os.environ.get('DISPLAY') == ':91'
assert 'WAYLAND_DISPLAY' not in os.environ
assert os.environ.get('BROWSEY_NATIVE_ISOLATED'), 'No isolated desktop identity'
assert '/ai_agent_testfolder/.bnt-' in os.environ['XAUTHORITY']
command = sys.argv[1]
payload = json.loads(sys.argv[2])
x = C.CDLL('libX11.so.6')
t = C.CDLL('libXtst.so.6')
x.XOpenDisplay.restype = C.c_void_p
x.XDefaultRootWindow.argtypes = [C.c_void_p]
x.XDefaultRootWindow.restype = C.c_ulong
x.XInternAtom.argtypes = [C.c_void_p, C.c_char_p, C.c_int]
x.XInternAtom.restype = C.c_ulong
x.XGetWindowProperty.argtypes = [C.c_void_p,C.c_ulong,C.c_ulong,C.c_long,C.c_long,C.c_int,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_int),C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ubyte))]
x.XQueryTree.argtypes = [C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ulong)),C.POINTER(C.c_uint)]
x.XGetGeometry.argtypes = [C.c_void_p,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.POINTER(C.c_uint),C.POINTER(C.c_uint),C.POINTER(C.c_uint)]
x.XTranslateCoordinates.argtypes = [C.c_void_p,C.c_ulong,C.c_ulong,C.c_int,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_ulong)]
x.XMoveResizeWindow.argtypes = [C.c_void_p,C.c_ulong,C.c_int,C.c_int,C.c_uint,C.c_uint]
x.XSetInputFocus.argtypes = [C.c_void_p,C.c_ulong,C.c_int,C.c_ulong]
x.XKeysymToKeycode.argtypes = [C.c_void_p,C.c_ulong];x.XKeysymToKeycode.restype = C.c_uint
x.XFree.argtypes = [C.c_void_p]
x.XFlush.argtypes = [C.c_void_p];x.XSync.argtypes = [C.c_void_p,C.c_int];x.XCloseDisplay.argtypes = [C.c_void_p]
t.XTestFakeMotionEvent.argtypes = [C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_ulong]
t.XTestFakeButtonEvent.argtypes = [C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
t.XTestFakeKeyEvent.argtypes = [C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
d = x.XOpenDisplay(None)
assert d, 'Private authenticated display unavailable'
root = x.XDefaultRootWindow(d)

def windows():
    parent, r = C.c_ulong(),C.c_ulong();children=C.POINTER(C.c_ulong)();n=C.c_uint()
    assert x.XQueryTree(d,root,C.byref(r),C.byref(parent),C.byref(children),C.byref(n))
    result=[children[i] for i in range(n.value)];x.XFree(children);return result

def pid(window):
    actual=C.c_ulong();fmt=C.c_int();n=C.c_ulong();remaining=C.c_ulong();data=C.POINTER(C.c_ubyte)()
    x.XGetWindowProperty(d,window,x.XInternAtom(d,b'_NET_WM_PID',False),0,1,False,6,C.byref(actual),C.byref(fmt),C.byref(n),C.byref(remaining),C.byref(data))
    value=C.cast(data,C.POINTER(C.c_ulong))[0] if n.value else None
    if data:x.XFree(data)
    return value

def geometry(window):
    r=C.c_ulong();px,py=C.c_int(),C.c_int();w,h,b,depth=[C.c_uint() for _ in range(4)]
    assert x.XGetGeometry(d,window,C.byref(r),C.byref(px),C.byref(py),C.byref(w),C.byref(h),C.byref(b),C.byref(depth))
    child=C.c_ulong();assert x.XTranslateCoordinates(d,window,root,0,0,C.byref(px),C.byref(py),C.byref(child))
    return {'id':window,'origin':[px.value,py.value],'size':[w.value,h.value]}

try:
    if command in ('window','place','wheel'):
        p=payload['pid'];assert isinstance(p,int) and p>0
        assert os.path.realpath(f'/proc/{p}/exe') == payload['executable']
        with open(f'/proc/{p}/environ','rb') as f:assert ('XDG_DATA_HOME='+payload['data']).encode() in f.read().split(b'\0')
        found=[w for w in windows() if pid(w)==p and min(geometry(w)['size'])>=300]
        assert len(found)==1, 'Expected exactly one owned main window'
        window=found[0]
        if command=='place':
            px,py,w,h=payload['bounds'];assert 0<=px<2000 and 0<=py<1100 and 300<=w<=2000 and 300<=h<=1100
            x.XMoveResizeWindow(d,window,px,py,w,h);x.XSetInputFocus(d,window,1,0);x.XSync(d,False);time.sleep(.15)
        elif command=='wheel':
            g=geometry(window);px,py=payload['point'];direction=payload['direction']
            assert isinstance(px,int) and isinstance(py,int) and direction in (-1,1)
            assert g['origin'][0]<=px<g['origin'][0]+g['size'][0] and g['origin'][1]<=py<g['origin'][1]+g['size'][1]
            key=x.XKeysymToKeycode(d,0xffe3)
            try:
                x.XSetInputFocus(d,window,1,0);t.XTestFakeMotionEvent(d,-1,px,py,0)
                t.XTestFakeKeyEvent(d,key,1,0);x.XFlush(d);time.sleep(.05)
                button=4 if direction>0 else 5
                t.XTestFakeButtonEvent(d,button,1,0);t.XTestFakeButtonEvent(d,button,0,0);x.XFlush(d);time.sleep(.2)
            finally:
                t.XTestFakeKeyEvent(d,key,0,0);x.XFlush(d)
        print(json.dumps(geometry(window)))
    elif command in ('drag','hold','release'):
        points=payload.get('points',[]);assert len(points)<=128
        assert all(len(p)==2 and all(isinstance(v,int) for v in p) and 0<=p[0]<2000 and 0<=p[1]<1100 for p in points)
        mode=payload.get('mode','default');assert mode in ('default','copy','move')
        modifier=x.XKeysymToKeycode(d,{'copy':0xffe3,'move':0xffe1}.get(mode,0)) if mode!='default' else 0
        try:
            if command!='release':
                assert len(points)>=2
                t.XTestFakeMotionEvent(d,-1,*points[0],0)
                if modifier and not payload.get('modifier_after_press',False):t.XTestFakeKeyEvent(d,modifier,1,0)
                x.XFlush(d);time.sleep(.1);t.XTestFakeButtonEvent(d,1,1,0);x.XFlush(d);time.sleep(.1)
                # Preserve the already selected fixture batch; modifiers must be
                # active for dragstart, after the ordinary selection press.
                if modifier and payload.get('modifier_after_press',False):t.XTestFakeKeyEvent(d,modifier,1,0)
                x.XFlush(d);time.sleep(.05)
                for point in points[1:]:
                    t.XTestFakeMotionEvent(d,-1,*point,0);x.XFlush(d);time.sleep(.04)
                time.sleep(.15)
                if payload.get('cancel'):
                    key=x.XKeysymToKeycode(d,0xff1b);t.XTestFakeKeyEvent(d,key,1,0);t.XTestFakeKeyEvent(d,key,0,0);x.XFlush(d);time.sleep(.1)
        finally:
            if command!='hold':
                t.XTestFakeButtonEvent(d,1,0,0);x.XFlush(d);time.sleep(.25)
            if modifier:t.XTestFakeKeyEvent(d,modifier,0,0)
            x.XFlush(d)
        print(json.dumps({'nativeInput':True,'mode':mode,'points':len(points),'held':command=='hold'}))
    else:raise AssertionError('Unknown isolated input action')
finally:x.XCloseDisplay(d)
