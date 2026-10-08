//! Development-only differential oracle. It runs the pinned decompilation's
//! collision loader and queries (vendored CC0 C, compiled natively by build.rs)
//! so tests can compare the Rust port bit for bit. The C keeps global state, so
//! every call goes through one process-wide lock.
//!
//! Boundary: this crate is never a dependency of the game runtime or renderer.
//! Replacement plan: once per-tick traces from original execution cover these
//! queries, those traces become the authority and this harness can be retired.
use std::sync::{Mutex, MutexGuard};

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
