//! BOB's act-1 objects from the owner's ROM: which placements the port
//! spawns (every coin placement) and which it records as unported, and a
//! played session that collects a yellow coin. The per-frame object words are
//! compared against the native decomp in oracle/tests/objects.rs; this test
//! checks the import and the play integration without the oracle.
use rustario64::{
    content::Act,
    import::{bob, engine, objects, rom::Rom},
    play::{Pad, Session},
    simulation::{
        collision::CollisionWorld,
        game::GameEntry,
        mario::{constants as c, core::SpawnPoint, tick::LevelEntry},
        object::{ObjectList, RespawnInfo, script::Behavior},
    },
};

fn rom() -> Option<Rom> {
    let path = std::env::var_os("RUSTARIO64_ROM")?;
    Some(Rom::open(std::path::Path::new(&path)).unwrap())
}

#[test]
#[ignore = "requires RUSTARIO64_ROM pointing at the supported US ROM"]
fn bob_act_1_spawns_its_coins_and_records_the_rest() {
    let Some(rom) = rom() else {
        eprintln!("RUSTARIO64_ROM is not set; skipping");
        return;
    };
    let imported = bob::import(&rom).unwrap();
    let trig = engine::trig_tables(&rom).unwrap();
    let anims = rustario64::import::animation::mario_animations(&rom).unwrap();
    let world = CollisionWorld::load_area_terrain(&imported.collision).unwrap();
    let camera = imported.visual.as_ref().unwrap().camera.unwrap();
    let script_entry = GameEntry::script_start(&imported.level, &camera).unwrap();
    let content = objects::bob(&rom, &imported.level, Act::new(1).unwrap()).unwrap();
    let scripts = &content.content.scripts;

    // The pinned levels/bob/areas/1/macro.inc.c places 88 macro objects:
    // five macro_yellow_coin_1, three macro_coin_line_horizontal, one
    // macro_coin_ring_horizontal and five macro_coin_ring_vertical_flying.
    let macros = &content.area.macros;
    assert_eq!(macros.len(), 88);
    let count = |b: Behavior| {
        macros
            .iter()
            .filter(|e| e.behavior == scripts.address(b))
            .count()
    };
    assert_eq!(count(Behavior::YellowCoin), 5);
    assert_eq!(count(Behavior::CoinFormation), 9);
    let ported_macros = macros
        .iter()
        .filter(|e| scripts.check(e.behavior).is_ok())
        .count();
    assert_eq!(ported_macros, 14, "only the coin placements are ported");
    let ported_infos: Vec<_> = content
        .area
        .spawn_infos
        .iter()
        .filter(|i| scripts.check(i.behavior_script).is_ok())
        .collect();
    assert_eq!(
        ported_infos.len(),
        1,
        "only the spin airborne warp is ported"
    );
    assert_eq!(
        scripts.behavior_at(ported_infos[0].behavior_script),
        Some(Behavior::SpinAirborneWarp)
    );

    // Entering the level spawns exactly those and records every other
    // placement with its reason.
    let session = Session::new(&world, &trig, &anims, content.level_objects(), script_entry);
    let w = session.world();
    let skipped = &w.area.skipped;
    assert_eq!(
        skipped.len(),
        macros.len() - ported_macros + content.area.spawn_infos.len() - 1
    );
    let behavior_of = |id| scripts.behavior_at(w.objects.slot(id).behavior);
    let level = w.objects.list(ObjectList::Level);
    let yellow = level
        .iter()
        .filter(|id| behavior_of(**id) == Some(Behavior::YellowCoin))
        .count();
    assert_eq!(yellow, 5);
    let spawners = w.objects.list(ObjectList::Spawner);
    assert_eq!(
        spawners
            .iter()
            .filter(|id| behavior_of(**id) == Some(Behavior::CoinFormation))
            .count(),
        9
    );
    assert_eq!(w.objects.list(ObjectList::Player).len(), 1);
    println!(
        "BOB act 1: {} macro objects and {} spawn infos; {} coin placements and the \
         spin airborne warp spawn, {} placements recorded as unported",
        macros.len(),
        content.area.spawn_infos.len(),
        ported_macros,
        skipped.len()
    );

    // Started inside the first yellow coin (the center of a vertical ring,
    // whose nearest coins are 200 units away), Mario collects it as he falls:
    // one coin, the coin object gone and its golden sparkles spawned.
    let coin = macros
        .iter()
        .find(|e| e.behavior == scripts.address(Behavior::YellowCoin))
        .unwrap();
    let entry = GameEntry {
        mario: LevelEntry {
            spawn: SpawnPoint::from_level_script(
                1,
                script_entry.mario.spawn.area_index as u8,
                0,
                coin.pos,
            ),
            ..script_entry.mario
        },
        ..script_entry
    };
    let mut session = Session::new(&world, &trig, &anims, content.level_objects(), entry);
    assert_eq!(session.mario().num_coins, 0);
    let mut sparkles = false;
    for _ in 0..60 {
        assert!(session.step(&Pad::default()), "{:?}", session.stopped());
        let w = session.world();
        sparkles |= w.objects.list(ObjectList::Default).iter().any(|id| {
            scripts.behavior_at(w.objects.slot(*id).behavior) == Some(Behavior::GoldenCoinSparkles)
        });
        if session.mario().num_coins == 1 {
            break;
        }
    }
    assert_eq!(session.mario().num_coins, 1);
    let w = session.world();
    let remaining = w
        .objects
        .list(ObjectList::Level)
        .iter()
        .filter(|id| {
            let o = w.objects.slot(**id);
            scripts.behavior_at(o.behavior) == Some(Behavior::YellowCoin)
                && matches!(o.respawn_info, Some(RespawnInfo::Macro(_)))
                && o.active_flags & c::ACTIVE_FLAG_ACTIVE != 0
        })
        .count();
    // The collected coin deactivates in its own update and unloads at the
    // end of the frame (the ring around it has spawned coins of its own).
    assert_eq!(remaining, 4);
    for _ in 0..3 {
        session.step(&Pad::default());
        let w = session.world();
        sparkles |= w.objects.list(ObjectList::Default).iter().any(|id| {
            scripts.behavior_at(w.objects.slot(*id).behavior) == Some(Behavior::GoldenCoinSparkles)
        });
    }
    assert!(sparkles, "collecting a coin spawns golden sparkles");
    // The HUD's counter has caught up (one step every other frame).
    assert_eq!(session.world().hud.coins, 1);
}
