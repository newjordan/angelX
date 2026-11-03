//! All-rich-media stress: every catalogued kind gets random assets.
//! Uses REAL in-tree code: sniff_media, decode_image_bytes_in_budget,
//! well_formed, premultiply_in_place, fit_inside.
//! cargo run --offline -p angel-glass --example stress-all-media --release
use std::hint::black_box;
use std::time::Instant;
use angel_glass::graphics::{decode_image_bytes_in_budget, fit_inside, sniff_media, MediaKind};
use angel_glass::graphics::{well_formed, premultiply_in_place};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { let mut x=self.0; x^=x>>12; x^=x<<25; x^=x>>27; self.0=x; x.wrapping_mul(0x2545_F491_4F6C_DD1D) }
    fn below(&mut self,n:u64)->u64{self.next()%n}
}

fn rand_rgb(rng:&mut Rng,w:u32,h:u32)->image::RgbImage{
    let mut im=image::RgbImage::new(w,h);
    for p in im.pixels_mut(){
        p.0=[rng.below(256) as u8,rng.below(256) as u8,rng.below(256) as u8];
    }
    im
}

fn encode(fmt:image::ImageFormat, rgb:&image::RgbImage)->Option<Vec<u8>>{
    let dynim=image::DynamicImage::ImageRgb8(rgb.clone());
    let mut cur=std::io::Cursor::new(Vec::new());
    dynim.write_to(&mut cur,fmt).ok()?;
    Some(cur.into_inner())
}

fn bench_decode(name:&str, bytes:&[u8]){
    // sniff timing
    let n=2000u64;
    let t=Instant::now();
    for _ in 0..n{ black_box(sniff_media(black_box(bytes))); }
    let sniff_ns=t.elapsed().as_nanos() as f64/n as f64;
    let kind=sniff_media(bytes);
    // decode timing (fewer iters, decode is ms)
    let reps=10u64;
    let t=Instant::now();
    let mut ok=false; let mut w=0; let mut h=0;
    for _ in 0..reps{
        if let Some(p)=decode_image_bytes_in_budget(black_box(bytes),1,0,0,8192){
            ok=true; w=p.width; h=p.height;
            black_box(p.rgba.len());
        }
    }
    let dec_ms=t.elapsed().as_secs_f64()*1000.0/reps as f64;
    // premultiply timing on decoded pixels (512KB proxy if decode ok)
    let mut prem_ms=0.0;
    if ok{
        if let Some(p)=decode_image_bytes_in_budget(bytes,2,0,0,8192){
            let mut buf=p.rgba.clone();
            let r=20u64;
            let t=Instant::now();
            for _ in 0..r{ let mut b=buf.clone(); premultiply_in_place(black_box(&mut b)); black_box(b.len()); }
            prem_ms=t.elapsed().as_secs_f64()*1000.0/r as f64;
            assert!(well_formed(&p));
        }
    }
    println!("MEDIA {name} sniff={:.1}ns decode={:.3}ms premul={:.3}ms bytes={} px={}x{} ok={} kind={:?}",sniff_ns,dec_ms,prem_ms,bytes.len(),w,h,ok,kind);
}

