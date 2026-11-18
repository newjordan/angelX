//! Cached room-space illumination. All fields and four flicker poses are
//! composed on room admission; live paint only copies registered RGB samples.
use super::*;

pub(super) fn colour(pack: Pack) -> Rgb {
    match pack {
        Pack::Crypt => [112, 73, 34],
        Pack::Cavern => [118, 79, 31],
        Pack::Hellforge => [138, 49, 19],
        Pack::Archive => [83, 104, 39],
        Pack::Fungal => [30, 102, 92],
        Pack::Unknown => [73, 58, 108],
        Pack::Swamp => [64, 92, 46],
    }
}
fn danger_colour(pack: Pack) -> Rgb {
    match pack {
        Pack::Hellforge => [175, 66, 22],
        Pack::Cavern => [100, 98, 36],
        Pack::Archive => [43, 89, 80],
        Pack::Fungal => [37, 96, 105],
        Pack::Crypt => [42, 85, 116],
        Pack::Unknown => [82, 72, 123],
        Pack::Swamp => [58, 96, 52],
    }
}
pub(super) fn fixtures(room: &Room, t: i32) -> Vec<(i32, i32)> {
    let (w,h)=(room.cols as i32,room.rows as i32);
    let mut out=vec![];
    for (c,r) in (3..w-2).step_by(5).flat_map(|c|[(c,0),(c,h-1)])
        .chain((2..h-2).step_by(5).flat_map(|r|[(0,r),(w-1,r)])) {
        if room.tile(c,r)==Tile::Wall && room.tile(c-1,r)!=Tile::Door && room.tile(c+1,r)!=Tile::Door
            && room.tile(c,r-1)!=Tile::Door && room.tile(c,r+1)!=Tile::Door {
            out.push((c*t+t/2,r*t+t/2));
        }
    }
    out
}
const AMBIENT: i32 = 36;
const REVEAL: i32 = 2;
const WARMTH: i32 = 150;

pub(super) fn open_fires(run: &Run) -> Vec<crate::render::overworld::light::Light> {
    let mut lights = Vec::new();
    if run.open_here().is_some() {
        super::super::open::fixtures(run, &mut Img::new(1, 1), &mut lights);
    }
    lights
}
fn luma(c:Rgb)->i32 {(c[0] as i32*54+c[1] as i32*183+c[2] as i32*19)/256}
fn lift(c:Rgb, add:Rgb, power:i32)->Rgb {
    let mut out=c;
    for k in 0..3 {out[k]=(c[k] as i32+add[k] as i32*power/256).min(255) as u8;}
    let value=luma(out);
    if value>110 {out=out.map(|v|(v as i32*110/value) as u8);}
    out
}
const STANDING: i32 = 120;

pub(super) struct Reveal {
    open: Vec<u16>,
    w: i32,
    s: i32,
}

impl Reveal {
    pub(super) fn grade(&self, layer: &mut Img) {
        for y in 0..layer.h {
            for x in 0..layer.w {
                let Some(c) = layer.get(x, y) else { continue };
                let open = self.open.get(((y / self.s) * self.w + x / self.s) as usize).copied().unwrap_or(0) as i32;
                let k = STANDING + open.min(256) * (256 - STANDING) / 256;
                layer.set(x, y, c.map(|v| (v as i32 * k / 256) as u8));
            }
        }
    }
}

