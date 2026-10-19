"""Native authored rows and action layouts; reference art guides gesture only.

The turnaround fixes construction, not acting. Every component and silhouette
below is drawn at the displayed grain. No reference image is opened or sampled.
"""
import json
from PIL import Image
from arcade_knight_contacts import CONTACT_ROWS, DETAILS

INK = dict(zip('cabdefrstuvklmn', [
    '22303a','2f4a52','456070','6e8a8e','8fa3a8','b3c2c4',
    '61303b','853d43','ad4e49','cc6854','dc8768',
    '684629','924f1d','a97639','be914b']))
INK.update(C=INK['c'], P='0a0c12', B='94968c', D='b0b2a8')
# Dented crown, cool/warm steel bands, and a broad raised visor brim. The
# repaired bandage is a connected 2px strip, not random scuff texture.
HELMET = [
    '.....ccccccccc',
    '...ccbbddddeeecc',
    '..cabbddddeeffffc',
    '.caabbbdddeeefffc',
    'caaabbbdddddeeeec',
    'caabbbdddeeCCCCCCcc',
    'caabbbddeCCnnnnnnc',
    'caabbbddemnkefnnnc',
    'caabbbddemnkPPnnnnc',
    '.caabbddemnkPPnnnc',
    '..cabbdddmnCCCCCnnc',
    '..cabbdddmmnnnnnnc',
    '...cabbbddmCCCCCCc',
]
# Four face kits retain the 7x6 opening, angled brow, 2x3 eye and 5px moustache.
FACES = {
    'neutral': ['nCCnnnn','mnkPPnn','mnkPPnnn','mnkPPnn','mmnnnnn','mCCCCCn','mCCnnCC'],
    'gritted': ['nCCCCnn','mnkPPCn','mnkPPnnn','mnkPPnn','mmnnnnn','mCCCCCn','mmbbbbn','mCCnnCC'],
    'shout':   ['nCCnnnn','mnkPPnn','mnkPPnnn','mnkPPnn','mmnnnnn','mCCCCCn','mnkkknn','mnkkknn','mCCCCCC'],
    'grin':    ['nCCnnnn','mnkPPnn','mnkPPnnn','mnkPPnn','mmnnnnn','mCCCCCn','mmddddn','mCCnnCC'],
}
PLUMES = [
    ['...cccccccc','..crrssstttc','.crrsssttttc','..crrsssttc','....crsscc','.....cccc'],
    ['.cccccccc','crrssstttc','crrsssttttc','.crrsssttc','...crsscc','....ccccc'],
    ['ccccccccc','crrsssttttc','.crrssstttc','..crrssstc','....csscc','.....ccc'],
    ['..cccccccc','.crrssstttc','crrsssttttc','.crrsssttc','...crsscc','....ccccc'],
]
PLUME_FLOP = ['..cccccccccc','.crrssstttttc','crrsssttttttc','crrsssstttc','.ccccrsscc','....cccc']
# Broad cel folds only. No crosshatch, checker, diagonally repeated stitches.
BODY = [
    '...ccccccccccccccc',
    '.ccaabbbdddeeeddbbcc',
    'caabbbddddeeeeddbbaac',
    'caaabbbdddddddbbbaaac',
    'caaabcrrrrssssttttttc',
    '.caabcrrrrssssttttttc',
    '..cabcrrrrssssttttttc',
    '...ccrrrrsssssttttttc',
    '...crrrrssssssttttttc',
    '..crrrrsssssssttttttc',
    '..crrrssssssstttttuuc',
    '..crrrsssssstttuvvuuc',
    '...crrrsssstttttuuuc',
    '....cccccccccccccc',
]
LEAN_BODY = [
    '........cccccccccccccc',
    '......ccaabbbdddeeeddbcc',
    '.....caabbbddddeeeedbbaac',
    '....caaabbbddddddbbbaaac',
    '...caaabcrrrrssssttttttc',
    '..caaabcrrrrssssttttttc',
    '.caaabcrrrrssssttttttc',
    '.caabcrrrrsssssttttttc',
    '..ccrrrrssssssttttttc',
    '..crrrrsssssssttttttc',
    '.crrrssssssstttttuuc',
    '.crrrsssssstttuvvuuc',
    '..crrrsssstttttuuuc',
    '...cccccccccccccc',
]
BACK_BODY = [
    'ccccccccccccccccccc',
    'caabbbdddeeeddbbbaac',
    '.caabbbddddeedbbbaac',
    '..caabbbdddddbbbaac',
    '...crrrrssssttttttc',
    '...crrrrssssttttttc',
    '...crrrrssssttttttc',
    '...crrrrssssstttttc',
    '....crrrrssssttttttc',
    '....crrrrssssssttttc',
    '....crrrrssssstttuuc',
    '.....crrrssssttuvvuuc',
    '.....crrrsssttttuuuc',
    '......cccccccccccccc',
]
BODY_LYING = [
    '....cccccccccccccccc',
    '..ccrrrsssssstttttttcc',
    '.crrrrsssssstttttuuubbc',
    'crrrrssssssttttuuuvvbbbc',
    'crrrrsssssstttuuuuubbbbc',
    'crrrrssssstttttttttbbbddc',
    'crrrrsssssstttttttbbbdeec',
    '.crrrrsssssttttttbbbddeec',
    '..crrrrsssssttttbbbddeec',
    '...crrrrssssstttbbbdddc',
    '....cccccccccccccccc',
]
PAULDRON = ['..cccc..','.cbdeec.','cabdeefc','caabdeec','caabbddc','.cccccc.']
HAND = ['.ccccc.','cbbdeec','cbbdeec','cabdddc','cabbddc','caaabbc','.ccccc.']
CUFF = ['cbbddc','caabbc','cmmnnc','cccccc']
ARM_RIGHT = ['..cccccc','.cabbdddc','cabbdddeec','cabbddddec','.caabbbbc','..cccccc']
ARM_UP = ['.ccccc','cabbdec','cabbdec','caabddc','caabddc','caabbdc','caabbdc','.cabbc','..ccc']
ARM_BACK = ['.....cccc','...ccbbddc','..cabbdddc','.cabbdddc','cabbddcc','caabbc','.cccc']
SHIELD = [
    '...cccccccc', '.ccbbbddddecc','cabbdddeeefffc','cabccrrrrccfc',
    'cabcrrssstcfc','cabcrrssstcfc','cabcrrssstcfc','cabcrrssstcfc',
    'cabcrrssstcfc','cabcrrssstcfc','cabcrrssstcfc','cabccrrsccdc',
    '.cabccrccdc','..cabbdddcc','...cabbddcc','....ccccccc',
]
BOOT = ['..cccc..','.cbddeec','cabbdeec','caaabddc','caaaaaac','cccccccc']
BOOT_BACK = ['.cccccc.','cabdddec','cabbddec','caabbddc','caaaaaac','cccccccc']
BOOT_UP = ['.cccccc.','cabddefc','cabddefc','caabbddc','caabbddc','.caabbc.','..cccc..']
LEG = ['.cccccc','cabbdeec','cabbdeec','caabbddc','caabbddc','caabbdc','caabbdc','caabbdc','caabbdc']
LEG_FORWARD = ['cccccc','cabbdec','.cabbdec','..cabbdec','...cabbdec','....cabbddc','.....cabbddc','......cabbc']
LEG_REAR = ['.....cccccc','....cabbdec','...cabbddc','..cabbddc','.cabbddc','cabbddc','caabdc','caabc']
LEG_TUCK = ['.cccccc','cabbdeec','cabbdddc','.cabbddc','..caabddc','..cabbddc','.cabbddc','..ccccc']
LEG_HIGH = ['..cccccc','.cabbddc','cabbddc','cabbddc','cabbddc','.caabddc','..caabddc','...ccccc']
LEG_UP = ['..cccccc','..cabbdec','.cabbdec','cabbddc','cabbddc','caabbdc','.cccccc']
SWORD_UP = [
    '..ccc','.cbdfc','cabdfc','cabdfc','cabdec','cabdec','cabdec','cabdec',
    'cabdec','cabdec','cabdec','cabdec','cabdec','cabdec','cabdec','cabdec',
    '.ccccc','cccccccc','cmmnnnnc','cccccccc','ccckkc','..ckkc','..cccc',
]
SWORD_RIGHT = ['...ccc','...cmc','...cmc','cccccccccccccccccccccc',
               'ckkkcbbbddddeeeefffffc','ckkkcaaabbbbdddddeeeec',
               'cccccccccccccccccccccc','...cmc','...ccc']
