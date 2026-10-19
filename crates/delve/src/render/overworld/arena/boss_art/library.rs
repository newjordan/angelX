//! Data-owned display envelopes. Simulation radii and feet remain authoritative.
use super::*;
use crate::rules::Tile;

#[derive(serde::Deserialize)]
struct Policy {
    #[serde(default = "enabled")]
    active: bool,
    v1: [i32; 2],
    v2: [i32; 2],
    study: BTreeMap<String, Vec<[i32; 2]>>,
    grade: BTreeMap<String, [u32; 2]>,
    cell_moss_mix: u32,
}

fn enabled() -> bool { true }

fn available(policy: &Policy) -> bool {
    policy.active || cfg!(feature = "boss-library-review")
}

fn policies() -> &'static BTreeMap<String, Policy> {
    static POLICIES: OnceLock<BTreeMap<String, Policy>> = OnceLock::new();
    POLICIES.get_or_init(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/boss-library-registry.json"
        )))
        .expect("library display registry")
    })
}

pub(crate) fn registered(art: Art) -> bool {
    policies().get(art.id()).is_some_and(available)
}

#[cfg(feature = "boss-library-review")]
thread_local! { static SAMPLE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(feature = "boss-library-review")]
pub(crate) fn review(sample: usize) {
    SAMPLE.with(|s| s.set(sample));
}

fn sample() -> usize {
    #[cfg(feature = "boss-library-review")]
    {
        SAMPLE.with(|s| s.get())
    }
    #[cfg(not(feature = "boss-library-review"))]
    {
        0
    }
}

#[cfg(feature = "testing")]
thread_local! { static HIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

pub(crate) fn hidden() -> bool {
    #[cfg(feature = "testing")]
    { HIDE.with(|h| h.get()) }
    #[cfg(not(feature = "testing"))]
    { false }
}

/// Keep the ground receiver at the same position while exporting its floor.
#[cfg(feature = "testing")]
pub(crate) fn floor_review<T>(draw: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore { fn drop(&mut self) { HIDE.with(|h| h.set(self.0)); } }
    let _restore = Restore(HIDE.with(|h| h.replace(true)));
    draw()
}

thread_local! { static CELLS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

pub(crate) fn in_cells<T>(cells: bool, draw: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            CELLS.with(|c| c.set(self.0));
        }
    }
    let _restore = Restore(CELLS.with(|c| c.replace(cells)));
    draw()
}

fn dimensions(art: Art, modern: bool) -> Option<([i32; 2], usize)> {
    let policy = policies().get(art.id())?;
    if !available(policy) { return None; }
    let kit = if modern { "v2" } else { "v1" };
    let index = sample().min(policy.study[kit].len());
    Some((
        if index == 0 {
            if modern { policy.v2 } else { policy.v1 }
        } else {
            policy.study[kit][index - 1]
        },
        index,
    ))
}

struct Bank {
    poses: [[Arc<Img>; 4]; 3],
    logical: [[Arc<Img>; 4]; 3],
}

/// Reflected light stays inside the delivered alpha. Only the outer two
/// logical pixels receive it, with a stronger upper/left source direction.
fn rim(im: &Img, scale: i32) -> Img {
    let mut out = im.clone();
    for y in 0..im.h { for x in 0..im.w {
        let Some(c) = im.get(x,y) else { continue; };
        let lit = (1..=2*scale).any(|d| im.get(x-d,y).is_none() || im.get(x,y-d).is_none());
        let edge = lit || (1..=scale).any(|d| im.get(x+d,y).is_none() || im.get(x,y+d).is_none());
        if !edge { continue; }
        let value = (54*u32::from(c[0])+183*u32::from(c[1])+19*u32::from(c[2]))/256;
        let target = if lit { 64 } else { 48 };
        if value >= target { continue; }
        // Hue ratios come from this material, rather than a foreign white rim.
        let gain = (target*256/value.max(1)).min(180*256/u32::from(*c.iter().max().unwrap()).max(1));
        out.set(x,y,c.map(|v| (u32::from(v)*gain/256) as u8));
    }}
    out
}

fn variants(rest: Img) -> [Arc<Img>; 4] {
    let mut frames = [
        rest.clone(),
        rest.clone(),
        rest.clone(),
        super::super::marks::heat(&rest),
    ];
    for y in 0..rest.h {
        for x in 0..rest.w {
            if let Some(c) = rest.get(x, y) {
                frames[1].set(x, y, super::super::marks::hit_rgb(c, false));
                frames[2].set(x, y, super::super::marks::hit_rgb(c, true));
            }
        }
    }
    frames.map(Arc::new)
}

