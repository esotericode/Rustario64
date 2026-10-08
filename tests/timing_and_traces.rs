use rustario64::{
    diagnostics,
    presentation::{self, GraphicsOptions, Snapshot},
    simulation::{FixedClock, InputEdges, TickInput},
    trace::{self, ObjectState},
};
use std::{collections::BTreeMap, time::Duration};

#[test]
fn clock_uses_rational_30hz_with_no_nanosecond_tick_rounding() {
    let mut clock = FixedClock::default();
    clock.add_elapsed(Duration::from_nanos(33_333_333)).unwrap();
    assert_eq!(clock.drain(8), 0);
    clock.add_elapsed(Duration::from_nanos(1)).unwrap();
    assert_eq!(clock.drain(8), 1);
    clock
        .add_elapsed(Duration::from_nanos(966_666_666))
        .unwrap();
    assert_eq!(clock.drain(100), 29);
    assert_eq!(clock.pending_ticks(), 0);
    assert_eq!(clock.alpha(), 0.0);
}

#[test]
fn stalled_clock_retains_every_pending_tick_after_bounded_drains() {
    let mut clock = FixedClock::default();
    clock.add_elapsed(Duration::from_secs(2)).unwrap();
    assert_eq!(clock.drain(8), 8);
    assert_eq!(clock.pending_ticks(), 52);
    assert_eq!(clock.alpha(), 1.0);
    assert_eq!(clock.drain(0), 0);
    assert_eq!(clock.pending_ticks(), 52);
    let mut remaining = 0;
    while clock.pending_ticks() > 0 {
        remaining += clock.drain(8);
    }
    assert_eq!(remaining, 52);
    assert_eq!(clock.alpha(), 0.0);
}

#[test]
fn clock_long_sequence_matches_elapsed_time_without_accumulated_drift() {
    let mut clock = FixedClock::default();
    let mut total = 0u64;
    for _ in 0..100_000 {
        clock.add_elapsed(Duration::from_nanos(16_666_667)).unwrap();
        total += u64::from(clock.drain(8));
    }
    assert_eq!(total, 50_000);
    assert!(clock.alpha() > 0.0 && clock.alpha() < 0.002);
}

#[test]
fn input_edges_are_consumed_once_per_tick() {
    let mut edges = InputEdges::default();
    let mut input = TickInput {
        buttons: 0x8000,
        ..TickInput::default()
    };
    assert_eq!(edges.consume(input), (0x8000, 0));
    assert_eq!(edges.consume(input), (0, 0));
    input.buttons = 0x4000;
    assert_eq!(edges.consume(input), (0x4000, 0x8000));
    input.buttons = 0;
    assert_eq!(edges.consume(input), (0, 0x4000));
}

#[test]
fn render_rates_and_graphics_settings_preserve_every_synthetic_tick() {
    let reference = diagnostics::counter_replay(30, GraphicsOptions::default()).unwrap();
    assert_eq!(reference.frames.len(), 300);
    for cap in [30, 60, 120, 144] {
        for interpolation in [false, true] {
            for enhancements in [false, true] {
                let options = GraphicsOptions {
                    interpolation,
                    enhanced_lighting: enhancements,
                    dynamic_shadows: enhancements,
                };
                let candidate = diagnostics::counter_replay(cap, options).unwrap();
                trace::compare(&reference, &candidate).unwrap();
            }
        }
    }
}

fn snapshots() -> (Snapshot, Snapshot) {
    let previous = Snapshot {
        entity: 1,
        epoch: 4,
        position: [0., 2., 4.],
        yaw: 32760,
        animation: 3,
        discontinuity: false,
    };
    let current = Snapshot {
        position: [10., 12., 14.],
        yaw: -32760,
        ..previous
    };
    (previous, current)
}

#[test]
fn presentation_interpolates_short_wrapped_angle_without_mutating_snapshots() {
    let (previous, current) = snapshots();
    let original = (previous, current);
    let pose =
        presentation::interpolate(Some(&previous), &current, 0.5, GraphicsOptions::default());
    assert_eq!(pose.position, [5., 7., 9.]);
    assert_eq!(pose.yaw, -32768);
    assert_eq!((previous, current), original);
}

#[test]
fn discontinuities_spawn_animation_switch_and_disabled_interpolation_snap() {
    let (previous, current) = snapshots();
    for modified in [
        Snapshot {
            epoch: 5,
            ..current
        },
        Snapshot {
            entity: 2,
            ..current
        },
        Snapshot {
            animation: 4,
            ..current
        },
        Snapshot {
            discontinuity: true,
            ..current
        },
    ] {
        let pose =
            presentation::interpolate(Some(&previous), &modified, 0.2, GraphicsOptions::default());
        assert_eq!(pose.position, current.position);
        assert_eq!(pose.yaw, current.yaw);
    }
    assert_eq!(
        presentation::interpolate(None, &current, 0.2, GraphicsOptions::default()).position,
        current.position
    );
    let disabled = GraphicsOptions {
        interpolation: false,
        ..GraphicsOptions::default()
    };
    assert_eq!(
        presentation::interpolate(Some(&previous), &current, 0.2, disabled).position,
        current.position
    );
    assert_eq!(
        presentation::interpolate(
            Some(&previous),
            &current,
            f32::NAN,
            GraphicsOptions::default()
        )
        .position,
        current.position
    );
}

