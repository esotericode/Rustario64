//! Shared boss-grab components. These authored actors never run a boss script.
//! Original native actions/interaction and Rust evolve from the same fixture;
//! every existing Mario/object snapshot word is compared after every call.
#[path = "support/playground.rs"]
mod playground;
use playground::{Builder, authored_animations, computed_tables, world};
use rustario64::{
    content::animation::{MarioAnimations, ObjectAnimations},
    simulation::{
        collision::CollisionWorld,
        controller::A_BUTTON,
        mario::{
            MarioState, StepWorld, animation::update_animation_frame, automatic, constants::*,
            interaction, moving, object as mario_object, stationary, tick::LevelObjects,
        },
        math::TrigTables,
        object::{
            ObjectId, held,
            render::ObjectModels,
            script::{Behavior, authored_scripts},
            spawn::spawn_object,
        },
    },
};
use rustario64_oracle::{
    Oracle, TickSetup,
    camera_trace::first_difference,
    grab::{GrabCall as Call, GrabFixture},
    object_trace::NativeObjects,
    tick_trace::{capture, rust_begin},
};

struct Fixture<'a> {
    m: MarioState,
    w: StepWorld<'a>,
    actor: ObjectId,
    anchor: ObjectId,
}
impl<'a> Fixture<'a> {
    fn begin(
        oracle: &Oracle,
        collision: &'a CollisionWorld,
        trig: &'a TrigTables,
        anims: &'a MarioAnimations,
        objects: &LevelObjects<'a>,
        setup: &TickSetup,
        f: GrabFixture,
    ) -> Self {
        oracle.tick_begin(setup);
        oracle.grab_fixture(&f);
        let (mut m, mut w) = rust_begin(collision, trig, anims, objects, setup);
        let parent = w.objects.mario.unwrap();
        let behavior = w.behaviors.address(Behavior::Bobomb);
        let actor = spawn_object(&mut m, &mut w, parent, MODEL_NONE, behavior);
        let anchor = spawn_object(&mut m, &mut w, actor, MODEL_NONE, behavior);
        let carried = spawn_object(&mut m, &mut w, parent, MODEL_NONE, behavior);
        let o = w.objects.slot_mut(actor);
        o.raw.set_u32(O_FLAGS, OBJ_FLAG_HOLDABLE);
        o.raw.set_u32(O_INTERACT_TYPE, INTERACT_GRABBABLE);
        o.raw.set_u32(O_INTERACTION_SUBTYPE, f.subtype);
        o.raw.set_u32(O_INTERACT_STATUS, f.object_status);
        o.raw.set_s32(O_MOVE_ANGLE_YAW, f.object_yaw);
        o.raw.set_s32(O_KING_BOBOMB_UNK88, f.anchor_state);
        o.active_flags = f.parent_active as i16;
        o.hitbox_radius = 200.0;
        o.hitbox_height = 300.0;
        for (i, value) in f.object_pos.into_iter().enumerate() {
            o.raw.set_f32(O_POS_X + i, value);
        }
        let o = w.objects.slot_mut(anchor);
        for (i, value) in f.anchor_pos.into_iter().enumerate() {
            o.raw.set_f32(O_POS_X + i, value);
        }
        o.raw.set_f32(O_GRAPH_Y_OFFSET, 35.0);
        o.raw.set_s32(O_MOVE_ANGLE_PITCH, 0x12345);
        o.raw.set_s32(O_MOVE_ANGLE_YAW, -0x12345);
        o.raw.set_s32(O_MOVE_ANGLE_ROLL, 0x34567);
        let o = w.objects.slot_mut(carried);
        o.raw.set_u32(O_FLAGS, OBJ_FLAG_HOLDABLE);
        o.raw.set_s32(O_HELD_STATE, HELD_HELD);
        m.action = f.action;
        m.action_arg = f.action_arg;
        m.invinc_timer = f.invinc_timer as i16;
        m.face_angle[1] = f.mario_yaw as i16;
        m.forward_vel = f.forward_vel;
        m.vel[1] = f.vel_y;
        m.obj.gfx.pos = f.gfx_pos;
        m.obj.raw.set_u32(O_INTERACT_STATUS, f.mario_status);
        m.interact_obj = Some(actor);
        m.used_obj = Some(actor);
        m.held_obj = match f.holding {
            1 => Some(carried),
            2 => Some(actor),
            _ => None,
        };
        if f.holding == 2 {
            w.objects
                .slot_mut(actor)
                .raw
                .set_s32(O_HELD_STATE, HELD_HELD);
        }
        w.objects.current = Some(parent);
        Self {
            m,
            w,
            actor,
            anchor,
        }
    }