fn bank(art: Art, modern: bool, cells: bool) -> Option<Arc<Bank>> {
    let ([w, h], sample) = dimensions(art, modern)?;
    type Banks = Mutex<BTreeMap<(&'static str, bool, bool, usize), Arc<Bank>>>;
    static BANKS: OnceLock<Banks> = OnceLock::new();
    let mut cache = BANKS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap();
    Some(
        cache
            .entry((art.id(), modern, cells, sample))
            .or_insert_with(|| {
                let kit = if modern { "v2" } else { "v1" };
                let suffix = if sample == 0 {
                    String::new()
                } else {
                    format!("-study-{sample}")
                };
                let exposure = policies()[art.id()].grade[kit][usize::from(cells)];
                let load = |pose| {
                    let mut im = png(file(&format!(
                        "{}/library-{kit}{suffix}-{pose}.png",
                        art.id()
                    )));
                    // Cache a bounded material grade, keeping source contours and hue ratios.
                    for y in 0..im.h {
                        for x in 0..im.w {
                            if let Some(c) = im.get(x, y) {
                                let peak = u32::from(*c.iter().max().unwrap()).max(1);
                                let exposure = if cells && peak <= 32 { 256 } else { exposure };
                                let gain = exposure.min(228 * 256 / peak);
                                let mut graded = c.map(|v| (u32::from(v) * gain / 256) as u8);
                                // Terminal reduction needs value without acid-green moss.
                                if cells && exposure > 256 && c[1] > c[0] && c[1] > c[2] {
                                    let mix = policies()[art.id()].cell_moss_mix;
                                    graded = std::array::from_fn(|i| {
                                        ((u32::from(graded[i]) * (256 - mix)
                                            + [228, 210, 172][i] * mix) / 256) as u8
                                    });
                                }
                                im.set(x, y, graded);
                            }
                        }
                    }
                    rim(&im, if modern { 2 } else { 1 })
                };
                let rest = load("rest");
                assert_eq!(
                    (rest.w, rest.h),
                    (w, h),
                    "display recipe/import disagreement"
                );
                let windup = load("windup");
                let mut breath = Img::new(w, h);
                breath.stamp(&resize(&rest, w, h - 1), 0, 1);
                let poses = [variants(rest), variants(breath), variants(windup)];
                let logical = std::array::from_fn(|p| {
                    std::array::from_fn(|hit| {
                        if modern {
                            Arc::new(resize(&poses[p][hit], (w + 1) / 2, (h + 1) / 2))
                        } else {
                            poses[p][hit].clone()
                        }
                    })
                });
                Arc::new(Bank {
                    poses,
                    logical,
                })
            })
            .clone(),
    )
}

pub(super) fn warm(art: Art, modern: bool) {
    if let Some(([w,h],_)) = dimensions(art,modern) {
        let _ = receiver(w,h,if modern {2} else {1});
    }
    for cells in [false, true] {
        let _ = bank(art, modern, cells);
        let _ = bank(art, false, cells);
    }
}

// The receiver covers the projected body and a soft margin, rather than
// painting an opaque disc over the floor. Kernels are cached per display size.
fn receiver(w: i32, h: i32, k: i32) -> Arc<Vec<(i32, i32, u16)>> {
    type Cache = Mutex<BTreeMap<(i32,i32,i32),Arc<Vec<(i32,i32,u16)>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Default::default).lock().unwrap().entry((w,h,k)).or_insert_with(|| {
        let (rx,ry) = (w/2+14*k,h/2+14*k);
        let cy = -h/2+2*k;
        let mut pixels = Vec::new();
        for y in cy-ry..=cy+ry { for x in -rx..=rx {
            let d = x*x*256/(rx*rx)+(y-cy)*(y-cy)*256/(ry*ry);
            let mut gain = if d<256 { 36+220*d*d/65536 } else { 256 };
            let contact = x*x*256/((w/3).max(k).pow(2))+y*y*256/(4*k).pow(2);
            if contact<256 { gain = gain.min(72+184*contact/256); }
            if gain<256 {pixels.push((x,y,gain as u16));}
        }}
        Arc::new(pixels)
    }).clone()
}

fn receivers(run: &Run, modern: bool, mut shade: impl FnMut(i32,i32,u16)) {
    if !super::super::dungeon_art::supported(run) { return; }
    let k = if modern {2} else {1};
    for enemy in &run.enemies {
        let Some(art) = enemy.boss.and_then(|i|run.bosses.get(i as usize)).and_then(|b|Art::for_id(&b.id)) else {continue;};
        let Some(([w,h],_)) = dimensions(art,modern) else {continue;};
        let (fx,fy) = ((enemy.x*super::super::UNIT*k as f32).round() as i32,((enemy.y+super::super::signatures::foot_lift(run,enemy))*super::super::UNIT*k as f32).round() as i32);
        for &(dx,dy,gain) in receiver(w,h,k).iter() {
            let (x,y) = (fx+dx,fy+dy);
            if matches!(run.room().tile(x.div_euclid(16*k),y.div_euclid(16*k)),Tile::Floor|Tile::Door|Tile::Stairs|Tile::Ledge) {shade(x,y,gain);}
        }
    }
}

