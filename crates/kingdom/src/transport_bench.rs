//! What each way of moving goods is for: the same heavy flow (a store of stone, a sink that takes all it is given)
//! over the same distance by cart, rollerway, rail and ropeway, and what each costs in goods and in hands.
//! `cargo test -p angel-kingdom --lib transport_bench -- --ignored --nocapture` prints the table; the tests beside it
//! hold the relations the balance rests on.
use super::*;
use crate::rollers::{Dir, Piece};
use crate::works_tests::{build, rich};

/// Units of `good` a minute that get from a store to a sink `dist` tiles away, measured over three minutes after the
/// first two. The store is never empty and the sink is emptied every second.
fn measure(kind: &str, good: &str, dist: i16, fleet: u16) -> Result<f64, String> {
    let c = catalog();
    let mut k = rich();
    for r in c.resources.keys() {
        if r != "fodder" {
            k.stock.insert(r.clone(), 2_000);
        }
    }
    let (store, sink): (u32, u32);
    match kind {
        "cart" => {
            // The camp is the sink: it asks for stone while it is under its level, and is emptied each second.
            store = build(&mut k, "depot", (142 + dist, 96));
            k.buildings.get_mut(&store).unwrap().construction = 0;
            let y = build(&mut k, "carters-yard", (136, 97));
            k.buildings.get_mut(&y).unwrap().construction = 0;
            k.apply(c, Command::Connect(y)).unwrap();
            k.apply(c, Command::Connect(store)).unwrap();
            k.apply(c, Command::Carts { yard: y, carts: fleet }).unwrap();
            sink = 1;
        }
        "rail" => {
            store = build(&mut k, "depot", (150, 96));
            sink = build(&mut k, "depot", (153 + dist, 96));
            for id in [store, sink] {
                k.buildings.get_mut(&id).unwrap().construction = 0;
            }
            k.apply(c, Command::Railway { from: store, to: sink, path: Vec::new(), wagons: fleet, good: Some(good.into()) })?;
        }
        "roller" => {
            store = build(&mut k, "depot", (150, 96));
            let swape = |k: &mut Kingdom, t: Tile| {
                let id = build(k, "swape", t);
                k.apply(catalog(), Command::Face { tile: t, dir: Dir::E }).unwrap();
                k.buildings.get_mut(&id).unwrap().construction = 0;
            };
            swape(&mut k, (153, 96));
            for i in 0..dist {
                k.apply(c, Command::Roller { piece: Piece::Roller, tile: (154 + i, 96), dir: Dir::E }).unwrap();
            }
            swape(&mut k, (154 + dist, 96));
            sink = build(&mut k, "depot", (155 + dist, 96));
            for id in [store, sink] {
                k.buildings.get_mut(&id).unwrap().construction = 0;
            }
        }
        "rope" => {
            store = build(&mut k, "depot", (150, 96));
            sink = build(&mut k, "depot", (153 + dist, 96));
            for id in [store, sink] {
                k.buildings.get_mut(&id).unwrap().construction = 0;
            }
            k.apply(c, Command::Line { kind: LineKind::Rope, from: store, to: sink, path: Vec::new(), vehicles: fleet })?;
        }
        _ => unreachable!(),
    }
    let mut delivered = 0u32;
    for second in 0..300 + 120 {
        k.buildings.get_mut(&store).unwrap().output.insert(good.into(), 400);
        k.advance_second(c);
        let taken = if sink == 1 {
            std::mem::take(k.stock.entry(good.into()).or_default())
        } else {
            let b = k.buildings.get_mut(&sink).unwrap();
            b.input.remove(good).unwrap_or(0) + b.output.remove(good).unwrap_or(0)
        };
        if second >= 120 {
            delivered += taken;
        }
    }
    Ok(f64::from(delivered) / 5.0)
}

/// What laying it costs in goods, and the hands it keeps at work, as the catalogue sets them.
fn price(kind: &str, dist: i16, fleet: u16) -> (String, u32) {
    let c = catalog();
    let show = |s: &Stock| s.iter().map(|(g, n)| format!("{n} {g}")).collect::<Vec<_>>().join(", ");
    let times = |s: &Stock, n: u32| -> Stock { s.iter().map(|(g, q)| (g.clone(), q * n)).collect() };
    match kind {
        "cart" => {
            let v = &c.lines["cart"].vehicle_cost;
            (format!("{} (+{} timber of road)", show(&times(v, u32::from(fleet))), dist), 1)
        }
        "roller" => {
            let p = &c.rollers["roller"].cost;
            let s = &c.plans["swape"].cost;
            let mut all = times(p, dist as u32);
            for (g, n) in s {
                *all.entry(g.clone()).or_default() += n * 2;
            }
            (show(&all), 2)
        }
        "rail" => {
            let t = &c.lines["tram"].tile_cost;
            let v = &c.lines["tram"].vehicle_cost;
            let mut all = times(t, dist as u32);
            for (g, n) in v {
                *all.entry(g.clone()).or_default() += n * u32::from(fleet);
            }
            (show(&all), 0)
        }
        "rope" => {
            let t = &c.lines["rope"].tile_cost;
            let v = &c.lines["rope"].vehicle_cost;
            let mut all = times(t, dist as u32);
            for (g, n) in v {
                *all.entry(g.clone()).or_default() += n * u32::from(fleet);
            }
            (show(&all), 0)
        }
        _ => unreachable!(),
    }
}

#[test]
#[ignore]
fn zz_transport_table() {
    let good = std::env::var("GOOD").unwrap_or_else(|_| "stone".into());
    eprintln!("units a minute of {good} that arrive (a cart takes {} to a load)", Kingdom::cart_lot(catalog(), &good));
    for dist in [6i16, 12, 24, 48] {
        for (kind, fleet) in [("cart", 4u16), ("cart", 8), ("roller", 0), ("rail", 2), ("rail", 4), ("rail", 8), ("rope", 8), ("rope", 24)] {
            if kind == "rope" && dist > 56 {
                continue;
            }
            let (cost, hands) = price(kind, dist, fleet);
            match measure(kind, &good, dist, fleet) {
                Ok(rate) => eprintln!("{dist:>3} tiles  {kind:<6} x{fleet:<2} {rate:>6.1}/min  hands {hands}  costs {cost}"),
                Err(e) => eprintln!("{dist:>3} tiles  {kind:<6} x{fleet:<2}    n/a ({e})"),
            }
        }
    }
}