    fn call(&mut self, oracle: &Oracle, call: Call) -> i32 {
        let native = oracle.grab_call(call);
        let Self {
            m,
            w,
            actor,
            anchor,
        } = self;
        let mut result = 0;
        match call {
            Call::Interactions => {
                m.collided_obj_interact_types = INTERACT_GRABBABLE;
                m.obj.num_collided_objs = 1;
                m.obj.collided_objs[0] = Some(*actor);
                interaction::mario_process_interactions(m, w);
            }
            Call::AutomaticAction => result = automatic::mario_execute_automatic_action(m, w),
            Call::AcknowledgeGrab => {
                result = i32::from(held::cur_obj_check_grabbed_mario(
                    w.objects.slot_mut(*actor),
                ))
            }
            Call::Anchor {
                forward_vel,
                vel_y,
                status,
            } => held::common_anchor_mario_behavior(m, w, *anchor, forward_vel, vel_y, status),
            Call::Escape { stick_mag, pressed } => {
                w.controller.stick_mag = stick_mag;
                w.controller.button_pressed = pressed;
                result = i32::from(held::player_performed_grab_escape_action(w));
            }
            Call::Release {
                forward_vel,
                vel_y,
                action,
            } => held::cur_obj_get_thrown_or_placed(m, w, *actor, forward_vel, vel_y, action),
            Call::ObjectAction => result = mario_object::mario_execute_object_action(m, w),
            Call::StationaryAction => result = stationary::mario_execute_stationary_action(m, w),
            Call::MovingAction => result = moving::mario_execute_moving_action(m, w),
            Call::AdvanceAnimation => {
                w.area_update_counter = w.area_update_counter.wrapping_add(1);
                update_animation_frame(&mut m.obj, w);
            }
            Call::Animation(index) => {
                result = i32::from(
                    rustario64::simulation::mario::animation::set_mario_animation(m, w, index),
                )
            }
            Call::AnchorState(state) => w
                .objects
                .slot_mut(*actor)
                .raw
                .set_s32(O_KING_BOBOMB_UNK88, state),
            Call::Intent {
                input,
                magnitude,
                yaw,
            } => {
                m.input = input;
                m.intended_mag = magnitude;
                m.intended_yaw = yaw;
            }
        }
        w.objects.current = w.objects.mario;
        assert_eq!(result, native, "return from {call:?}");
        self.compare(oracle, &format!("after {call:?}"));
        result
    }

    fn compare(&self, oracle: &Oracle, label: &str) {
        let native = oracle.tick_snapshot();
        let rust = capture(&self.m, &self.w);
        if let Some(diff) = first_difference(&native, &rust) {
            panic!("{label}: {diff}");
        }
    }
}

fn seed() -> GrabFixture {
    GrabFixture {
        action: ACT_IDLE,
        subtype: INT_SUBTYPE_GRABS_MARIO,
        parent_active: i32::from(ACTIVE_FLAG_ACTIVE),
        object_pos: [0.0, 0.0, -100.0],
        anchor_pos: [25.0, 100.0, -50.0],
        gfx_pos: [35.0, 250.0, -65.0],
        ..Default::default()
    }
}

fn terrain() -> Vec<i16> {
    let mut b = Builder::default();
    b.flat(SURFACE_DEFAULT, [-7000, 7000], [-7000, 7000], 0, true);
    b.stream()
}

