use super::*;

#[test]
fn a_run_finds_thirty_things_out_in_the_open_from_its_seed() {
    let geo = kingdom::geography(&[]);
    let camps = super::super::dressing::get();
    for seed in [1u64, 41, 7_000_003, u64::MAX] {
        let all = finds(seed);
        assert_eq!(all.len(), COUNT, "seed {seed}: {all:?}");
        assert_eq!(COUNT, 30);
        assert_eq!(*all, place(seed), "the same seed, the same finds");
        for (i, f) in all.iter().enumerate() {
            assert_eq!(geo.room.tile(f.at.0, f.at.1), Tile::Floor, "{f:?} stands in the way");
            assert!(!kingdom::is_road(&geo, f.at), "{f:?} on a road");
            assert!(!in_set_piece(f.at.0, f.at.1), "{f:?} in a set piece");
            assert_eq!(biome::at(f.at.0, f.at.1), f.biome);
            for g in &all[i + 1..] {
                assert!(chebyshev(f.at, g.at) >= APART, "{f:?} crowds {g:?}");
            }
        }
        let wastes = all.iter().filter(|f| f.biome == Biome::Wastes).count();
        assert!(wastes >= 5, "the Wastes hold the most: {wastes}");
        // The new kinds are out there, and most finds stand in the clearing
        // of a wild camp at the end of a footpath.
        for kind in [Kind::LostPack, Kind::Hermit, Kind::Well] {
            assert!(all.iter().any(|f| f.kind == kind), "seed {seed}: no {kind:?}");
        }
        let walked = all.iter().filter(|f| camps.trail(f.at.0, f.at.1)).count();
        assert!(walked * 10 >= all.len() * 8, "seed {seed}: only {walked} of {} are at a footpath's end", all.len());
        // Every biome has its share, the quieter ones included.
        for (home, share, _) in SHARE.iter().take(8) {
            assert!(all.iter().filter(|f| f.biome == *home).count() >= *share, "seed {seed}: {home:?} short");
        }
    }
    assert_ne!(*finds(1), *finds(2), "another run, other finds");
}
