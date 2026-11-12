"""Native Apollo capture through a private D-Bus/X display, never model services.
Run under dbus-run-session with the isolated seat environment. All clicks are
XTest events; AT-SPI is observation/label evidence, not a substitute click.
"""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
import gi

gi.require_version('Atspi','2.0')
from gi.repository import Atspi
from PIL import ImageGrab
sys.path.insert(0,str(Path(__file__).resolve().parent))
from native_x11 import act

Atspi.init(); Atspi.set_timeout(800,3000)

def tree(app_filter=''):
    found=[];budget=[2400]
    windows=subprocess.check_output(['xwininfo','-root','-tree'],text=True)
    anchors={}
    for line in windows.splitlines():
        m=re.search(r'0x[0-9a-f]+ "(.*?)": .*?\s(\d+)x(\d+)[+-]\d+[+-]\d+\s+([+-]\d+)([+-]\d+)',line)
        if m and int(m[2])>100 and int(m[3])>100:
            anchors.setdefault(m[1],[]).append([int(m[4]),int(m[5])])
    def walk(n,path,app,anchor=None):
        budget[0]-=1
        if n is None or budget[0]<0 or len(path)>30:return
        try:
            role=n.get_role_name(); name=(n.get_name() or '').strip()
            states=[s.value_nick for s in n.get_state_set().get_states()]
            component=n.get_component_iface();box=None
            if component:
                if role in ('frame','dialog'):
                    matches=anchors.get(name,[])
                    anchor=matches[0] if len(matches)==1 else None
                rect=Atspi.Component.get_extents(component,Atspi.CoordType.WINDOW)
                if anchor is not None and rect.width>1 and rect.height>1:
                    x,y=rect.x+anchor[0],rect.y+anchor[1]
                    box=[x,y,x+rect.width,y+rect.height]
            text=''
            ti=n.get_text_iface()
            if ti:
                length=Atspi.Text.get_character_count(ti)
                text=Atspi.Text.get_text(ti,0,min(length,500))
            ai=n.get_action_iface(); actions=[]
            if ai:
                actions=[Atspi.Action.get_action_name(ai,i) for i in range(Atspi.Action.get_n_actions(ai))]
            found.append({'app':app,'path':path,'role':role,'name':name,'box':box,'states':states,'text':text,'actions':actions})
            count=n.get_child_count()
            for i in range(min(count,350)):walk(n.get_child_at_index(i),path+[i],app,anchor)
        except Exception:pass
    desk=Atspi.get_desktop(0)
    for i in range(desk.get_child_count()):
        app=desk.get_child_at_index(i);name=app.get_name() or ''
        if app_filter.lower() in name.lower():walk(app,[i],name)
    return found


def normalized(nodes):
    return [(n['role'],n['name'],n['text'],[s for s in n['states'] if s in ('checked','selected','expanded','pressed','editable')]) for n in nodes]


def elements(nodes,size):
    roles={'push button','toggle button','check box','radio button','menu item','check menu item','radio menu item','page tab','combo box','text','entry','spin button','slider','table cell','icon','list item','link'}
    out=[]
    for n in nodes:
        b=n['box']
        if (n['role'] not in roles or not n['name'] or len(n['name'])>150 or b is None
                or not all(s in n['states'] for s in ('showing','visible'))
                or not ('sensitive' in n['states'] or 'enabled' in n['states'])):continue
        if not (0<=b[0]<b[2]<=size[0] and 0<=b[1]<b[3]<=size[1]):continue
        out.append({'target':f'the "{n["name"]}" {n["role"]} in {n["app"]}',
                    'box':b,'node_path':n['path'],'role':n['role'],'name':n['name'],
                    'label_method':'native-atspi-window-extents+x11-anchor','states':n['states'],'actions':n['actions']})
    # GTK exposes a wrapper and its nested toggle: use the deepest node.
    out=[e for e in out if not any(e is not other and e['name']==other['name']
         and other['node_path'][:len(e['node_path'])]==e['node_path']
         and len(other['node_path'])>len(e['node_path']) for other in out)]
    counts=Counter(e['target'] for e in out)
    return [e for e in out if counts[e['target']]==1]


def atomic_json(path,obj):
    tmp=path.with_suffix(path.suffix+'.tmp');tmp.write_text(json.dumps(obj,indent=2)+'\n');tmp.replace(path)