#[test]
fn grab_angle_invulnerability_and_drop_boundaries_match() {
    let data = terrain();
    let collision = world(&data);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_000B);
    let scripts = authored_scripts();
    let models = ObjectModels::default();
    let object_anims = ObjectAnimations::default();
    let objects = LevelObjects::mario_only(&scripts, &models, &object_anims);
    let oracle = Oracle::load(&data);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    oracle.tick_set_objects(&NativeObjects::new(&objects));
    let setup = TickSetup::from_level_script(LEVEL_BOB, 1, 0, [0; 3], 0, CAMERA_MODE_RADIAL as u8);
    let mut grabbed = 0;
    let mut not_grabbed = 0;
    let mut calls = 0;
    for action in [
        ACT_IDLE,
        ACT_JUMP,
        ACT_PUNCHING,
        ACT_BACKWARD_AIR_KB,
        ACT_WALKING,
    ] {
        for invinc_timer in [0, 1, 20] {
            for yaw in [-0x2AAB, -0x2AAA, -0x2AA9, 0, 0x2AA9, 0x2AAA, 0x2AAB, 0x8000] {
                for holding in [0, 1] {
                    let f = GrabFixture {
                        action,
                        invinc_timer,
                        object_yaw: yaw,
                        holding,
                        ..seed()
                    };
                    let mut state =
                        Fixture::begin(&oracle, &collision, &trig, &anims, &objects, &setup, f);
                    state.compare(&oracle, "initial fixture");
                    state.call(&oracle, Call::Interactions);
                    calls += 1;
                    if state.m.action == ACT_GRABBED {
                        grabbed += 1;
                        assert!(state.m.held_obj.is_none());
                        assert_eq!(state.call(&oracle, Call::AcknowledgeGrab), 1);
                        calls += 1;
                        state.call(&oracle, Call::AutomaticAction);
                        calls += 1;
                    } else {
                        not_grabbed += 1;
                    }
                }
            }
        }
    }
    assert!(grabbed > 0 && not_grabbed > 0);
    println!("grab boundaries: {calls} exact calls; {grabbed} grabbed, {not_grabbed} rejected");
}

#[test]
fn animated_anchor_throw_and_escape_release_match() {
    let data = terrain();
    let collision = world(&data);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_000B);
    let scripts = authored_scripts();
    let models = ObjectModels::default();
    let object_anims = ObjectAnimations::default();
    let objects = LevelObjects::mario_only(&scripts, &models, &object_anims);
    let oracle = Oracle::load(&data);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    oracle.tick_set_objects(&NativeObjects::new(&objects));
    let setup = TickSetup::from_level_script(LEVEL_BOB, 1, 0, [0; 3], 0, CAMERA_MODE_RADIAL as u8);
    let mut calls = 0;
    for anchor_state in [-1, 0, 1, 2, 3, 4] {
        for parent_active in [
            0,
            i32::from(ACTIVE_FLAG_ACTIVE),
            i32::from(ACTIVE_FLAG_FAR_AWAY),
        ] {
            for (forward_vel, status) in [
                (50.0, INT_STATUS_MARIO_UNK6),
                (50.0, 0),
                (-50.0, 0),
                (-0.0, 0),
            ] {
                let f = GrabFixture {
                    action: ACT_GRABBED,
                    object_yaw: 0x12345,
                    anchor_state,
                    parent_active,
                    ..seed()
                };
                let mut state =
                    Fixture::begin(&oracle, &collision, &trig, &anims, &objects, &setup, f);
                // An animated anchor locates Mario before the parent releases him.
                state.call(&oracle, Call::AnchorState(1));
                state.call(
                    &oracle,
                    Call::Anchor {
                        forward_vel,
                        vel_y: 50.0,
                        status,
                    },
                );
                state.call(&oracle, Call::AnchorState(anchor_state));
                state.call(
                    &oracle,
                    Call::Anchor {
                        forward_vel,
                        vel_y: 50.0,
                        status,
                    },
                );
                state.call(&oracle, Call::AutomaticAction);
                calls += 5;
                if matches!(anchor_state, 2 | 3) {
                    assert!(matches!(
                        state.m.action,
                        ACT_THROWN_FORWARD | ACT_THROWN_BACKWARD
                    ));
                    assert_eq!(state.m.pos, [25.0, 135.0, -50.0]);
                }
            }
        }
    }
    println!("anchor/release: {calls} exact calls");
}

#[test]
fn grab_escape_hysteresis_persists_across_attempts() {
    let data = terrain();
    let collision = world(&data);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_000B);
    let scripts = authored_scripts();
    let models = ObjectModels::default();
    let object_anims = ObjectAnimations::default();
    let objects = LevelObjects::mario_only(&scripts, &models, &object_anims);
    let oracle = Oracle::load(&data);
    oracle.set_mario_animations(&anims);
    oracle.tick_set_objects(&NativeObjects::new(&objects));
    let setup = TickSetup::from_level_script(LEVEL_BOB, 1, 0, [0; 3], 0, CAMERA_MODE_RADIAL as u8);
    let mut state = Fixture::begin(&oracle, &collision, &trig, &anims, &objects, &setup, seed());
    // Neutral initializes the original function-static state without changing source.
    state.call(
        &oracle,
        Call::Escape {
            stick_mag: 0.0,
            pressed: 0,
        },
    );
    let mut actions = 0;
    for _ in 0..64 {
        for (mag, pressed, expected) in [
            (40.0, 0, 0),
            (40.001, 0, 1),
            (64.0, 0, 0),
            (30.0, 0, 0),
            (40.001, 0, 0),
            (29.999, 0, 0),
            (40.001, A_BUTTON, 1),
            (35.0, A_BUTTON, 1),
            (35.0, 0, 0),
            (0.0, 0, 0),
        ] {
            let got = state.call(
                &oracle,
                Call::Escape {
                    stick_mag: mag,
                    pressed,
                },
            );
            assert_eq!(got, expected);
            actions += got;
        }
    }
    assert_eq!(actions, 192);
    println!("escape controls: 641 exact calls; {actions} escape actions");
}