SWORD_DIAG = [
    '................ccc','...............cbfc','..............cbdfc',
    '.............cabdec','............cabdec','...........cabdec',
    '..........cabdec','.........cabdec','........cabdec','.......cabdec',
    '......cabdec','.....cabdec','....cabdec','...cabdec','..cabdec',
    '.cccccc','cmmnnc','cccccc','.ckkc','.ckkc','..ccc',
]
SWORD_DOWN = [
    '..cccc','..ckkc','.cckkcc','cmmnnnc','.cccccc','..cabdec',
    '...cabdec','....cabdec','.....cabdec','......cabdec','.......cabdec',
    '........cabdec','.........cabdec','..........cabdec','...........cabdec',
    '............cabdfc','.............cbdfc','..............cbfc','...............ccc',
]
SWORD_BACK = [
    '..ccc','.cfbc','cfdbc','cfdbac','.cedbac','..cedbac','...cedbac',
    '....cedbac','.....cedbac','......cedbac','.......cedbac','........cedbac',
    '.........cedbac','..........cedbac','...........cedbac','............cccccc',
    '.............cnmmc','.............cccccc','..............ckkc',
    '..............ckkc','...............ccc',
]
# 30px-wide, 140-degree authored crescent. Three material bands follow the
# curved stroke, rather than radial hairlines or a ring around the character.
SMEAR = [
    '......cccccccccccccccc',
    '....ccbbbbdddddeeeeeeccc',
    '..ccbbbdddddeeeeeeeeffffcc',
    '.cbbbdddddeeeeccccceeeefffcc',
    'cbbbddddeeeccc.....ccceeeeffc',
    'cbbdddeeecc.........cceeeeffc',
    '.cbddeecc............cceeeffc',
    '..ccccc...............cceeffc',
    '.......................ceeffc',
    '.......................ceeffc',
    '.......................ceeffc',
    '......................cdeeffc',
    '.....................cddeefc',
    '....................cdddeefc',
    '...................cdddeefc',
    '..................cdddeefc',
    '.................cdddeefc',
    '................cdddeefc',
    '...............cdddeefc',
    '..............cdddeefc',
    '.............cdddeefc',
    '..............cccccc',
]

