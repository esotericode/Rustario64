//! Development-only differential oracle. It runs the pinned decompilation's
//! collision loader and queries (vendored CC0 C, compiled natively by build.rs)
//! so tests can compare the Rust port bit for bit. The C keeps global state, so
//! every call goes through one process-wide lock.
//!
//! Boundary: this crate is never a dependency of the game runtime or renderer.
//! Replacement plan: once per-tick traces from original execution cover these
//! queries, those traces become the authority and this harness can be retired.
pub mod input_trace;
pub mod tick_trace;

use rustario64::{content::animation::MarioAnimations, simulation::TickInput};
use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct OracleSurface {
    surface_type: i16,
    force: i16,
    flags: i8,
    room: i8,
    lower_y: i16,
    upper_y: i16,
    vertices: [i16; 9],
    normal: [f32; 3],
    origin_offset: f32,
}

unsafe extern "C" {
    fn oracle_load(data: *const i16, words: i32) -> i32;
    fn oracle_node_count() -> i32;
    fn oracle_get_surface(index: i32, out: *mut OracleSurface);
    fn oracle_cell_list(
        dynamic: i32,
        cell_z: i32,
        cell_x: i32,
        kind: i32,
        out: *mut i32,
        max: i32,
    ) -> i32;
    fn oracle_find_floor(
        x: f32,
        y: f32,
        z: f32,
        include_intangible: i16,
        for_camera: i16,
        index: *mut i32,
        include_after: *mut i16,
    ) -> f32;
    fn oracle_find_ceil(x: f32, y: f32, z: f32, for_camera: i16, index: *mut i32) -> f32;
    fn oracle_find_walls(
        position: *mut f32,
        offset_y: f32,
        radius: f32,
        for_camera: i16,
        pass_vanish_walls: i32,
        num_walls: *mut i16,
        walls: *mut i32,
    ) -> i32;
    fn oracle_find_water_level(x: f32, z: f32) -> f32;
    fn oracle_find_poison_gas_level(x: f32, z: f32) -> f32;
    fn oracle_set_trig(sine: *const f32, arctan: *const i16);
    fn oracle_sins(x: i32) -> f32;
    fn oracle_coss(x: i32) -> f32;
    fn oracle_atan2s(y: f32, x: f32) -> i16;
    fn oracle_atan2f(y: f32, x: f32) -> f32;
    fn oracle_approach_s32(current: i32, target: i32, inc: i32, dec: i32) -> i32;
    fn oracle_approach_f32(current: f32, target: f32, inc: f32, dec: f32) -> f32;
    fn oracle_mario_call(state: *mut OracleMario, which: i32, arg: u32) -> i32;
    fn oracle_input_tick(state: *mut OracleMario, input: *mut OracleInput, which: i32);
    fn oracle_constant_count() -> i32;
    fn oracle_constant(i: i32, value: *mut i64) -> *const std::ffi::c_char;
    fn oracle_set_mario_anims(
        count: i32,
        headers: *const i16,
        indices: *const *const u16,
        index_lens: *const i32,
        values: *const *const i16,
        value_lens: *const i32,
    ) -> i32;
    fn oracle_tick_begin(setup: *const TickSetup);
    fn oracle_tick_run(input: *const OracleTickInput);
    fn oracle_tick_snapshot(
        names: *mut *const *const std::ffi::c_char,
        words: *mut *const u32,
    ) -> i32;
}

/// A level entry for the tick oracle (layout matches `OracleTickSetup` in
/// c/tick.c): gMarioSpawnInfo as the level script sets it, the level and
/// area, the camera mode inputs, and the save-file inputs.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickSetup {
    pub start_pos: [i32; 3],
    pub start_angle: [i32; 3],
    pub area_index: i32,
    pub active_area_index: i32,
    pub behavior_arg: u32,
    pub level_num: i32,
    pub terrain_type: u32,
    pub camera_mode: i32,
    pub camera_def_mode: i32,
    pub save_flags: u32,
    pub total_stars: i32,
    /// The rendered area's index (gCurGraphNodeRoot->areaIndex).
    pub root_area_index: i32,
}