#[test]
fn thrown_or_placed_object_dispatch_matches() {
    let data = terrain();
    let collision = world(&data);
    let trig = computed_tables();
    let anims = authored_animations(0x5EED_000B);
    let scripts = authored_scripts();
    let models = ObjectModels::default();
    let object_anims = ObjectAnimations::default();
    let objects = LevelObjects::mario_only(&scripts, &models, &object_anims);
    let oracle = Oracle::load(&data);
    oracle.set_mario_animations(&anims);
    oracle.tick_set_objects(&NativeObjects::new(&objects));
    let setup = TickSetup::from_level_script(LEVEL_BOB, 1, 0, [0; 3], 0, CAMERA_MODE_RADIAL as u8);
    let mut calls = 0;
    for object_pos in [
        [0.0, -20.0, 0.0],
        [100.0, 100.0, 200.0],
        [15000.0, 0.0, 0.0],
    ] {
        for subtype in [0, INT_SUBTYPE_HOLDABLE_NPC, INT_SUBTYPE_GRABS_MARIO] {
            for forward_vel in [0.0, -0.0, 20.0, -20.0] {
                for vel_y in [-10.0, 0.0, 50.0] {
                    let f = GrabFixture {
                        object_pos,
                        subtype,
                        holding: 2,
                        ..seed()
                    };
                    let mut state =
                        Fixture::begin(&oracle, &collision, &trig, &anims, &objects, &setup, f);
                    state.call(
                        &oracle,
                        Call::Release {
                            forward_vel,
                            vel_y,
                            action: 4,
                        },
                    );
                    calls += 1;
                    let raw = &state.w.objects.slot(state.actor).raw;
                    assert_eq!(raw.s32(O_HELD_STATE), HELD_FREE);
                    assert_eq!(
                        raw.s32(O_ACTION),
                        if subtype & INT_SUBTYPE_HOLDABLE_NPC == 0 && forward_vel != 0.0 {
                            4
                        } else {
                            0
                        }
                    );
                }
            }
        }
    }
    println!("object release: {calls} exact calls");
}