class Drawing:
    def __init__(self):
        self.px={};self.owners={};self.details=[];self.semantic={};self.silhouette=[]
    def part(self,name,rows,x,y):
        for yy,row in enumerate(rows):
            for xx,ch in enumerate(row):
                if ch=='.':continue
                p=x+xx,y+yy
                assert ch in INK and 0<=p[0]<64 and 0<=p[1]<56,(name,p,ch)
                base=dict(helmet='a',coat='r',belt='k',shield='a',legs='a',boots='a',
                          arm='a',hand='a',sword='a',plume='r',pauldron='a',smear='b')[name]
                self.px[p]=INK[base if ch=='c' else ch];self.owners[p]=name
    def detail(self,name,points):self.details.append((name,points))
    def save(self,path,tags,head,hip,expression):
        for owner,x,y,row in CONTACT_ROWS.get(self.contact_key,[]):
            for j,ch in enumerate(row):
                p=x+j,y
                if ch=='.':self.px.pop(p,None);self.owners.pop(p,None)
                else:self.px[p]=INK[ch];self.owners[p]=owner
        hx,hy=head[0]-9,head[1]-6
        # Expression marks are construction, never smoothed into neighbouring
        # coat or armour planes by the fixed contact corrections.
        for yy,row in enumerate(FACES[expression]):
            for xx,ch in enumerate(row):
                p=hx+10+xx,hy+6+yy
                if p in self.px:
                    self.px[p]=INK[ch];self.owners[p]='helmet'
        for yy,row in enumerate(['BB','BB','DD','DD']):
            for xx,ch in enumerate(row):
                p=hx+4+xx,hy+2+yy
                if p in self.px:self.px[p]=INK[ch];self.owners[p]='helmet'
        for xx in [10,11]:self.px[hx+xx,hy+2]=INK['a']
        solid=set(self.px)
        for x,y in solid:
            if any(p not in solid for p in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]):self.px[x,y]=INK['c']
        im=Image.new('RGBA',(64,56))
        for p,c in self.px.items():im.putpixel(p,tuple(bytes.fromhex(c))+(255,))
        im.save(path)
        parts={n:sorted([list(p) for p in self.px if self.owners[p]==n])
               for n in ['helmet','sword','shield','pauldron','hand','boots','plume']}
        parts.update(self.semantic)
        regions=[dict(name=n,material=mat,minimum_tones=5,
                      pixels=sorted([list(p) for p in self.px if self.owners[p]==n]))
                 for n,mat in [('helmet','steel'),('coat','cloth')]]
        self.details.extend(DETAILS.get(self.contact_key,[]))
        features=[]
        for name,points in self.details:
            if all(p in self.px for p in points) and len({self.px[p] for p in points})==1:
                features.append(dict(name=name,pixels=[list(p) for p in points]))
        return dict(file=path.name,required_materials={'steel':5,'cloth':5},material_regions=regions,
                    features=features,parts=parts,knight_rules=True,silhouette_features=self.silhouette,
                    acting=tags,landmarks=dict(head=list(head),hip=list(hip)),expression=expression)