def verify_effect(element,before,after):
    """Require the clicked widget's expected effect, never focus/hover alone."""
    name=element['name']
    text=lambda ns:[n['text'] for n in ns if n['role'] in ('text','entry') and n['text']]
    if 'calculator' in element['target'] and name.isdigit():
        a,b=text(before),text(after)
        return {'kind':'calculator-digit','before':a,'after':b,'expected':a[0]+name if a else None,
                'passed':bool(a and b and b[0]==a[0]+name)}
    matches=[n for n in after if n['path']==element['node_path'] and n['name']==name]
    opened=bool(matches and any(s in matches[0]['states'] and s not in element['states']
                               for s in ('checked','expanded','pressed')))
    before_names={(n['role'],n['name']) for n in before if 'showing' in n['states']}
    added=[(n['role'],n['name']) for n in after if 'showing' in n['states'] and (n['role'],n['name']) not in before_names]
    return {'kind':'named-toggle-opens','opened':opened,'added_controls':added,'passed':opened and bool(added)}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root');parser.add_argument('--probe',action='store_true')
    args=parser.parse_args();root=Path(args.root).resolve()
    if not os.environ.get('DISPLAY','').startswith(':18'):raise ValueError('Private dataset display required')
    root.mkdir(exist_ok=False,parents=True);(root/'shots').mkdir();(root/'trees').mkdir();(root/'projects').mkdir()
    pages=[];transitions=[];failures=[];attempts=[];child=None
    plan=[
        ('calculator-basic','train','native-calculator',['gnome-calculator','--mode=basic','--equation=17+25'],'gnome-calculator'),
        ('calculator-advanced','train','native-calculator',['gnome-calculator','--mode=advanced','--equation=12*8'],'gnome-calculator'),
        ('calculator-financial','validation','native-calculator',['gnome-calculator','--mode=financial','--equation=1500*1.05'],'gnome-calculator'),
        ('calculator-programming','test','native-calculator',['gnome-calculator','--mode=programming','--equation=255'],'gnome-calculator'),
        ('editor-note','train','native-text-editor',['gnome-text-editor',str(root/'projects/notes.txt')],'text'),
        ('editor-code','train','native-text-editor',['gnome-text-editor',str(root/'projects/example.py')],'text'),
        ('editor-checklist','validation','native-text-editor',['gnome-text-editor',str(root/'projects/checklist.md')],'text'),
        ('editor-shader','test','native-text-editor',['gnome-text-editor',str(root/'projects/example.glsl')],'text'),
        ('files-documents','train','native-files',['nautilus','--new-window',str(root/'projects/Documents')],'nautilus'),
        ('files-assets','validation','native-files',['nautilus','--new-window',str(root/'projects/Assets')],'nautilus'),
        ('files-scenes','test','native-files',['nautilus','--new-window',str(root/'projects/Scenes')],'nautilus'),
    ]
    (root/'projects/notes.txt').write_text('Capture session notes\n\nAll content in this file is synthetic.\nUse the native editor menus to explore preferences and search.\n')
    (root/'projects/example.py').write_text('def triangle_area(base, height):\n    return base * height / 2\n\nprint(triangle_area(6, 4))\n')
    (root/'projects/checklist.md').write_text('# Release checklist\n\n- [ ] Review camera\n- [ ] Export preview\n- [ ] Save scene\n- [ ] Check lighting\n')
    (root/'projects/example.glsl').write_text('vec3 shade(vec3 normal) {\n    vec3 light = normalize(vec3(1.0, 2.0, 3.0));\n    return vec3(max(dot(normal, light), 0.0));\n}\n')
    for directory,names in [('Documents',['Brief.txt','Project.md','Archive']),('Assets',['Materials','Textures','Meshes']),('Scenes',['Forest','Harbor','Temple'])]:
        parent=root/'projects'/directory;parent.mkdir()
        for n in names:
            if '.' in n:(parent/n).write_text('Synthetic capture file '+n+'\n')
            else:(parent/n).mkdir()
    atomic_json(root/'plan.json',{'host':'Apollo','scope':'native-general-ui','model_calls':False,
                                 'scenarios':[{'group':'native-general:'+g,'split':s,'family':f,'argv':cmd} for g,s,f,cmd,_ in plan]})
    def save():
        for name,value in [('pages.json',pages),('transitions.json',transitions),('failures.json',failures),('attempts.json',attempts)]:atomic_json(root/name,value)
    def capture(group,split,family,state,app):
        # Snapshot screen and tree while no input is being sent.
        act('move',x=1800,y=1000);time.sleep(.2)
        nodes=tree(app);im=ImageGrab.grab(xdisplay=os.environ['DISPLAY']).convert('RGB')
        targets=elements(nodes,im.size)
        if not targets:raise ValueError('no visible unambiguous native controls: '+app)
        i=len(pages);shot=f'shots/{i:05d}.png';raw=f'trees/{i:05d}.json'
        im.save(root/shot);atomic_json(root/raw,nodes)
        row={'shot':shot,'sha256':hashlib.sha256((root/shot).read_bytes()).hexdigest(),
             'url':'native://'+family+'/'+group,'family':family,'group':'native-general:'+group,
             'split':split,'state':state,'width':im.width,'height':im.height,'elements':targets,
             'tree':raw,'tree_sha256':hashlib.sha256((root/raw).read_bytes()).hexdigest(),
             'collection':'real-native-ui-scripted-XTest','host':'Apollo'}
        pages.append(row);save();return row,nodes
    def stop_child():
        nonlocal child
        if child:
            try:os.killpg(child.pid,signal.SIGTERM)
            except ProcessLookupError:pass
            try:child.wait(timeout=8)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=5)
            child=None
    try:
        for group,split,family,argv,app in plan[:1] if args.probe else plan:
            log=(root/(group+'.log')).open('wb')
            child=subprocess.Popen(argv,env={**os.environ,'GTK_A11Y':'atspi','NO_AT_BRIDGE':'0','GDK_BACKEND':'x11','GSK_RENDERER':'cairo'},stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            try:
                for _ in range(80):
                    time.sleep(.25)
                    if elements(tree(app),(1920,1080)):break
                    if child.poll() is not None:raise RuntimeError('app exited '+str(child.returncode))
                time.sleep(1)
                before,nodes=capture(group,split,family,'initial',app)
                print(json.dumps({'group':group,'targets':len(before['elements']),'names':[e['name'] for e in before['elements']]}),flush=True)
                if args.probe:continue
                # Only safe named native controls; no account, disk, purchase, or deletion action.
                if family=='native-calculator':
                    wants=['7','8','9','4','5','6','1','2','3','0','Main Menu']
                elif family=='native-text-editor':
                    wants=['View','Main menu','Open']
                else:wants=['Search Current Folder','View Options','Main Menu']
                for index,want in enumerate(wants):
                    # Refresh so labels correspond to what the click actually saw.
                    current=elements(tree(app),(1920,1080))
                    matches=[e for e in current if e['name'].casefold()==want.casefold()]
                    if len(matches)!=1:continue
                    before,nodes=capture(group,split,family,'before-'+str(index)+'-'+want,app)
                    matches=[e for e in before['elements'] if e['name'].casefold()==want.casefold()]
                    if len(matches)!=1:raise ValueError('target changed before click')
                    e=matches[0];b=e['box'];click=[int((b[0]+b[2])/2),int((b[1]+b[3])/2)]
                    act('click',x=click[0],y=click[1]);time.sleep(.7)
                    after,after_nodes=capture(group,split,family,'after-'+str(index)+'-'+want,app)
                    effect=verify_effect(e,nodes,after_nodes)
                    changed=effect['passed'] and before['sha256']!=after['sha256']
                    attempts.append({'group':before['group'],'target':e['target'],'before':before['shot'],'after':after['shot'],
                                     'coordinate':click,'verified':changed,'oracle':effect})
                    if changed:
                        transitions.append({'group':before['group'],'label':'click '+e['name'],'before':before['shot'],'after':after['shot'],
                            'target':e['target'],'box':b,'coordinate':click,'scripted':True,'verified':True,
                            'oracle':effect})
                    if family!='native-calculator' or want=='Main Menu':act('key',keys='Escape')
                    time.sleep(.2);save()
                if not any(t['group']=='native-general:'+group for t in transitions):
                    raise ValueError('scenario produced no verified named-control clicks')
            except Exception as exc:
                failures.append({'group':'native-general:'+group,'error':str(exc)});save();print('FAILED '+group+': '+str(exc),flush=True)
            finally:stop_child();log.close();time.sleep(.5)
    finally:
        stop_child();save()
        atomic_json(root/'receipt.json',{'complete':not failures,'pages':len(pages),'transitions':len(transitions),
            'failures':len(failures),'host':'Apollo','native':True,'model_calls':False,'apps_stopped':child is None,
            'isolation':{'display':os.environ['DISPLAY'],'home':os.environ['HOME'],'private_dbus':True}})
    return 1 if failures else 0

if __name__=='__main__':sys.exit(main())