#[repr(C)]
struct OracleTickInput {
    buttons: u32,
    stick: [i32; 2],
    camera_yaw: i32,
}

/// Flat MarioState plus the world inputs the step code reads (layout matches
/// `OracleMario` in c/oracle.c). Surfaces: index, -1 NULL, -2 water pseudo-floor.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OracleMario {
    pub input: u16,
    pub flags: u32,
    pub action: u32,
    pub terrain_sound_addend: u32,
    pub particle_flags: u32,
    pub collided_obj_interact_types: u32,
    pub intended_mag: f32,
    pub intended_yaw: i16,
    pub frames_since_a: u8,
    pub frames_since_b: u8,
    pub squish_timer: u8,
    pub wall_kick_timer: u8,
    pub double_jump_timer: u8,
    pub face_angle: [i16; 3],
    pub angle_vel: [i16; 3],
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub forward_vel: f32,
    pub slide_vel_x: f32,
    pub slide_vel_z: f32,
    pub wall: i32,
    pub ceil: i32,
    pub floor: i32,
    pub ceil_height: f32,
    pub floor_height: f32,
    pub floor_angle: i16,
    pub water_level: i16,
    pub peak_height: f32,
    pub quicksand_depth: f32,
    pub getting_blown_gravity: f32,
    pub wing_flutter: i8,
    pub gfx_pos: [f32; 3],
    pub gfx_angle: [i16; 3],
    pub global_timer: u32,
    pub area_terrain_type: u16,
    pub level_num: i16,
    pub water_pseudo_origin_offset: f32,
    pub include_intangible: i16,
}

/// Functions callable through `Oracle::mario_call`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarioCall {
    GroundStep,
    AirStep(u32),
    StationaryGroundStep,
    StopAndSetHeightToFloor,
    BonkReflection(bool),
    ApplyGravity,
    ApplyVerticalWind,
    VelFromPitchAndYaw,
    VelFromYaw,
    SetForwardVel(f32),
    UpdateMovingSand,
    UpdateWindyGround,
}

/// (name, value) for every constant the C side evaluated from the real headers.
pub fn decomp_constants() -> Vec<(String, i64)> {
    let _guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: the table is static; names are NUL-terminated string literals.
    unsafe {
        (0..oracle_constant_count())
            .map(|i| {
                let mut value = 0;
                let name = std::ffi::CStr::from_ptr(oracle_constant(i, &mut value));
                (name.to_string_lossy().into_owned(), value)
            })
            .collect()
    }
}

/// Controller and explicit camera/object context. button_down is the previous
/// sample on entry, and the consumed sample on return.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct OracleInput {
    pub raw_stick: [i16; 2],
    pub stick_x: f32,
    pub stick_y: f32,
    pub stick_mag: f32,
    pub button_down: u16,
    pub button_pressed: u16,
    pub sample_buttons: u16,
    pub camera_yaw: i16,
    pub camera_movement_flags: u16,
    pub object_interact_status: u32,
    pub object_collided_interact_types: u32,
    pub death_warp_requests: i32,
}
#[derive(Debug, Clone, Copy)]
pub enum InputCall {
    Controller,
    ButtonsAndJoystick,
    Full,
}

static LOCK: Mutex<()> = Mutex::new(());

/// A surface as stored by the original loader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CSurface {
    pub surface_type: i16,
    pub force: i16,
    pub flags: i8,
    pub room: i8,
    pub lower_y: i16,
    pub upper_y: i16,
    pub vertices: [[i16; 3]; 3],
    pub normal: [f32; 3],
    pub origin_offset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallResult {
    pub position: [f32; 3],
    pub collisions: i32,
    pub num_walls: i16,
    pub walls: [Option<u16>; 4],
}

fn index(raw: i32) -> Option<u16> {
    u16::try_from(raw).ok()
}