pub(crate) fn ground(cv: &mut Img, run: &Run, origin: (i32,i32)) {
    receivers(run,false,|x,y,gain| {
        let (x,y)=(x-origin.0,y-origin.1);
        if let Some(c)=cv.get(x,y) {cv.set(x,y,c.map(|v|(u16::from(v)*gain/256) as u8));}
    });
}

pub(crate) fn ground_v2(cv: &mut super::super::art_v2::paint::Canvas, run: &Run) {
    let origin = ((cv.origin.0*2.0) as i32,(cv.origin.1*2.0) as i32);
    receivers(run,true,|x,y,gain| {
        let (x,y)=(x-origin.0,y-origin.1);
        if x>=0 && y>=0 && (x as usize)<cv.w && (y as usize)<cv.h {
            let c=&mut cv.px[y as usize*cv.w+x as usize];
            *c=c.map(|v|(u16::from(v)*gain/256) as u8);
        }
    });
}

fn pose(enemy: &Enemy, tick: u64, art: Art) -> usize {
    let staged = matches!(art, Art::Grinwick | Art::BigRipper) && enemy.signature.active
        && matches!(enemy.stage, crate::rules::signatures::TELL | crate::rules::signatures::INHALE);
    if staged || enemy.ring_warning().is_some() || enemy.arms_warning().is_some() {
        2
    } else {
        ((tick / 18 + u64::from(enemy.id)) % 4 == 1) as usize
    }
}

pub(super) fn figure(
    run: &Run,
    enemy: &Enemy,
    art: Art,
    modern: bool,
    logical: bool,
) -> Option<Arc<Img>> {
    let bank = bank(art, modern, CELLS.with(|c| c.get()))?;
    let hit = if enemy.hurt >= 4 {
        2
    } else if enemy.hurt > 0 {
        1
    } else if enemy.flashing(run.tick) {
        3
    } else {
        0
    };
    let pose = pose(enemy, run.tick, art);
    Some(if logical { &bank.logical } else { &bank.poses }[pose][hit].clone())
}

