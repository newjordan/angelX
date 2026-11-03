    use super::*;
    use angel_realm_host::together_realm::{self as realm, Realm};
    use shooter::merchant::{Purchase, Shop};
    use shooter::squire_company::{Company, Kind, Roster, Squire};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    struct Fixture {
        app: App,
        dir: PathBuf,
    }

    impl Fixture {
        fn new(company: Company) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let dir = std::env::temp_dir().join(format!(
                "angel-squire-host-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
            let mut realm = Realm::beside(Some(&dir.join("island.json")));
            realm.home.squire_company = company;
            realm.home.goblins = 4;
            realm.treasury.add(realm::Spoil::Gold, 10_000);
            let mut run =
                shooter::Run::at_home(61, 1, None, realm.home.clone(), realm.treasury.clone());
            run.players.get_mut(&1).unwrap().name = "Ada".into();
            run.join_seat(2, "Bryn");
            run.arrive(
                shooter::world::room_of(&run.dungeon, shooter::RoomKind::SquireBarracks).unwrap(),
                (12.0, 10.0),
            );
            // The realm's unrelated surface is authoritative, not this older
            // run copy. Persisting a company must not replace the whole home.
            run.home.goblins = 0;
            realm.save().unwrap();
            app.dungeon.realm = Some(realm);
            app.dungeon.save_path = Some(dir.join("session.json"));
            app.dungeon.shooter.set(Some(run));
            assert!(app.settle_home(), "{}", app.dungeon.notice);
            Self { app, dir }
        }

        fn path(&self) -> PathBuf {
            self.dir.join("island.realm.json")
        }

        fn saved(&self) -> Realm {
            serde_json::from_slice(&std::fs::read(self.path()).unwrap()).unwrap()
        }

        fn gold(&self) -> u32 {
            self.app
                .dungeon
                .realm
                .as_ref()
                .unwrap()
                .treasury
                .get(realm::Spoil::Gold)
        }

        fn request(&mut self, id: u32, action: usize) -> u32 {
            let run = self.app.dungeon.shooter.edit().unwrap();
            let p = quote(run, id, action);
            let gold = run.peddle_price(p).unwrap();
            run.peddled.push(p);
            gold
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn stand(run: &mut shooter::Run, id: u32, (x, y, w, h): (i32, i32, i32, i32)) {
        let hero = run.players.get_mut(&id).unwrap();
        hero.x = (x as f32 + w as f32 / 2.0) * shooter::TILE_UNITS;
        hero.y = (y as f32 + h as f32 / 2.0) * shooter::TILE_UNITS;
    }

    fn press(run: &mut shooter::Run, id: u32) {
        run.tick_squire_company(&BTreeMap::new());
        run.tick_squire_company(&BTreeMap::from([(
            id,
            shooter::Input {
                interact: true,
                ..Default::default()
            },
        )]));
    }

    fn quote(run: &mut shooter::Run, id: u32, action: usize) -> Purchase {
        let plate = match action {
            0..=4 => shooter::squire_company::company::RECRUIT_PLATES[action],
            5 => shooter::squire_company::company::TRAIN_PLATE,
            6..=8 => shooter::squire_company::company::UPGRADE_PLATES[action - 6],
            _ => panic!("not a paid company action"),
        };
        stand(run, id, plate);
        press(run, id);
        let p = run.peddled.pop().expect("the Yard quoted this order");
        assert_eq!(p.shop, Shop::Squires);
        p
    }

    fn owned() -> Company {
        let squire = Squire {
            id: 1,
            name: "Ash".into(),
            kind: Kind::Ninja,
            level: 1,
            trained_on: None,
        };
        Company {
            next_id: 1,
            rosters: BTreeMap::from([(
                "Ada".into(),
                Roster {
                    squires: vec![squire],
                    active: Some(1),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        }
    }

    #[test]
    fn hires_are_saved_by_name_and_duplicate_quotes_are_not_charged() {
        let mut f = Fixture::new(Company::default());
        let before = f.gold();
        let run = f.app.dungeon.shooter.edit().unwrap();
        let ada = quote(run, 1, 0);
        let bryn = quote(run, 2, 0);
        let cost = run.peddle_price(ada).unwrap() + run.peddle_price(bryn).unwrap();
        run.peddled.extend([ada, ada, bryn]);
        assert!(f.app.settle_home());
        let saved = f.saved();
        assert_eq!(f.gold(), before - cost);
        assert_eq!(saved.treasury.get(realm::Spoil::Gold), before - cost);
        assert_eq!(saved.home.squire_company.rosters["Ada"].squires.len(), 1);
        assert_eq!(saved.home.squire_company.rosters["Bryn"].squires.len(), 1);
        assert_ne!(
            saved.home.squire_company.rosters["Ada"].active,
            saved.home.squire_company.rosters["Bryn"].active
        );
        assert_eq!(saved.home.goblins, 4);
        assert_eq!(
            f.app.dungeon.shooter.as_ref().unwrap().home.squire_company,
            saved.home.squire_company
        );
        assert!(f.app.settle_home());
        assert_eq!(
            f.gold(),
            before - cost,
            "settling again never charges twice"
        );
    }

    #[test]
    fn training_and_every_upgrade_are_paid_and_saved_before_delivery() {
        let mut f = Fixture::new(owned());
        for action in [5, 6, 7, 8] {
            let before = f.gold();
            let cost = f.request(1, action);
            assert!(f.app.settle_home());
            let saved = f.saved();
            assert_eq!(f.gold(), before - cost);
            assert_eq!(saved.treasury.get(realm::Spoil::Gold), before - cost);
            assert_eq!(
                saved.home.squire_company,
                f.app.dungeon.shooter.as_ref().unwrap().home.squire_company
            );
        }
        let saved = f.saved();
        let company = &saved.home.squire_company;
        assert_eq!((company.board, company.hall, company.yard), (1, 1, 1));
        assert_eq!(company.rosters["Ada"].squires[0].level, 2);
        assert_eq!(
            company.rosters["Ada"].squires[0].trained_on,
            Some(company.delves)
        );
    }

    #[test]
    fn free_selection_and_cadence_are_saved_without_other_home_replacement() {
        let mut f = Fixture::new(owned());
        let before = f.gold();
        let run = f.app.dungeon.shooter.edit().unwrap();
        stand(run, 1, shooter::squire_company::company::SELECT_PLATE);
        press(run, 1);
        assert_eq!(run.home.squire_company.rosters["Ada"].active, None);
        run.home.squire_company.delves += 1;
        assert!(run.peddled.is_empty());
        assert!(f.app.settle_home());
        let saved = f.saved();
        assert_eq!(saved.home.squire_company.rosters["Ada"].active, None);
        assert_eq!(saved.home.squire_company.delves, 1);
        assert_eq!(saved.home.goblins, 4);
        assert_eq!(f.gold(), before);
        assert!(f.app.settle_home());
        assert_eq!(
            f.saved().home.squire_company.delves,
            1,
            "cadence is not double-counted"
        );
    }

    #[test]
    fn stale_and_conflicting_orders_do_not_spend_gold() {
        let mut f = Fixture::new(Company::default());
        let before = f.gold();
        f.request(1, 0);
        f.app.dungeon.shooter.edit().unwrap().home.squire_company.delves += 1;
        assert!(f.app.settle_home());
        assert_eq!(f.gold(), before);
        assert!(f.saved().home.squire_company.rosters.is_empty());
        let run = f.app.dungeon.shooter.edit().unwrap();
        let a = quote(run, 1, 6);
        let b = quote(run, 2, 6);
        let cost = run.peddle_price(a).unwrap();
        run.peddled.extend([a, b]);
        assert!(f.app.settle_home());
        assert_eq!(f.gold(), before - cost);
        assert_eq!(f.saved().home.squire_company.board, 1);
    }

    #[test]
    fn failed_save_rolls_back_each_paid_action_and_retry_delivers_once() {
        for action in [0, 5, 6, 7, 8] {
            let mut f = Fixture::new(owned());
            let before = f.gold();
            let company = f.saved().home.squire_company;
            let cost = f.request(1, action);
            let bytes = std::fs::read(f.path()).unwrap();
            std::fs::remove_file(f.path()).unwrap();
            std::fs::create_dir(f.path()).unwrap(); // atomic rename must fail
            assert!(!f.app.settle_home(), "action {action}");
            assert_eq!(f.gold(), before, "no gold lost for action {action}");
            assert_eq!(f.app.dungeon.realm.as_ref().unwrap().home.squire_company, company);
            let run = f.app.dungeon.shooter.as_ref().unwrap();
            assert_eq!(
                run.home.squire_company, company,
                "no unpaid delivery for action {action}"
            );
            assert_eq!(run.peddled.len(), 1, "retry retains the order");
            assert!(!run.squire_company_reserved);
            assert_eq!(run.squire_pending_delves, 0);
            std::fs::remove_dir(f.path()).unwrap();
            std::fs::write(f.path(), bytes).unwrap();
            assert!(f.app.settle_home());
            assert_eq!(f.gold(), before - cost);
            assert_ne!(f.saved().home.squire_company, company);
            assert_eq!(
                f.saved().home.squire_company,
                f.app.dungeon.shooter.as_ref().unwrap().home.squire_company
            );
            assert!(f.app.settle_home());
            assert_eq!(f.gold(), before - cost);
        }
    }

    #[test]
    fn insufficient_treasury_never_delivers_a_hire_lesson_or_upgrade() {
        for action in [0, 5, 6, 7, 8] {
            let mut f = Fixture::new(owned());
            let company = f.saved().home.squire_company;
            let realm = f.app.dungeon.realm.as_mut().unwrap();
            realm.treasury = Default::default();
            realm.save().unwrap();
            // The run still sees its old, rich treasury. Only the realm's
            // live balance can authorize payment.
            f.request(1, action);
            assert!(f.app.settle_home());
            assert_eq!(f.gold(), 0);
            assert_eq!(f.saved().home.squire_company, company);
            let run = f.app.dungeon.shooter.as_ref().unwrap();
            assert_eq!(run.home.squire_company, company);
            assert!(run.peddled.is_empty());
        }
    }

    #[test]
    fn stale_training_selection_and_invalid_customers_are_not_charged() {
        for invalid in 0..4 {
            let mut f = Fixture::new(owned());
            let before = f.gold();
            f.request(1, 5);
            let run = f.app.dungeon.shooter.edit().unwrap();
            match invalid {
                0 => run.home.squire_company.rosters.get_mut("Ada").unwrap().active = None,
                1 => run.players.get_mut(&1).unwrap().hp = 0,
                2 => stand(run, 1, (20, 12, 1, 1)),
                3 => run.players.get_mut(&1).unwrap().name = "Not Ada".into(),
                _ => unreachable!(),
            }
            assert!(f.app.settle_home());
            assert_eq!(f.gold(), before);
            assert_eq!(f.saved().home.squire_company.rosters["Ada"].squires[0].level, 1);
            assert!(f.app.dungeon.shooter.as_ref().unwrap().peddled.is_empty());
        }
    }

    #[test]
    fn failed_free_selection_and_cadence_save_preserves_live_edits_for_retry() {
        let mut f = Fixture::new(owned());
        let before = f.gold();
        let company = f.saved().home.squire_company;
        let run = f.app.dungeon.shooter.edit().unwrap();
        stand(run, 1, shooter::squire_company::company::SELECT_PLATE);
        press(run, 1);
        run.home.squire_company.delves += 1;
        let edited = run.home.squire_company.clone();
        let bytes = std::fs::read(f.path()).unwrap();
        std::fs::remove_file(f.path()).unwrap();
        std::fs::create_dir(f.path()).unwrap();
        assert!(!f.app.settle_home());
        assert_eq!(f.gold(), before);
        assert_eq!(f.app.dungeon.realm.as_ref().unwrap().home.squire_company, company);
        assert_eq!(f.app.dungeon.shooter.as_ref().unwrap().home.squire_company, edited);
        std::fs::remove_dir(f.path()).unwrap();
        std::fs::write(f.path(), bytes).unwrap();
        assert!(f.app.settle_home());
        assert_eq!(f.saved().home.squire_company, edited);
        assert_eq!(f.gold(), before);
    }