/// Exclusive access to the C collision state with a loaded terrain stream.
pub struct Oracle {
    _guard: MutexGuard<'static, ()>,
    surfaces: usize,
}

impl Oracle {
    /// Advance a selected input stage. No action or camera algorithm runs.
    pub fn input_tick(&self, state: &mut OracleMario, input: &mut OracleInput, call: InputCall) {
        for s in [state.wall, state.ceil, state.floor] {
            assert!(s == -1 || s == -2 || (0..self.surfaces as i32).contains(&s));
        }
        assert!(input.raw_stick.iter().all(|s| (-128..=127).contains(s)));
        let which = match call {
            InputCall::Controller => 0,
            InputCall::ButtonsAndJoystick => 1,
            InputCall::Full => 2,
        };
        // SAFETY: valid flat records, checked surface indices, process-wide lock.
        unsafe { oracle_input_tick(state, input, which) };
    }

    /// load_area_terrain on a raw TerrainData stream (big-endian words already
    /// converted to host i16). The stream must end with TERRAIN_LOAD_END.
    pub fn load(stream: &[i16]) -> Self {
        let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let words = i32::try_from(stream.len()).expect("stream length fits i32");
        // SAFETY: the C side copies `words` shorts from a valid slice before use.
        let surfaces = unsafe { oracle_load(stream.as_ptr(), words) };
        Self {
            _guard: guard,
            surfaces: usize::try_from(surfaces).expect("non-negative surface count"),
        }
    }

    pub fn surface_count(&self) -> usize {
        self.surfaces
    }

    pub fn node_count(&self) -> usize {
        // SAFETY: reads a C global while holding the lock.
        usize::try_from(unsafe { oracle_node_count() }).expect("non-negative node count")
    }

