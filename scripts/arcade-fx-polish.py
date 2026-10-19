"""Native core, outlined warm chunks, thick rays and overlapping smoke lobes."""
from PIL import Image, ImageDraw

# A directional hot plane open to the upper-left, rather than a concentric
# bright ring trapped inside a darker circular core.
CORE = [
    '....gggg....', '..gggggghh..', '.ggggggghhh.',
    'gggggggghhhh','ggggggghhhhh','.gggghhhhhh.',
    '..gghhhhhh..','...hhhhhh...','....hhhh....',
]
CHUNKS = [
    ['..cc.','.cxxc','cxxxc','.cxxc','..cc.'],
    ['.ccc.','cxxxc','cxxxc','.cxxc','..cc.'],
    ['.cc..','cxxc.','cxxxc','cxxxc','.ccc.'],
]
PUFF = [
    '.....cccccc.........',
    '...ccppqqqqc........',
    '..cpppqqqqqqc.......',
    '.cppppqqqqqqc.......',
    'cmmmpppqqqqqccc.....',
    'cmmmppppqqqppqqcc...',
    'cmmmpppppppppqqqqc..',
    '.cmmmppppppppqqqqc..',
    '..cmmmpppppppqqqqc..',
    '...cmmmppppppqqqc...',
    '..cmmmmmmmpppppc....',
    '.cmmmmmmmmppppc.....',
    '.cmmmppppppppc......',
    '..ccccccccccc.......',
]
COLOURS={'c':'22303a','x':'d77b39','g':'e2cb8b','h':'d8a95e',
         'm':'322b2b','p':'515349','q':'63655c'}
def rows(a,shape,x,y):
    for yy,row in enumerate(shape):
        for xx,ch in enumerate(row):
            if ch!='.':a.putpixel((x+xx,y+yy),tuple(bytes.fromhex(COLOURS[ch]))+(255,))

def rays(state,i):
    a=Image.new('RGBA',(64,64));d=ImageDraw.Draw(a)
    if state=='impact':
        centre=(32,43)
        ends=[[(28,29),(44,33),(47,47),(37,56),(21,51),(21,36)],
              [(26,20),(51,29),(57,45),(43,61),(15,58),(10,34)],
              [(30,29),(45,35),(48,47),(36,55),(23,52),(22,37)]][i]
        core=[[(32,34),(35,40),(41,43),(35,46),(32,52),(29,46),(24,43),(29,40)],
              [(32,30),(37,39),(47,43),(38,48),(32,57),(27,48),(18,44),(27,39)],
              [(32,37),(35,41),(40,44),(35,47),(32,51),(29,47),(25,44),(29,41)]][i]
    else:
        centre=(23,44)
        ends=[[(36,31),(40,42),(37,54),(28,57)],
              [(41,24),(53,35),(57,46),(46,60),(32,61)],
              [(39,31),(48,40),(51,50),(39,58)]][i]
        core=[(23,38),(28,42),(34,44),(28,47),(23,51),(19,46),(20,42)]
    # Four-pixel ink envelope around a two-pixel honey/ember stroke. Draw all
    # envelopes first so crossing rays have one warm connected nucleus.
    for end in ends:d.line([centre,end],fill='#322b2b',width=4)
    for end in ends:d.line([centre,end],fill='#d77b39' if i==2 else '#d8a95e',width=2)
    d.polygon(core,fill='#a34828',outline='#22303a',width=1)
    inner=[(x+(centre[0]-x)//3,y+(centre[1]-y)//3) for x,y in core]
    d.polygon(inner,fill='#d77b39')
    d.ellipse((centre[0]-3,centre[1]-3,centre[0]+3,centre[1]+3),fill='#d8a95e')
    d.rectangle((centre[0]-1,centre[1]-1,centre[0]+1,centre[1]+1),fill='#e2cb8b')
    return a

def polish(canvas,state,i):
    if state in ['impact','spark']:
        if state=='impact' and i==3:
            a=Image.new('RGBA',(64,64));rows(a,PUFF,23,31);return a
        return rays(state,i)
    a=canvas.copy();d=ImageDraw.Draw(a)
    if state=='explosion' and i in [2,3,4]:
        ox,oy=[(26,33),(25,31),(26,30)][i-2]
        # Flatten the source's internal ring before laying the new hot plane.
        for y in range(oy-2,oy+12):
            for x in range(ox-3,ox+15):
                if a.getpixel((x,y))[:3] in [(226,203,139),(216,169,94)]:
                    a.putpixel((x,y),(216,169,94,255))
        for y,row in enumerate(CORE):
            for x,c in enumerate(row):
                if c!='.' and a.getpixel((ox+x,oy+y))[3]:
                    a.putpixel((ox+x,oy+y),tuple(bytes.fromhex(COLOURS[c]))+(255,))
    if state=='explosion' and 2<=i<=8:
        pixels={(x,y) for y in range(a.height) for x in range(a.width) if a.getpixel((x,y))[3]}
        left=set(pixels)
        while left:
            group={left.pop()};todo=list(group)
            while todo:
                x,y=todo.pop()
                for dx,dy in [(-1,-1),(0,-1),(1,-1),(-1,0),(1,0),(-1,1),(0,1),(1,1)]:
                    p=x+dx,y+dy
                    if p in left:left.remove(p);group.add(p);todo.append(p)
            if len(group)<20:
                for p in group:d.point(p,fill=(0,0,0,0))
        arc=[[(16,39),(42,38)],[(12,32),(46,31)],[(8,28),(50,27)],
             [(5,29),(53,28)],[(3,33),(55,32)],[(2,39),(56,38)],[(3,47),(55,46)]][i-2]
        for j,(x,y) in enumerate(arc):rows(a,CHUNKS[(i+j)%3],x,y)
    if state=='explosion' and i>=5:
        for y in range(a.height):
            for x in range(a.width):
                p=(x,y);c=a.getpixel(p)
                if c[:3] in [(61,58,54),(81,83,73)] and y>=43-(i-5):
                    a.putpixel(p,(50,43,43,255))
    return a