pub(super) fn floor(cv:&mut Img,run:&Run,s:i32,fires:&[(i32,i32)]) -> (Vec<(i32,i32,[Rgb;4])>, Reveal) {
    let room=run.room(); let t=16*s;
    let (w,h)=(cv.w,cv.h);
    let (lw,lh)=(w/s,h/s);
    let mut light=vec![[0u16;3];(lw*lh) as usize];
    let mut reveal=vec![0u16;(lw*lh) as usize];
    let mut pool=|x:i32,y:i32,radius:i32,colour:Rgb,power:i32| {
        let (x,y,radius)=(x/s,y/s,radius/s);
        for yy in (y-radius).max(0)..(y+radius).min(lh) {
            for xx in (x-radius).max(0)..(x+radius).min(lw) {
                let d=(xx-x)*(xx-x)+(yy-y)*(yy-y);let r2=radius*radius;
                if d>=r2 {continue;}
                let a=(r2-d)*256/r2;let a=a*a/256*power/256;
                let i=(yy*lw+xx) as usize;
                let pixel=&mut light[i];
                for k in 0..3 {pixel[k]=pixel[k].saturating_add((colour[k] as i32*a/256) as u16);}
                reveal[i]=reveal[i].saturating_add((a*REVEAL) as u16);
            }
        }
    };
    for &(x,y) in fires {pool(x,y,t*3,colour(run.dungeon.pack),100);}
    for l in open_fires(run) {
        pool(l.x as i32*s,l.y as i32*s,l.r as i32*s,colour(run.dungeon.pack),(l.s*340.0) as i32);
    }
    let tint=danger_colour(run.dungeon.pack);
    for r in 0..room.rows as i32 {for c in 0..room.cols as i32 {
        if room.tile(c,r)==Tile::Hazard {pool(c*t+t/2,r*t+t/2,t*3/2,tint,70);}
    }}
    for (side,&(c,r)) in super::super::doors(room).iter().enumerate() {
        if room.doors[side] && !run.barred() {pool(c*t+t/2,r*t+t/2,t*2,[112,78,40],110);}
    }
    let raw=cv.clone();let mut shimmer=vec![];
    for r in 0..room.rows as i32 {for c in 0..room.cols as i32 {
        let tile=room.tile(c,r);
        if tile==Tile::Wall {continue;}
        let north=room.tile(c,r-1)==Tile::Wall;
        let west=room.tile(c-1,r)==Tile::Wall;
        let east=room.tile(c+1,r)==Tile::Wall;
        let dry_n=room.tile(c,r-1)!=Tile::Hazard;
        let dry_s=room.tile(c,r+1)!=Tile::Hazard;
        let dry_w=room.tile(c-1,r)!=Tile::Hazard;
        let dry_e=room.tile(c+1,r)!=Tile::Hazard;
        let step=if tile==Tile::Hazard || s>2 {1}else{s};
        for yy in (0..t).step_by(step as usize) {for xx in (0..t).step_by(step as usize) {
        let (x,y)=(c*t+xx,r*t+yy);
        let Some(source)=raw.get(x,y) else {continue;};
        let open=reveal[(y/s*lw+x/s) as usize] as i32;
        let ambient=AMBIENT+open.min(256)*(256-AMBIENT)/256;
        let contact=if north && yy>=t/2 && yy<t/2+4*s {
            128+(yy-t/2)*128/(4*s)
        } else if west || east {
            let d=if west {xx} else {t-1-xx};
            154+d*102/t
        } else {256};
        let mut base=source.map(|v|(v as i32*ambient*contact/65536).min(110) as u8);
        if tile==Tile::Hazard {
            let rim=(yy==0 && dry_n)
                || (yy==t-1 && dry_s)
                || (xx==0 && dry_w)
                || (xx==t-1 && dry_e);
            let frames=std::array::from_fn(|phase| {
                if rim {return tint.map(|v|v.saturating_add(20));}
                let offset=if run.dungeon.pack==Pack::Hellforge {phase as i32*s} else {(phase%2) as i32*s};
                let sample=raw.get(c*t+(xx+offset)%t,r*t+(yy+offset)%t).unwrap_or(source);
                let gain=if run.dungeon.pack==Pack::Hellforge {240} else {170};
                lift(sample.map(|v|(v as i32*gain/256) as u8),tint,if phase%2==0 {12}else{24})
            });
            cv.set(x,y,frames[0]);shimmer.push((x,y,frames));continue;
        }
        if run.dungeon.pack==Pack::Crypt {base[2]=base[2].saturating_add(3);}
        let emission=light[(y/s*lw+x/s) as usize].map(|v|(v as i32*WARMTH/256).min(70) as u8);
        if emission==[0;3] {for dy in 0..step {for dx in 0..step {cv.set(x+dx,y+dy,base);}}continue;}
        let normal=lift(base,emission,256);
        let frames=[normal,lift(base,emission,276),normal,lift(base,emission,236)];
        for dy in 0..step {for dx in 0..step {
            cv.set(x+dx,y+dy,frames[0]);
            if frames.iter().any(|&c|c!=frames[0]) {shimmer.push((x+dx,y+dy,frames));}
        }}
    }}
    }}
    (shimmer, Reveal { open: reveal, w: lw, s })
}