    pub fn surface(&self, i: usize) -> CSurface {
        assert!(i < self.surfaces);
        let mut out = OracleSurface::default();
        // SAFETY: `i` is within the allocated pool; `out` is a valid destination.
        unsafe { oracle_get_surface(i as i32, &mut out) };
        let v = out.vertices;
        CSurface {
            surface_type: out.surface_type,
            force: out.force,
            flags: out.flags,
            room: out.room,
            lower_y: out.lower_y,
            upper_y: out.upper_y,
            vertices: [[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], v[8]]],
            normal: out.normal,
            origin_offset: out.origin_offset,
        }
    }

    pub fn cell_list(&self, dynamic: bool, cell_x: usize, cell_z: usize, kind: usize) -> Vec<u16> {
        let mut out = vec![0i32; 2400];
        // SAFETY: the C side writes at most `max` entries and returns the length.
        let n = unsafe {
            oracle_cell_list(
                i32::from(dynamic),
                cell_z as i32,
                cell_x as i32,
                kind as i32,
                out.as_mut_ptr(),
                out.len() as i32,
            )
        };
        let n = usize::try_from(n).expect("non-negative list length");
        assert!(n <= out.len(), "cell list longer than the oracle buffer");
        out[..n]
            .iter()
            .map(|&i| index(i).expect("valid index"))
            .collect()
    }

    /// Returns height, floor, and gFindFloorIncludeSurfaceIntangible afterwards.
    pub fn find_floor(
        &self,
        p: [f32; 3],
        include_intangible: bool,
        for_camera: bool,
    ) -> (f32, Option<u16>, bool) {
        let mut floor = -1;
        let mut after = 0;
        // SAFETY: out-pointers reference valid locals.
        let h = unsafe {
            oracle_find_floor(
                p[0],
                p[1],
                p[2],
                i16::from(include_intangible),
                i16::from(for_camera),
                &mut floor,
                &mut after,
            )
        };
        (h, index(floor), after != 0)
    }

    pub fn find_ceil(&self, p: [f32; 3], for_camera: bool) -> (f32, Option<u16>) {
        let mut ceil = -1;
        // SAFETY: out-pointer references a valid local.
        let h = unsafe { oracle_find_ceil(p[0], p[1], p[2], i16::from(for_camera), &mut ceil) };
        (h, index(ceil))
    }

    pub fn find_walls(
        &self,
        p: [f32; 3],
        offset_y: f32,
        radius: f32,
        for_camera: bool,
        pass_vanish_walls: bool,
    ) -> WallResult {
        let mut position = p;
        let mut num_walls = 0;
        let mut walls = [-1i32; 4];
        // SAFETY: position has three floats; walls has four entries.
        let collisions = unsafe {
            oracle_find_walls(
                position.as_mut_ptr(),
                offset_y,
                radius,
                i16::from(for_camera),
                i32::from(pass_vanish_walls),
                &mut num_walls,
                walls.as_mut_ptr(),
            )
        };
        WallResult {
            position,
            collisions,
            num_walls,
            walls: walls.map(index),
        }
    }

    /// Inject trig tables (0x1400 sine/cosine entries, 0x401 arctan entries).
    pub fn set_trig(&self, sine: &[f32], arctan: &[u16]) {
        assert_eq!(sine.len(), 0x1400);
        assert_eq!(arctan.len(), 0x401);
        let arctan: Vec<i16> = arctan.iter().map(|&v| v as i16).collect();
        // SAFETY: both slices have exactly the lengths the C side copies.
        unsafe { oracle_set_trig(sine.as_ptr(), arctan.as_ptr()) };
    }

    /// Run one mario_step.c function on the flat state, in place.
    pub fn mario_call(&self, state: &mut OracleMario, call: MarioCall) -> i32 {
        let (which, arg) = match call {
            MarioCall::GroundStep => (0, 0),
            MarioCall::AirStep(arg) => (1, arg),
            MarioCall::StationaryGroundStep => (2, 0),
            MarioCall::StopAndSetHeightToFloor => (3, 0),
            MarioCall::BonkReflection(negate) => (4, u32::from(negate)),
            MarioCall::ApplyGravity => (5, 0),
            MarioCall::ApplyVerticalWind => (6, 0),
            MarioCall::VelFromPitchAndYaw => (7, 0),
            MarioCall::VelFromYaw => (8, 0),
            MarioCall::SetForwardVel(v) => (9, v.to_bits()),
            MarioCall::UpdateMovingSand => (10, 0),
            MarioCall::UpdateWindyGround => (11, 0),
        };
        for s in [state.wall, state.ceil, state.floor] {
            assert!(s == -1 || s == -2 || (0..self.surfaces as i32).contains(&s));
        }
        // SAFETY: `state` is a valid OracleMario; surface indices were checked.
        unsafe { oracle_mario_call(state, which, arg) }
    }

    /// Load Mario's animation table into the C side's host-layout DMA table.
    pub fn set_mario_animations(&self, anims: &MarioAnimations) {
        let count = i32::try_from(anims.animations.len()).expect("table fits i32");
        let headers: Vec<i16> = anims
            .animations
            .iter()
            .flat_map(|a| {
                [
                    a.flags,
                    a.y_trans_divisor,
                    a.start_frame,
                    a.loop_start,
                    a.loop_end,
                    a.bone_count,
                ]
            })
            .collect();
        let indices: Vec<*const u16> = anims.animations.iter().map(|a| a.index.as_ptr()).collect();
        let index_lens: Vec<i32> = anims
            .animations
            .iter()
            .map(|a| i32::try_from(a.index.len()).expect("index fits i32"))
            .collect();
        let values: Vec<*const i16> = anims.animations.iter().map(|a| a.values.as_ptr()).collect();
        let value_lens: Vec<i32> = anims
            .animations
            .iter()
            .map(|a| i32::try_from(a.values.len()).expect("values fit i32"))
            .collect();
        // SAFETY: every pointer/length pair describes a live slice; the C side
        // copies the data before returning.
        let status = unsafe {
            oracle_set_mario_anims(
                count,
                headers.as_ptr(),
                indices.as_ptr(),
                index_lens.as_ptr(),
                values.as_ptr(),
                value_lens.as_ptr(),
            )
        };
        assert_eq!(status, 0, "oracle_set_mario_anims failed");
    }

    /// Enter a level as c/tick.c's oracle_tick_begin does. Requires the
    /// terrain (`load`) and `set_mario_animations`.
    pub fn tick_begin(&self, setup: &TickSetup) {
        // SAFETY: a valid setup record under the process-wide lock.
        unsafe { oracle_tick_begin(setup) };
    }

    /// Run one complete frame of the decomp's Mario code.
    pub fn tick(&self, input: TickInput) {
        let input = OracleTickInput {
            buttons: u32::from(input.buttons),
            stick: input.stick.map(i32::from),
            camera_yaw: i32::from(input.camera_yaw),
        };
        // SAFETY: a valid input record; tick_begin ran (the C side aborts otherwise).
        unsafe { oracle_tick_run(&input) };
    }

    /// Every compared word after the last frame, named as c/tick.c names it.
    pub fn tick_snapshot(&self) -> BTreeMap<String, u32> {
        let mut names = std::ptr::null();
        let mut words = std::ptr::null();
        // SAFETY: the C side returns static arrays of `n` entries that stay
        // valid until the next call, which the lock excludes.
        unsafe {
            let n = usize::try_from(oracle_tick_snapshot(&mut names, &mut words))
                .expect("non-negative snapshot length");
            let names = std::slice::from_raw_parts(names, n);
            let words = std::slice::from_raw_parts(words, n);
            names
                .iter()
                .zip(words)
                .map(|(&name, &word)| {
                    let name = std::ffi::CStr::from_ptr(name)
                        .to_string_lossy()
                        .into_owned();
                    (name, word)
                })
                .collect()
        }
    }

    pub fn find_water_level(&self, x: f32, z: f32) -> f32 {
        // SAFETY: pure query of C state under the lock.
        unsafe { oracle_find_water_level(x, z) }
    }

    pub fn find_poison_gas_level(&self, x: f32, z: f32) -> f32 {
        // SAFETY: pure query of C state under the lock.
        unsafe { oracle_find_poison_gas_level(x, z) }
    }
}