fn heavy_actions(
    oracle: &Oracle,
    collision: &CollisionWorld,
    trig: &TrigTables,
    anims: &MarioAnimations,
    objects: &LevelObjects<'_>,
    label: &str,
) {
    let setup = TickSetup::from_level_script(LEVEL_BOB, 1, 0, [0; 3], 0, CAMERA_MODE_RADIAL as u8);
    let mut calls = 0;
    let mut actions = std::collections::BTreeSet::new();
    for walking_ticks in [0, 15, 40] {
        let f = GrabFixture {
            action: ACT_PICKING_UP,
            ..seed()
        };
        let mut state = Fixture::begin(oracle, collision, trig, anims, objects, &setup, f);
        // Picking up continues the animation that led into it; a NULL
        // animation would be an invalid original-game state.
        state.call(oracle, Call::Animation(MARIO_ANIM_FIRST_PUNCH));
        calls += 1;
        // Follow the original action dispatch's cancellation loop, then the
        // animation advance. This is an action component chain, not a game frame.
        let mut step = |state: &mut Fixture<'_>, input, magnitude, yaw| {
            state.call(
                oracle,
                Call::Intent {
                    input,
                    magnitude,
                    yaw,
                },
            );
            calls += 1;
            loop {
                actions.insert(state.m.action);
                let call = match state.m.action & ACT_GROUP_MASK {
                    ACT_GROUP_OBJECT => Call::ObjectAction,
                    ACT_GROUP_STATIONARY => Call::StationaryAction,
                    ACT_GROUP_MOVING => Call::MovingAction,
                    other => panic!("heavy action left checked groups: {other:#X}"),
                };
                calls += 1;
                if state.call(oracle, call) == 0 {
                    break;
                }
            }
            state.call(oracle, Call::AdvanceAnimation);
            calls += 1;
        };
        for _ in 0..240 {
            step(&mut state, 0, 0.0, 0);
            if state.m.action == ACT_HOLD_HEAVY_IDLE {
                break;
            }
        }
        assert_eq!(
            state.m.action, ACT_HOLD_HEAVY_IDLE,
            "heavy pickup never finished"
        );
        assert_eq!(state.m.held_obj, Some(state.actor));
        assert_eq!(state.m.body.grab_pos, GRAB_POS_HEAVY_OBJ);
        for _ in 0..walking_ticks {
            step(&mut state, INPUT_NONZERO_ANALOG, 32.0, 0x1800);
            assert_eq!(state.m.action, ACT_HOLD_HEAVY_WALKING);
        }
        // Heavy throw releases with HELD_DROPPED on its 13th tick, unlike
        // a light throw. King Bob-omb's dispatcher supplies the launch speeds.
        step(&mut state, INPUT_B_PRESSED, 0.0, 0);
        assert_eq!(state.m.action, ACT_HEAVY_THROW);
        for _ in 0..12 {
            assert_eq!(state.m.held_obj, Some(state.actor));
            step(&mut state, 0, 0.0, 0);
        }
        assert_eq!(state.m.action_timer, 13);
        assert!(state.m.held_obj.is_none());
        assert_eq!(
            state.w.objects.slot(state.actor).raw.s32(O_HELD_STATE),
            HELD_DROPPED
        );
        state.call(
            oracle,
            Call::Release {
                forward_vel: 20.0,
                vel_y: 50.0,
                action: 4,
            },
        );
        calls += 1;
    }
    for action in [
        ACT_PICKING_UP,
        ACT_HOLD_HEAVY_IDLE,
        ACT_HOLD_HEAVY_WALKING,
        ACT_HEAVY_THROW,
    ] {
        assert!(actions.contains(&action), "no heavy action {action:#X}");
    }
    println!("{label}: {calls} exact component calls; all four heavy actions, 3 releases");
}

#[test]
fn heavy_pickup_walk_and_thirteenth_tick_release_match() {
    let data = terrain();
    let collision = world(&data);
    let trig = computed_tables();
    let mut anims = authored_animations(0x5EED_000B);
    // Invented forward-playing animations make every heavy transition reachable.
    for index in [
        MARIO_ANIM_FIRST_PUNCH,
        MARIO_ANIM_PICK_UP_LIGHT_OBJ,
        MARIO_ANIM_GRAB_HEAVY_OBJECT,
        MARIO_ANIM_HEAVY_THROW,
    ] {
        anims.animations[index as usize].flags = 0;
    }
    let scripts = authored_scripts();
    let models = ObjectModels::default();
    let object_anims = ObjectAnimations::default();
    let objects = LevelObjects::mario_only(&scripts, &models, &object_anims);
    let oracle = Oracle::load(&data);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    oracle.tick_set_objects(&NativeObjects::new(&objects));
    heavy_actions(
        &oracle,
        &collision,
        &trig,
        &anims,
        &objects,
        "authored heavy actions",
    );
}

#[test]
#[ignore = "requires RUSTARIO64_ROM; original Mario heavy animations and trig"]
fn heavy_actions_match_with_owner_rom_animations() {
    use rustario64::import::{animation, engine, rom::Rom};
    let rom = Rom::open(std::path::Path::new(
        &std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM"),
    ))
    .unwrap();
    let data = terrain();
    let collision = world(&data);
    let trig = engine::trig_tables(&rom).unwrap();
    let anims = animation::mario_animations(&rom).unwrap();
    let scripts = authored_scripts();
    let models = ObjectModels::default();
    let object_anims = ObjectAnimations::default();
    let objects = LevelObjects::mario_only(&scripts, &models, &object_anims);
    let oracle = Oracle::load(&data);
    oracle.set_trig(trig.sine_table(), trig.arctan_table());
    oracle.set_mario_animations(&anims);
    oracle.tick_set_objects(&NativeObjects::new(&objects));
    heavy_actions(
        &oracle,
        &collision,
        &trig,
        &anims,
        &objects,
        "owner-ROM heavy actions",
    );
}
