//! Independently authored fixtures and a synthetic counter replay for the runnable
//! foundation. These contain no Mario terrain, animation, or gameplay algorithms.
use crate::{
    import::{self, reader::Reader},
    presentation::{self, GraphicsOptions, Snapshot},
    simulation::{FixedClock, InputEdges, TICKS_PER_SECOND, TickInput},
    trace::*,
};
use std::{collections::BTreeMap, time::Duration};

pub fn collision_fixture() -> Vec<u8> {
    // One triangle, a tree-shaped preset record (synthetic position), and a waterbox.
    let words: &[i16] = &[
        0x40, 3, -100, 0, -100, 100, 0, -100, 0, 0, 100, 0, 1, 0, 2, 1, 0x41, 0x43, 1, 0x79, 12,
        34, 56, 0x44, 1, 0, -50, -50, 50, 50, -10, 0x42,
    ];
    words.iter().flat_map(|v| v.to_be_bytes()).collect()
}

/// Literal-only MIO0 generator. Authored for fixtures, not adapted from an encoder.
pub fn literal_mio0(bytes: &[u8]) -> Vec<u8> {
    let masks = bytes.len().div_ceil(8);
    let offset = 16 + masks;
    let mut out = b"MIO0".to_vec();
    for n in [bytes.len(), offset, offset] {
        out.extend_from_slice(&(n as u32).to_be_bytes());
    }
    out.resize(offset, 0xFF);
    out.extend_from_slice(bytes);
    out
}

/// A complete static-import fixture using the adapter's layout, synthetic content.
pub fn bob_segments_fixture() -> (Vec<u8>, Vec<u8>) {
    use import::version;
    let mut terrain = vec![0; (version::BOB_COLLISION & 0xFFFFFF) as usize];
    for p in terrain[..0x2800].as_chunks_mut::<2>().0.iter_mut() {
        p.copy_from_slice(&0x07C1u16.to_be_bytes());
    }
    terrain.extend_from_slice(&collision_fixture());
    let macro_address = (u32::from(version::TERRAIN_SEGMENT) << 24) | terrain.len() as u32;
    // Two synthetic modern macro records, followed by the normal terminator.
    for value in [
        0x8026u16,
        (-123i16) as u16,
        321,
        (-456i16) as u16,
        0xA5B6,
        396,
        111,
        222,
        333,
        0,
        30,
    ] {
        terrain.extend_from_slice(&value.to_be_bytes());
    }
    let mut script = vec![0x1B, 4, 0, 0, 0x18, 12, 0, 7];
    script.extend_from_slice(&(version::BOB_TERRAIN.start as u32).to_be_bytes());
    script.extend_from_slice(&(version::BOB_TERRAIN.end as u32).to_be_bytes());
    script.extend_from_slice(&[0x1F, 8, 1, 0, 0x0E, 0, 2, 0]);
    let link = script.len();
    script.extend_from_slice(&[0x06, 8, 0, 0, 0, 0, 0, 0]);
    script.extend_from_slice(&[0x26, 8, 10, 9, 1, 10, 0, 0]);
    script.extend_from_slice(&[0x2E, 8, 0, 0]);
    script.extend_from_slice(&version::BOB_COLLISION.to_be_bytes());
    script.extend_from_slice(&[0x39, 8, 0, 0]);
    script.extend_from_slice(&macro_address.to_be_bytes());
    script.extend_from_slice(&[0x20, 4, 0, 0, 0x2B, 12, 1, 0]);
    for value in [90i16, 11, 22, 33] {
        script.extend_from_slice(&value.to_be_bytes());
    }
    script.extend_from_slice(&[
        0x11, 8, 0, 0, 0x80, 0x24, 0, 0, 0x1C, 4, 0, 0, 0x04, 4, 0, 1,
    ]);
    let target = (u32::from(version::SCRIPT_SEGMENT) << 24) | script.len() as u32;
    script[link + 4..link + 8].copy_from_slice(&target.to_be_bytes());
    for (acts, x) in [(0x1Fu8, -7i16), (0x20, 8)] {
        script.extend_from_slice(&[0x24, 24, acts, 77]);
        for value in [x, 23, -45, 0, 90, -180] {
            script.extend_from_slice(&value.to_be_bytes());
        }
        script.extend_from_slice(&0x12345678u32.to_be_bytes());
        script.extend_from_slice(&0x13001234u32.to_be_bytes());
    }
    script.extend_from_slice(&[0x07, 4, 0, 0]);
    (terrain, script)
}

