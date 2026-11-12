"""Bounded private native-app seat. Run only inside a new mage-native directory."""
import argparse
import json
import os
from pathlib import Path
import secrets
import signal
import subprocess
import time

p=argparse.ArgumentParser()
p.add_argument('root');p.add_argument('--minutes',type=int,default=40)
a=p.parse_args();root=Path(a.root).resolve()
if 'mage-native' not in root.name or root.exists():raise SystemExit('New mage-native root required')
if not 1<=a.minutes<=90:raise SystemExit('invalid lifetime')
root.mkdir(parents=True);(root/'home').mkdir();(root/'runtime').mkdir(mode=0o700)
(root/'commands').mkdir();(root/'replies').mkdir();(root/'shots').mkdir()
xvfb=Path.home()/'.local/share/sq-blender/runtime/usr/bin/Xvfb'
auth=root/'Xauthority';auth.touch(mode=0o600)
display=next((f':{n}' for n in range(180,220) if not Path(f'/tmp/.X{n}-lock').exists() and not Path(f'/tmp/.X11-unix/X{n}').exists()),None)
if not display:raise SystemExit('No free private X display')
subprocess.run(['xauth','-f',str(auth),'add',display,'.',secrets.token_hex(16)],check=True)
env={**os.environ,'DISPLAY':display,'XAUTHORITY':str(auth),'HOME':str(root/'home'),
     'XDG_CONFIG_HOME':str(root/'home/.config'),'XDG_CACHE_HOME':str(root/'home/.cache'),
     'XDG_DATA_HOME':str(root/'home/.local/share'),'XDG_RUNTIME_DIR':str(root/'runtime'),
     'LIBGL_ALWAYS_SOFTWARE':'1','OMP_NUM_THREADS':'4','LP_NUM_THREADS':'4',
     'MAGE_NATIVE_ROOT':str(root)}
for key in ('WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','SESSION_MANAGER'):env.pop(key,None)
processes={};files=[];stop=False

def spawn(name,cmd,extra=None):
    if name in processes and processes[name].poll() is None:raise ValueError('App already running')
    log=(root/(name+'.log')).open('ab');files.append(log)
    child=subprocess.Popen(cmd,env={**env,**(extra or {})},cwd=root,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
    processes[name]=child
    return child.pid

def terminate(name):
    child=processes.get(name)
    if not child:return
    try:os.killpg(child.pid,signal.SIGTERM)
    except ProcessLookupError:return
    try:child.wait(timeout=10)
    except subprocess.TimeoutExpired:
        try:os.killpg(child.pid,signal.SIGKILL)
        except ProcessLookupError:pass
        child.wait(timeout=5)

def stopped(sig,frame):
    global stop
    stop=True
signal.signal(signal.SIGTERM,stopped);signal.signal(signal.SIGINT,stopped)
spawn('xvfb',[str(xvfb),display,'-screen','0','1920x1080x24','-nolisten','tcp','-auth',str(auth)])
try:
    for _ in range(80):
        if processes['xvfb'].poll() is not None:raise RuntimeError('Xvfb exited')
        if Path('/tmp/.X11-unix/X'+display[1:]).exists():break
        time.sleep(.1)
    state={'root':str(root),'display':display,'width':1920,'height':1080,'env':{k:env[k] for k in ('DISPLAY','XAUTHORITY','HOME','XDG_CONFIG_HOME','XDG_CACHE_HOME','XDG_DATA_HOME','XDG_RUNTIME_DIR','LIBGL_ALWAYS_SOFTWARE','OMP_NUM_THREADS','LP_NUM_THREADS','MAGE_NATIVE_ROOT')},'supervisor_pid':os.getpid()}
    (root/'seat.json').write_text(json.dumps(state,indent=2))
    print(json.dumps({'ready':True,**state}),flush=True)
    deadline=time.monotonic()+a.minutes*60
    while not stop and time.monotonic()<deadline:
        for path in sorted((root/'commands').glob('*.json')):
            reply=root/'replies'/path.name
            if reply.exists():continue
            try:
                c=json.loads(path.read_text());action=c['action']
                if action=='launch':
                    if c['name'] not in ('blender','unity','steam','wm'):raise ValueError('unsupported capture app')
                    result={'pid':spawn(c['name'],c['argv'],c.get('env'))}
                elif action=='stop':terminate(c['name']);result={'stopped':c['name']}
                elif action=='status':result={n:{'pid':p.pid,'exit':p.poll()} for n,p in processes.items()}
                elif action=='shutdown':stop=True;result={'shutdown':True}
                else:raise ValueError('unknown command')
                reply.write_text(json.dumps({'ok':True,**result}))
            except Exception as exc:reply.write_text(json.dumps({'ok':False,'error':str(exc)}))
        time.sleep(.2)
finally:
    for name in reversed(list(processes)):terminate(name)
    for f in files:f.close()
    (root/'cleanup.json').write_text(json.dumps({n:{'pid':p.pid,'exit':p.poll()} for n,p in processes.items()},indent=2))
    print('Private native seat stopped',flush=True)