/// Exclusive access to the C math_util functions with injected trig tables.
pub struct MathOracle {
    _guard: MutexGuard<'static, ()>,
}

impl MathOracle {
    /// `sine` holds 0x1400 entries (sine then cosine), `arctan` 0x401.
    pub fn new(sine: &[f32], arctan: &[u16]) -> Self {
        assert_eq!(sine.len(), 0x1400);
        assert_eq!(arctan.len(), 0x401);
        let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let arctan: Vec<i16> = arctan.iter().map(|&v| v as i16).collect();
        // SAFETY: both slices have exactly the lengths the C side copies.
        unsafe { oracle_set_trig(sine.as_ptr(), arctan.as_ptr()) };
        Self { _guard: guard }
    }

    pub fn sins(&self, x: i32) -> f32 {
        // SAFETY: pure table lookup under the lock.
        unsafe { oracle_sins(x) }
    }

    pub fn coss(&self, x: i32) -> f32 {
        // SAFETY: pure table lookup under the lock.
        unsafe { oracle_coss(x) }
    }

    pub fn atan2s(&self, y: f32, x: f32) -> i16 {
        // SAFETY: pure function of its arguments and the injected tables.
        unsafe { oracle_atan2s(y, x) }
    }

    pub fn atan2f(&self, y: f32, x: f32) -> f32 {
        // SAFETY: pure function of its arguments and the injected tables.
        unsafe { oracle_atan2f(y, x) }
    }

    pub fn approach_s32(&self, current: i32, target: i32, inc: i32, dec: i32) -> i32 {
        // SAFETY: pure arithmetic.
        unsafe { oracle_approach_s32(current, target, inc, dec) }
    }

    pub fn approach_f32(&self, current: f32, target: f32, inc: f32, dec: f32) -> f32 {
        // SAFETY: pure arithmetic.
        unsafe { oracle_approach_f32(current, target, inc, dec) }
    }
}
