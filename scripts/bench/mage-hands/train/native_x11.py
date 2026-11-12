"""X11 capture/input for an explicitly isolated Apollo dataset display.
No desktop discovery, models, services, or account access. Requires DISPLAY and
XAUTHORITY supplied by the run owner; refuses conventional desktop displays.
"""
import argparse
import ctypes as C
import json
import os
from pathlib import Path
import time


def connect():
    display = os.environ.get('DISPLAY', '')
    if not display.startswith(':') or not display[1:].isdigit() or int(display[1:]) < 150:
        raise ValueError('Only explicit private dataset displays >= :150 allowed')
    auth = Path(os.environ.get('XAUTHORITY', '/nonexistent'))
    if not auth.is_file() or 'mage-native' not in str(auth):
        raise ValueError('A private mage-native Xauthority is required')
    x = C.CDLL('libX11.so.6')
    t = C.CDLL('libXtst.so.6')
    x.XOpenDisplay.argtypes = [C.c_char_p]; x.XOpenDisplay.restype = C.c_void_p
    d = x.XOpenDisplay(display.encode())
    if not d: raise RuntimeError('Cannot open private X display')
    x.XCloseDisplay.argtypes = [C.c_void_p]
    x.XFlush.argtypes = [C.c_void_p]
    x.XSync.argtypes = [C.c_void_p, C.c_int]
    x.XStringToKeysym.argtypes = [C.c_char_p]; x.XStringToKeysym.restype = C.c_ulong
    x.XKeysymToKeycode.argtypes = [C.c_void_p, C.c_ulong]; x.XKeysymToKeycode.restype = C.c_uint
    t.XTestFakeMotionEvent.argtypes = [C.c_void_p,C.c_int,C.c_int,C.c_int,C.c_ulong]
    t.XTestFakeButtonEvent.argtypes = [C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
    t.XTestFakeKeyEvent.argtypes = [C.c_void_p,C.c_uint,C.c_int,C.c_ulong]
    return x,t,d


def act(action, **kwargs):
    if action == 'shot':
        from PIL import ImageGrab
        x,t,d=connect()
        try:
            x.XSync(d,False)
            path=Path(kwargs['path'])
            path.parent.mkdir(parents=True,exist_ok=True)
            if path.exists(): raise ValueError('Refusing screenshot overwrite')
            im=ImageGrab.grab(xdisplay=os.environ['DISPLAY']).convert('RGB')
            im.save(path)
            return {'path':str(path),'size':list(im.size)}
        finally: x.XCloseDisplay(d)
    x,t,d=connect()
    try:
        if action in ('move','click'):
            px,py=int(kwargs['x']),int(kwargs['y'])
            if not 0<=px<3840 or not 0<=py<2160: raise ValueError('coordinate outside allowed capture dimensions')
            t.XTestFakeMotionEvent(d,-1,px,py,0); x.XFlush(d);time.sleep(.08)
            if action=='click':
                button=int(kwargs.get('button',1))
                if button not in (1,2,3,4,5):raise ValueError('invalid button')
                t.XTestFakeButtonEvent(d,button,True,0);x.XFlush(d);time.sleep(.05)
                t.XTestFakeButtonEvent(d,button,False,0)
        elif action=='key':
            keys=kwargs['keys'].split('+'); codes=[]
            for key in keys:
                sym=x.XStringToKeysym(key.encode()); code=x.XKeysymToKeycode(d,sym)
                if not code:raise ValueError('unknown key: '+key)
                codes.append(code)
            for code in codes:t.XTestFakeKeyEvent(d,code,True,0)
            x.XFlush(d);time.sleep(.04)
            for code in reversed(codes):t.XTestFakeKeyEvent(d,code,False,0)
        else:raise ValueError('unknown action')
        x.XSync(d,False)
        return {'action':action,**kwargs}
    finally:x.XCloseDisplay(d)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('action',choices=['shot','move','click','key'])
    p.add_argument('--path');p.add_argument('--x',type=int);p.add_argument('--y',type=int)
    p.add_argument('--keys');p.add_argument('--button',type=int,default=1)
    a=vars(p.parse_args()); action=a.pop('action')
    print(json.dumps(act(action,**a)))
