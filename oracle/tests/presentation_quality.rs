//! Read-only high-detail drawing must preserve all captured gameplay words.
//! Keep this check in the oracle suite: its native math symbols must never
//! be linked into renderer tests, which use host atan2f for their cameras.
use rustario64::{
    content::Act,
    import::{animation, bob, engine, mario, objects, rom::Rom},
    play::{Pad, Session},
    presentation::mario::MarioDrawer,
    simulation::{collision::CollisionWorld, game::GameEntry},
};
use rustario64_oracle::camera_trace::capture_game;

#[test]
#[ignore = "requires RUSTARIO64_ROM; full-state presentation independence"]
fn high_detail_drawing_preserves_every_captured_gameplay_word() {
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let source = mario::import(&rom).unwrap();
    let trig = engine::trig_tables(&rom).unwrap();
    let anims = animation::mario_animations(&rom).unwrap();
    let imported = bob::import(&rom).unwrap();
    let world = CollisionWorld::load_area_terrain(&imported.collision).unwrap();
    let camera = imported.visual.as_ref().unwrap().camera.unwrap();
    let entry = GameEntry::script_start(&imported.level, &camera).unwrap();
    let content = objects::bob(&rom, &imported.level, Act::new(1).unwrap()).unwrap();
    let mut session = Session::new(&world, &trig, &anims, content.level_objects(), entry);
    let mut drawer = MarioDrawer::new(&source, &trig, &anims);
    let mut detailed_frames = 0;
    for tick in 0..240 {
        assert!(session.step(&Pad {
            up: tick < 180,
            a: tick == 140,
            b: tick == 200,
            ..Pad::default()
        }));
        let before = capture_game(session.game());
        let pose = session.mario_pose();
        let distance = [100, 1000, 2000][(tick / 32) % 3];
        drawer.update(&pose, Some(distance)).unwrap();
        let original = drawer.frame(1.0, false).unwrap();
        let original_count: usize = original.vertices.iter().map(Vec::len).sum();
        // This is the high-detail draw selection, independent of the original
        // simulation render pass and its LOD-dependent hand-position callbacks.
        drawer.reset();
        drawer.update(&pose, Some(0)).unwrap();
        let enhanced = drawer.frame(1.0, false).unwrap();
        let enhanced_count: usize = enhanced.vertices.iter().map(Vec::len).sum();
        assert!(enhanced_count >= original_count);
        detailed_frames += usize::from(enhanced_count > original_count);
        for alpha in [0.0, 0.25, 0.5, 1.0] {
            assert!(drawer.frame(alpha, true).is_some());
        }
        assert_eq!(
            capture_game(session.game()),
            before,
            "graphics changed frame {tick}"
        );
    }
    assert!(detailed_frames > 0);
    println!("240 frames preserve all captured words while changing presentation LOD");
}