fn main(){
    let mut rng=Rng(0x9E3779B97F4A7C15);
    println!("ALLMEDIA seed=fixed budget=8192");
    // direct stills at 160x120 random (ICO at 32x32, HDR try 64x64)
    let cases:[(MediaKind,u32,u32);10]=[
        (MediaKind::Png,160,120),(MediaKind::Jpeg,160,120),(MediaKind::Gif,160,120),
        (MediaKind::Bmp,160,120),(MediaKind::WebP,160,120),(MediaKind::Tiff,160,120),
        (MediaKind::Qoi,160,120),(MediaKind::Hdr,64,64),(MediaKind::Ico,32,32),(MediaKind::Pnm,160,120),
    ];
    for (kind,w,h) in cases{
        let rgb=rand_rgb(&mut rng,w,h);
        let fmt=match kind.image_format(){ Some(f)=>f, None=>{println!("MEDIA {:?} SKIP no-format",kind); continue;} };
        match encode(fmt,&rgb){
            Some(bytes)=>{
                // sanity: sniff must name it
                let s=sniff_media(&bytes);
                if s!=Some(kind){ println!("MEDIA {:?} SNIFF-MISMATCH got={:?} bytes={}",kind,s,bytes.len()); }
                bench_decode(kind.extension(),&bytes);
            }
            None=>println!("MEDIA {:?} ENCODE-FAIL",kind),
        }
    }
    // big-frame proxy: PNG+JPEG at 1280x720 (photo path)
    for (kind,w,h) in [(MediaKind::Png,320,200),(MediaKind::Jpeg,640,360)]{
        let rgb=rand_rgb(&mut rng,w,h);
        if let Some(bytes)=encode(kind.image_format().unwrap(),&rgb){
            bench_decode(&format!("{}-{}x{}",kind.extension(),w,h),&bytes);
        }
    }
    // routed headers: must sniff right, never decode
    let routed:Vec<(&str,Vec<u8>)>=vec![
        ("avif",b"....ftypavif....".to_vec()),
        ("heif",b"....ftypheic....".to_vec()),
        ("svg",b"  <!--c--><?xml version=\"1.0\"?><svg width=\"10\"></svg>".to_vec()),
        ("mp4",b"....ftypmp42....".to_vec()),
        ("webm",vec![0x1A,0x45,0xDF,0xA3,0x01,0x02]),
    ];
    for (name,mut hdr) in routed{
        // fix ftyp offset: bytes 4..8 must be b"ftyp"
        if name=="avif"||name=="heif"||name=="mp4"{ hdr[4..8].copy_from_slice(b"ftyp"); }
        let t=Instant::now(); let n=5000u64;
        for _ in 0..n{ black_box(sniff_media(black_box(&hdr))); }
        let ns=t.elapsed().as_nanos() as f64/n as f64;
        let k=sniff_media(&hdr);
        let dec=decode_image_bytes_in_budget(&hdr,9,0,0,8192);
        println!("ROUTED {name} sniff={:.1}ns kind={:?} decode_none={}",ns,k,dec.is_none());
    }
    // premultiply variants at 512x512
    for (vname,alpha) in [("opaque",255u8),("transparent",0u8),("random",128u8)]{
        let w=512u32; let h=512u32;
        let mut buf=vec![0u8;(w*h*4) as usize];
        let mut r2=Rng(12345);
        for px in buf.chunks_exact_mut(4){
            px[0]=r2.below(256) as u8; px[1]=r2.below(256) as u8; px[2]=r2.below(256) as u8;
            px[3]=match alpha{ 255=>255, 0=>0, _=>r2.below(256) as u8 };
        }
        let reps=30u64;
        let t=Instant::now();
        for _ in 0..reps{ let mut b=buf.clone(); premultiply_in_place(&mut b); black_box(b.len()); }
        let ms=t.elapsed().as_secs_f64()*1000.0/reps as f64;
        let mpix=w as f64*h as f64/1e6;
        println!("PREMUL {vname} {w}x{h} {ms:.3}ms {mpix:.2}Mpix {:.1}Mpix/s",mpix/(ms/1000.0));
    }
    // wallpaper fit_inside on random hostile sizes
    let mut r3=Rng(999);
    let t=Instant::now();
    for _ in 0..100000{
        let w=r3.below(9000) as u32; let h=r3.below(9000) as u32;
        black_box(fit_inside(black_box(w),black_box(h),8192));
    }
    println!("FIT fit_inside_100k={:.1}ns/op",t.elapsed().as_nanos() as f64/100000.0);
    // blast video-frame proxy: premultiply 4 stream sizes
    for (w,h) in [(320u32,200u32),(192,160),(160,160),(480,140)]{
        let mut buf=vec![0u8;(w*h*4) as usize];
        for (i,b) in buf.iter_mut().enumerate(){ *b=(i%251) as u8; }
        let reps=50u64; let t=Instant::now();
        for _ in 0..reps{ let mut b=buf.clone(); premultiply_in_place(&mut b); black_box(b.len()); }
        println!("BLAST {w}x{h} premul={:.3}ms",t.elapsed().as_secs_f64()*1000.0/reps as f64);
    }
    println!("ALLMEDIA done");
}