#[test]
fn exact_comparison_reports_first_tick_and_float_bit_field() {
    let reference = diagnostics::counter_replay(30, GraphicsOptions::default()).unwrap();
    let mut candidate = reference.clone();
    candidate.frames[6].state.position_bits[0] = 1; // One ULP away from zero.
    candidate.frames[8].state.action = 42;
    let d = trace::compare(&reference, &candidate).unwrap_err();
    assert_eq!(d.tick, Some(7));
    assert_eq!(d.field, "frame.state.position_bits[0]");
    assert_eq!(d.expected, "0");
    assert_eq!(d.actual, "1");
}

#[test]
fn exact_comparison_distinguishes_signed_zero_and_discrete_state() {
    let reference = diagnostics::counter_replay(30, GraphicsOptions::default()).unwrap();
    let mut candidate = reference.clone();
    candidate.frames[0].state.velocity_bits[2] = (-0.0f32).to_bits();
    assert_eq!(
        trace::compare(&reference, &candidate).unwrap_err().field,
        "frame.state.velocity_bits[2]"
    );
    candidate = reference.clone();
    candidate.frames[1].state.action_timer += 1;
    assert_eq!(
        trace::compare(&reference, &candidate).unwrap_err().tick,
        Some(2)
    );
    candidate = reference.clone();
    candidate.frames[2].input.camera_yaw = 1;
    assert_eq!(
        trace::compare(&reference, &candidate).unwrap_err().field,
        "frame.input.camera_yaw"
    );
}

#[test]
fn comparator_rejects_mismatched_initial_state_world_or_profile() {
    let reference = diagnostics::counter_replay(30, GraphicsOptions::default()).unwrap();
    let mut candidate = reference.clone();
    candidate.metadata.initial_state.rng = 123;
    assert_eq!(
        trace::compare(&reference, &candidate).unwrap_err().field,
        "metadata.initial_state.rng"
    );
    candidate = reference.clone();
    candidate.metadata.initial_world_digest = "another world".into();
    assert_eq!(
        trace::compare(&reference, &candidate).unwrap_err().field,
        "metadata.initial_world_digest"
    );
    candidate = reference.clone();
    candidate
        .metadata
        .gameplay_options
        .insert("modern camera".into(), "yes".into());
    assert!(
        trace::compare(&reference, &candidate)
            .unwrap_err()
            .field
            .starts_with("metadata.gameplay_options")
    );
    candidate = reference.clone();
    candidate.producer = "a different producer".into();
    trace::compare(&reference, &candidate).unwrap();
}

#[test]
fn comparator_rejects_empty_replays_bad_metadata_and_tick_gaps() {
    let reference = diagnostics::counter_replay(30, GraphicsOptions::default()).unwrap();
    let mut candidate = reference.clone();
    candidate.frames.clear();
    assert!(
        trace::compare(&reference, &candidate)
            .unwrap_err()
            .field
            .ends_with("validation")
    );
    candidate = reference.clone();
    candidate.metadata.tick_rate = 60;
    assert!(trace::compare(&reference, &candidate).is_err());
    candidate = reference.clone();
    candidate.metadata.reference_revision = "not pinned".into();
    assert!(trace::compare(&reference, &candidate).is_err());
    candidate = reference.clone();
    candidate.frames[6].tick = 8;
    assert!(trace::compare(&reference, &candidate).is_err());
    candidate = reference.clone();
    candidate.frames.pop();
    let d = trace::compare(&reference, &candidate).unwrap_err();
    assert_eq!(d.tick, Some(300));
    assert_eq!(d.field, "frames.length");
}

#[test]
fn comparator_preserves_object_update_order_and_rejects_duplicate_ids() {
    let mut reference = diagnostics::counter_replay(30, GraphicsOptions::default()).unwrap();
    let object = ObjectState {
        id: 7,
        behavior: 1,
        action: 0,
        timer: 0,
        position_bits: [0; 3],
        velocity_bits: [0; 3],
        fields: BTreeMap::new(),
    };
    reference.frames[0].state.objects = vec![
        object.clone(),
        ObjectState {
            id: 8,
            ..object.clone()
        },
    ];
    let mut candidate = reference.clone();
    candidate.frames[0].state.objects.reverse();
    assert_eq!(
        trace::compare(&reference, &candidate).unwrap_err().field,
        "frame.state.objects[0].id"
    );
    candidate.frames[0].state.objects = vec![object.clone(), object];
    assert!(
        trace::compare(&reference, &candidate)
            .unwrap_err()
            .field
            .ends_with("validation")
    );
}

#[test]
fn trace_deserialization_requires_fields_and_rejects_unknown_fields() {
    let trace = diagnostics::counter_replay(30, GraphicsOptions::default()).unwrap();
    let mut value = serde_json::to_value(&trace).unwrap();
    value["frames"][0]["state"]
        .as_object_mut()
        .unwrap()
        .remove("contacts");
    assert!(serde_json::from_value::<trace::Trace>(value).is_err());
    let mut value = serde_json::to_value(&trace).unwrap();
    value["frames"][0]["input"]["unrecorded_option"] = serde_json::json!(true);
    assert!(serde_json::from_value::<trace::Trace>(value).is_err());
}