def helmet(d,hx,hy,expression,plume,flop=False):
    d.part('helmet',HELMET,hx,hy)
    d.part('helmet',FACES[expression],hx+10,hy+6)
    # The bandage bridges the dent with a warm grey stitch-shaped plane.
    d.part('helmet',['BB','BB','DD','DD'],hx+4,hy+2)
    d.part('helmet',['aa'],hx+10,hy+2)
    d.detail('bandage seam',[(hx+6,hy+3),(hx+6,hy+4)])
    d.detail('crown glint',[(hx+11,hy+1),(hx+12,hy+1),(hx+13,hy+1)])
    d.detail('helmet dent',[(hx+10,hy+2),(hx+11,hy+2)])
    d.part('plume',PLUME_FLOP if flop else PLUMES[plume%4],hx-1-(2 if flop else 0),hy-6)
    d.semantic['face']=[[hx+x,hy+y] for y in range(6,6+len(FACES[expression])) for x in range(10,17)]
    d.semantic['eye']=[[hx+x,hy+y] for y in range(7,10) for x in [13,14]]
    d.semantic['visor']=[[hx+x,hy+y] for y in [7,8,9] for x in [13,14]]
    d.semantic['moustache']=[[hx+x,hy+11] for x in range(11,16)]
    d.detail('visor slit',[(hx+13,hy+8),(hx+13,hy+9)])
    d.detail('visor slit',[(hx+14,hy+8),(hx+14,hy+9)])
    jaw_y=hy+(12 if expression=='neutral' else 13)
    d.detail('jaw light',[(hx+13,jaw_y),(hx+14,jaw_y)])
    d.detail('shout cheek',[(hx+11,hy+12),(hx+11,hy+13)])
    d.detail('jaw left',[(hx+11,jaw_y),(hx+12,jaw_y)])
    d.detail('jaw right',[(hx+15,jaw_y),(hx+16,jaw_y)])
    d.detail('cheek shadow',[(hx+12,hy+y) for y in [7,8,9]])

    d.detail('angled brow',[(hx+11,hy+6),(hx+12,hy+6)])
    if expression in ('grin','gritted'):d.detail('teeth',[(hx+x,hy+12) for x in [12,13,14]])