pub fn initial_counter_state() -> TickState {
    TickState {
        action: 0,
        action_state: 0,
        action_timer: 0,
        position_bits: [0; 3],
        velocity_bits: [0; 3],
        face_angles: [0; 3],
        camera_yaw: 0,
        rng: 0,
        contacts: vec![],
        interactions: vec![],
        objects: vec![],
        fields: BTreeMap::new(),
    }
}

/// Same ten-second synthetic counter/input replay at any presentation cadence.
/// State fields exercise the comparator; they do not represent Mario movement.
pub fn counter_replay(render_hz: u32, options: GraphicsOptions) -> Result<Trace, &'static str> {
    if !(1..=1000).contains(&render_hz) {
        return Err("render rate must be 1..=1000");
    }
    let initial_state = initial_counter_state();
    let mut trace = Trace {
        schema: TRACE_SCHEMA,
        producer: "rustario64 synthetic counter".into(),
        metadata: Metadata {
            rom_sha1: "synthetic".into(),
            reference_revision: import::version::REFERENCE_REVISION.into(),
            reference_configuration: "synthetic counter v1; no gameplay oracle".into(),
            scenario: "counter-and-input-edges-10s".into(),
            tick_rate: TICKS_PER_SECOND,
            course: 1,
            area: 1,
            act: 1,
            initial_world_digest: "authored empty world v1".into(),
            gameplay_options: BTreeMap::new(),
            initial_state: initial_state.clone(),
        },
        frames: vec![],
    };
    let mut clock = FixedClock::default();
    let mut edges = InputEdges::default();
    let mut tick = 0u64;
    let mut previous = None;
    let mut current = Snapshot {
        entity: 1,
        epoch: 0,
        position: [0.0; 3],
        yaw: 0,
        animation: 0,
        discontinuity: false,
    };
    let total_frames = u64::from(render_hz) * 10;
    let mut elapsed_ns = 0;
    for frame in 1..=total_frames {
        let end = frame * 1_000_000_000 / u64::from(render_hz);
        clock.add_elapsed(Duration::from_nanos(end - elapsed_ns))?;
        elapsed_ns = end;
        for _ in 0..clock.drain(8) {
            tick += 1;
            let input = TickInput {
                buttons: if tick % 40 < 20 { 0x8000 } else { 0 },
                stick: [0, 0],
                camera_yaw: 0,
            };
            let (pressed, released) = edges.consume(input);
            let mut state = initial_state.clone();
            state.action_timer = tick as u16;
            state
                .fields
                .insert("synthetic_tick_counter".into(), tick as u32);
            state.fields.insert("pressed".into(), u32::from(pressed));
            state.fields.insert("released".into(), u32::from(released));
            trace.frames.push(Frame { tick, input, state });
            previous = Some(current);
            // Presentation-only diagnostic marker, not Mario's authoritative position.
            current.position[0] = tick as f32;
        }
        let _pose = presentation::interpolate(previous.as_ref(), &current, clock.alpha(), options);
    }
    if clock.pending_ticks() != 0 {
        return Err("diagnostic rate leaves a backlog; use >= 4 Hz");
    }
    if tick != 300 {
        return Err("scheduler did not produce 300 ticks in ten seconds");
    }
    Ok(trace)
}

pub fn parser_smoke() -> import::Result<import::bob::BobImport> {
    let (terrain, script) = bob_segments_fixture();
    let packed = literal_mio0(&terrain);
    let unpacked = import::mio0::decode(&packed, import::version::MAX_SEGMENT_BYTES)?;
    // Header reader is exercised separately from the synthetic segment importer.
    if Reader::new(&packed, "fixture").u32(4)? as usize != terrain.len() {
        return Err(import::ImportError::new(
            "fixture",
            4,
            "literal generator length mismatch",
        ));
    }
    import::bob::decode_segments(unpacked, script)
}