pub(super) fn paint(
    cv: &mut super::super::art_v2::paint::Canvas,
    run: &Run,
    enemy: &Enemy,
    art: Art,
    cells: bool,
) -> bool {
    let Some(im) = in_cells(cells, || figure(run, enemy, art, true, false)) else {
        return false;
    };
    let x0 = ((enemy.x * super::super::UNIT - cv.origin.0) * 2.0).round() as i32 - im.w / 2;
    let y0 = (((enemy.y + super::super::signatures::foot_lift(run,enemy)) * super::super::UNIT - cv.origin.1) * 2.0).round() as i32 - im.h + 2;
    for y in 0..im.h { for x in 0..im.w {
        let (tx,ty)=(x0+x,y0+y);
        if tx>=0 && ty>=0 && tx<cv.w as i32 && ty<cv.h as i32 {
            if let Some(c)=im.get(x,y) {cv.px[ty as usize*cv.w+tx as usize]=c;}
        }
    }}
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reflected_edges_keep_alpha_and_interior_material() {
        for modern in [false,true] {
            let k=if modern {2} else {1};
            let mut im=Img::new(12*k,12*k);
            for y in 0..im.h {for x in 0..im.w {im.set(x,y,[25,17,14]);}}
            let out=rim(&im,k);
            assert_eq!(out.get(5*k,5*k),im.get(5*k,5*k));
            assert!(out.get(0,0).unwrap()[0]>im.get(0,0).unwrap()[0]);
            assert_eq!(out.get(-1,0),None);
            assert_eq!(out.pixels().count(),im.pixels().count());
        }
    }
    #[test]
    fn receivers_follow_feet_without_darkening_hazards_or_changing_the_run() {
        let mut run=super::super::super::art_v2::testing::boss("iron-tusk-chief");
        if !registered(Art::IronTusk) {return;}
        run.dungeon.rooms[run.at].set_for_test(12,8,Tile::Hazard);
        let original=serde_json::to_vec(&run).unwrap();
        receivers(&run,false,|x,y,_|assert_ne!(run.room().tile(x.div_euclid(16),y.div_euclid(16)),Tile::Hazard));
        let mut before=Img::new(384,224);
        before.rect(0,0,384,224,'2');
        let mut shaded=before.clone();ground(&mut shaded,&run,(0,0));
        assert_ne!(before,shaded);
        assert_eq!(original,serde_json::to_vec(&run).unwrap());
        run.enemies[0].x=-100.0;run.enemies[0].y=-100.0;
        let mut offscreen=before.clone();ground(&mut offscreen,&run,(0,0));
        assert_eq!(before,offscreen);
        assert!(Arc::ptr_eq(&receiver(46,54,1),&receiver(46,54,1)));
    }
    #[test]
    fn labels_follow_the_actual_projection_even_when_the_preference_falls_back() {
        let mut run=super::super::super::art_v2::testing::boss("iron-tusk-chief");
        let draw=|modern| {
            let mut img=Img::new(384,224);
            super::super::super::wanted::nameplates(&mut img,&run,(0,0),modern);
            img.content_hash()
        };
        crate::settings::set(crate::settings::Settings {art_v2:false,feel_ledger:false,open_floors:false});
        let native=draw(false);
        crate::settings::set(crate::settings::Settings {art_v2:true,feel_ledger:false,open_floors:false});
        assert_eq!(native,draw(false));
        if !registered(Art::IronTusk) {
            assert_eq!(native,draw(true), "retained body uses the same projection in both kits");
            assert!(figure(&run,&run.enemies[0],Art::IronTusk,false,true).is_none());
            crate::settings::set(Default::default());
            return;
        }
        assert_ne!(native,draw(true));
        crate::settings::set(Default::default());
        run.enemies[0].y=1.0;
        let enemy=&run.enemies[0];
        let im=figure(&run,enemy,Art::IronTusk,false,true).unwrap();
        let mut bars=Img::new(384,224);
        super::super::super::marks::enemy_bar(&mut bars,&run,enemy,192,8,&im,(0,0));
        assert!(bars.get(192,17).is_some(), "clipped art must retain its health bar");
    }
    #[test]
    fn warning_ink_wins_reduction_without_matching_library_material() {
        let inks = super::super::warning_inks();
        for (id, policy) in policies() {
            let art = Art::for_id(id).unwrap();
            if !available(policy) { continue; }
            let bank = bank(art, false, true).unwrap();
            for pose in &bank.poses { for im in pose {
                for y in 0..im.h { for x in 0..im.w {
                    if let Some(c) = im.get(x,y) { assert!(!inks.contains(&c), "{id}: material cannot claim warning priority"); }
                }}
            }}
        }
        let scenery = Img::black(384,224);
        let mut figures = Img::new(384,224);
        let body = [228,210,175];
        for y in 0..4 { for x in 0..4 { figures.set(x,y,body); }}
        // Exercise explicit priority with a dim warning. Bright flight cores
        // already beat this body by luminance without a protected bank.
        let luma=|c:[u8;3]|299*u32::from(c[0])+587*u32::from(c[1])+114*u32::from(c[2]);
        let warning = *inks.iter().filter(|&&c| !crate::render::overworld::ink::is_signal(c) && luma(c)<luma(body)).max().unwrap();
        figures.set(1,1,warning);
        let field = super::super::super::Field::fit(96,56);
        let mut ordinary = Img::black(96,56);
        let mut alerted = ordinary.clone();
        field.blit_ranked(&scenery,&figures,&mut ordinary,None,None);
        field.blit_ranked(&scenery,&figures,&mut alerted,None,Some(inks));
        assert_eq!(ordinary.get(0,0),Some(body));
        assert_eq!(alerted.get(0,0),Some(warning));
    }
    #[test]
    fn library_display_is_data_owned_cached_and_independent_of_hit_radius() {
        for (id, policy) in policies() {
            let art = Art::for_id(id).unwrap();
            if !available(policy) {
                assert!(!registered(art));
                assert!(dimensions(art, false).is_none());
                continue;
            }
            let mut run = super::super::super::art_v2::testing::boss(id);
            let original = run.clone();
            for modern in [false, true] {
                let a = figure(&run, &run.enemies[0], art, modern, false).unwrap();
                let ([w, h], _) = dimensions(art, modern).unwrap();
                assert_eq!((a.w, a.h), (w, h));
                assert!(Arc::ptr_eq(
                    &a,
                    &figure(&run, &run.enemies[0], art, modern, false).unwrap()
                ));
                let index = run.enemies[0].boss.unwrap() as usize;
                run.bosses[index].radius *= 2.0;
                assert!(Arc::ptr_eq(
                    &a,
                    &figure(&run, &run.enemies[0], art, modern, false).unwrap()
                ));
                run = original.clone();
            }
            let before = figure(&run, &run.enemies[0], art, true, false).unwrap();
            run.enemies[0].hp = run.enemies[0].max_hp / 4;
            run.enemies[0].dread = 1;
            assert_eq!(pose(&run.enemies[0], run.tick, art), 2);
            assert_ne!(
                before.content_hash(),
                figure(&run, &run.enemies[0], art, true, false)
                    .unwrap()
                    .content_hash()
            );
            run.enemies[0].dread = crate::rules::desperate::RING_WARN;
            assert_ne!(pose(&run.enemies[0], run.tick, art), 2);
        }
    }
}