def blade(d,kind,x,y):
    rows=dict(up=SWORD_UP,right=SWORD_RIGHT,diag=SWORD_DIAG,down=SWORD_DOWN,back=SWORD_BACK)[kind]
    d.part('sword',rows,x,y)
    hand=dict(up=(x,y+18),right=(x-2,y+2),diag=(x,y+15),down=(x,y),back=(x+12,y+15))[kind]
    d.part('hand',HAND,*hand)
    # Brass is a two-pixel cuff trim on the steel fist, never a mitten.
    d.part('hand',['nn'],hand[0]+1,hand[1]+5)
    d.detail('repaired brass cuff',[(hand[0]+1,hand[1]+5),(hand[0]+2,hand[1]+5)])
    if kind=='up':pts=[(x+4,y+j) for j in [1,2,3]]
    elif kind=='right':pts=[(x+j,y+4) for j in [18,19,20]]
    elif kind=='diag':pts=[(x+17,y+1),(x+17,y+2)]
    elif kind=='down':pts=[(x+16,y+15),(x+16,y+16)]
    else:pts=[(x+1,y+2),(x+1,y+3)]
    d.detail('blade glint',pts)
    return hand

def frame(folder,state,i):
    source_i=i
    if state=='defeat':i=[0,1,3,2,3][i]
    d=Drawing();d.contact_key=f'{state}-{i}';tags=[];ex='neutral';hx,hy=23,12;bx,by=19,25
    body=BODY;shield=(12,30);sword=('up',45,16);arm=(38,30);shoulder=(35,25)
    # Each tuple: leg row set/origin, boot row set/origin. Contact spread is
    # fourteen pixels, passing legs lift visibly, and the head leads the hips.
    idlelegs=[(LEG,22,39,BOOT_BACK,20,46),(LEG,33,39,BOOT,34,46)]
    legs=idlelegs;plume=i%4;hip=(30,39)
    if state=='idle':
        dy=[0,-1,0,1][i];shift=[0,0,-1,-1][i]
        hx+=shift;hy+=dy;bx+=shift;by+=dy;shoulder=(35+shift,25+dy)
        shield=(12+shift,30+[0,0,1,1][i]);arm=(38+shift,30+dy);sword=('up',45+shift,16+dy)
    elif state=='run':
        tags=['lean'];body=LEAN_BODY;bx=17;hx=23
        dy=[0,2,1,-1,1,-1][i];hy+=dy;by+=dy
        hip=(28,39+dy);plume=[3,0,1,2,3,0][i]
        shoulder=(37,25+dy)
        shield=[(8,29),(10,31),(14,28),(13,24),(14,26),(10,27)][i]
        sword=[('diag',40,20),('diag',39,22),('diag',37,19),('diag',34,17),('diag',37,19),('diag',40,18)][i]
        arm=(37,29+dy)
        legs=[
            [(LEG_REAR,20,39,BOOT_BACK,18,46),(LEG_FORWARD,29,39,BOOT,32,46)],
            [(LEG_HIGH,22,40,BOOT_BACK,18,44),(LEG_TUCK,31,41,BOOT,34,46)],
            [(LEG_TUCK,21,39,BOOT_BACK,21,42),(LEG,30,39,BOOT,31,46)],
            [(LEG_FORWARD,23,37,BOOT_BACK,28,42),(LEG_REAR,32,37,BOOT,34,42)],
            [(LEG_FORWARD,29,39,BOOT_BACK,32,46),(LEG_REAR,20,39,BOOT,18,46)],
            [(LEG,25,38,BOOT_BACK,24,44),(LEG_HIGH,32,37,BOOT,30,41)],
        ][i]
    elif state=='attack':
        tags=['lean' if i in [1,2,3] else 'squash'];ex='gritted'
        hx,hy=[(24,15),(26,12),(27,12),(25,15),(24,14),(23,12)][i]
        bx,by=[(18,28),(19,25),(21,25),(20,28),(19,27),(19,25)][i]
        body=LEAN_BODY if i in [1,2,3] else BODY
        shoulder=[(35,27),(39,25),(41,25),(38,28),(36,27),(35,25)][i]
        shield=[(11,31),(9,28),(7,29),(9,32),(11,32),(12,30)][i]
        sword=[('back',10,7),('up',44,6),('right',42,25),('down',41,33),('down',13,33),('up',45,16)][i]
        arm=[(25,29),(38,23),(36,27),(35,33),(21,32),(38,30)][i]
        legs=[
            [(LEG_TUCK,21,40,BOOT_BACK,19,46),(LEG_TUCK,32,40,BOOT,35,46)],
            [(LEG_REAR,22,39,BOOT_BACK,18,46),(LEG_FORWARD,32,39,BOOT,39,46)],
            [(LEG_REAR,22,39,BOOT_BACK,18,46),(LEG_FORWARD,34,39,BOOT,40,46)],
            [(LEG_REAR,22,40,BOOT_BACK,18,46),(LEG_TUCK,36,40,BOOT,40,46)],
            [(LEG_TUCK,22,40,BOOT_BACK,20,46),(LEG,33,40,BOOT,35,46)],idlelegs,
        ][i];hip=(32,39)
        if i==1:d.part('smear',SMEAR,30,1)
    elif state=='hit':
        tags=['lean'];ex='shout';body=BACK_BODY
        hx,hy=[(16,9),(18,12)][i];bx,by=[(15,25),(17,27)][i]
        shoulder=(29+i,25+i*2);shield=(7+i*2,24+i*3);arm=(30+i,24+i*3)
        sword=('back',32,12+i*3)
        legs=[(LEG_REAR,21,39,BOOT_BACK,18,46),(LEG_FORWARD,32,39,BOOT,37,46)]
    elif state=='guard':
        tags=['squash'];hx,hy=26,14+i;bx,by=21,27+i;shoulder=(39,27+i)
        shield=(37,17+i);sword=('up',14,17+i);arm=(24,30+i)
        legs=[(LEG_REAR,23,40,BOOT_BACK,21,46),(LEG_TUCK,34,40,BOOT,38,46)]
    elif state=='jump':
        tags=['squash' if i==1 else 'stretch'];hx,hy=[(25,10),(27,11)][i];bx,by=[(20,23),(21,24)][i]
        shoulder=(37+i,23+i);shield=(12,23-i);sword=('diag',39,9-i)
        arm=(37,23);legs=[(LEG_TUCK,22,35,BOOT_BACK,21,41-i),(LEG_HIGH,33,34,BOOT,35,39-i)]
    elif state=='fall':
        tags=['stretch'];hx,hy=25,10;bx,by=20,23;shoulder=(37,23)
        shield=(11,26);sword=('down',42,26);arm=(37,28)
        legs=[(LEG_FORWARD,24,37,BOOT_BACK,28,45),(LEG,34,37,BOOT,35,45)]
    elif state=='land':
        tags=['squash'];hx,hy=24,16-i;bx,by=18,29-i;shoulder=(36,29-i)
        shield=(9,33-i);sword=('down',43,30-i);arm=(38,33-i)
        legs=[(LEG_TUCK,19,41,BOOT_BACK,18,46),(LEG_HIGH,32,41,BOOT,36,46)]
    elif state=='victory':
        tags=['stretch'];ex='grin';hx,hy=[(23,14),(23,16),(23,16)][i];bx,by=19,28
        shoulder=(37,29);shield=[(10,28),(8,24),(8,26)][i]
        sword=[('diag',39,8),('up',44,0),('up',44,0)][i]
        arm=[(36,26),(41,22),(41,23)][i]
        legs=[(LEG_REAR,23,40,BOOT_BACK,20,46),(LEG_FORWARD,32,40,BOOT,37,46)]
    elif state=='defeat':
        tags=['lean'];ex='shout'
        if i<2:
            hx,hy=[(20,12),(16,22)][i];bx,by=[(16,27),(14,35)][i]
            body=BACK_BODY;shoulder=(29,by);shield=(6,31+i*5);arm=(30,by)
            legs=[(LEG_HIGH,28,37,BOOT_UP,31,32+i*5),(LEG_UP,38,36,BOOT_UP,41,30+i*4)]
        else:
            hx,hy=12,36-(1 if i==2 else 0);bx,by=28,39-(1 if i==2 else 0);body=BODY_LYING
            shoulder=(38,41);shield=(1,40);arm=(31,48)
            legs=[(LEG_UP,38,36-i%2,BOOT_UP,41,29-i%2),(LEG_UP,45,39-i%2,BOOT_UP,49,33-i%2)]
    # Back gear and armour first; hands and shield remain visible above them.
    for rows,x,y,boot,bxx,byy in legs:d.part('legs',rows,x,y);d.part('boots',boot,bxx,byy)
    d.part('coat',body,bx,by)
    if state=='hit' or (state=='defeat' and i==0):
        d.part('arm',['.cccccc','cabbdddc','cabbdddc','caabbbbc','.cccccc'],hx+7,hy+12)
    if state!='defeat' or i<2:d.part('belt',['ckkkmmmmnnnnnc'],bx+5,by+9)
    helmet(d,hx,hy,ex,plume,flop=state=='defeat')
    d.part('pauldron',PAULDRON,*shoulder)
    d.part('arm',ARM_BACK if state=='attack' and i in [0,4] else ARM_UP if state=='victory' and i>0 else ARM_RIGHT,*arm)
    if state!='defeat':
        hand=blade(d,*sword)
        if state=='attack' and i==0:
            # Shoulder-cocked grip at the rear of the helmet, connected to elbow.
            d.part('arm',ARM_UP,24,24);blade(d,*sword)
        if state=='victory':
            # Off-hand pump is above the counterbalancing shield, then settles.
            d.part('arm',ARM_UP,16,22 if i==1 else 24)
            d.part('hand',HAND,14,18 if i==1 else 21)
    d.part('shield',SHIELD,*shield)
    if state=='victory' and i==1:d.part('hand',HAND,14,18)
    if state=='guard':
        # Re-open the face beside the shield; the raised field braces the cheek.
        helmet(d,hx,hy,ex,plume)
        d.part('shield',SHIELD,42,17+i)
    # Explicit joint contacts are native strokes. No silhouette cleanup or
    # pigment interpolation decides the acting on our behalf.
    if state=='attack' and i==2:d.part('arm',['aaa']*7,20,31)
    d.detail('cloth hem',[(bx+15,by+11),(bx+16,by+11)])
    d.head=(hx,hy)
    return d.save(folder/f'{state}-{source_i}.png',tags,(hx+9,hy+6),hip,ex)

def build(folder,states):
    m=dict(version=3,canvas=[64,56],anchor=[30,51],facing='right',native_height=46,
           model=dict(plume=6,helmet=13,torso=14,legs=13),states={})
    for state,n,duration in states:
        frames=[frame(folder,state,i) for i in range(n)]
        times=[duration]*n
        if state=='attack':times=[220,60,90,100,100,130]
        if state=='defeat':times=[95,95,75,60,180]
        if state=='run':times=[90,65,75,65,90,75]
        if state=='land':times=[60,80]
        m['states'][state]=dict(durations_ms=times,loop=state in ['idle','run','guard','victory'],frames=frames)
    (folder/'manifest.json').write_text(json.dumps(m,separators=(',',':'))+'\n')
