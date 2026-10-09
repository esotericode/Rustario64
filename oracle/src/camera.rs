//! Safe, locked adapters to the verbatim camera.c component excerpts.
use crate::Oracle;
use rustario64::simulation::{
    camera::{PlayerGeometry, RadialState},
    collision::CollisionFlags,
    mario::constants::ACT_FLAG_ON_POLE,
    math::TrigTables,
};

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum CameraFloat {
    AsymptoticBool,
    Asymptotic,
    SetOrApproach,
    SymmetricBool,
    Symmetric,
}

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum CameraAngle {
    AsymptoticBool,
    Asymptotic,
    SymmetricBool,
    Symmetric,
    SetOrApproach,
}

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum CameraVector {
    AnglesAndDistances,
    RotateXz,
    RotateYz,
    ScaleLine,
    Approach,
    SetOrApproach,
    InBounds,
    ClampPitch,
    AreaBounds,
}

#[repr(C)]
struct RadialSetup {
    mario: [f32; 3],
    action: u32,
    center: [f32; 2],
    area: i32,
    offset_yaw: i16,
    lakitu_dist: i16,
    lakitu_pitch: i16,
    unused: i16,
    floor_height: f32,
}

unsafe extern "C" {
    fn oracle_camera_float(
        which: i32,
        current: f32,
        target: f32,
        amount: f32,
        status: i16,
        out: *mut f32,
    ) -> i32;
    fn oracle_camera_angle(
        which: i32,
        current: i16,
        target: i16,
        amount: i16,
        status: i16,
        out: *mut i16,
    ) -> i32;
    fn oracle_camera_vectors(
        which: i32,
        a: *const f32,
        b: *const f32,
        amount: *const f32,
        angle: i16,
        status: i16,
        out: *mut u32,
    );
    fn oracle_camera_vec3s(current: *mut i16, target: *const i16, divisor: *const i16);
    fn oracle_camera_buttons(current: u16, pressed: u16, down: u16) -> i32;
    fn oracle_camera_geometry(pos: *const f32, for_camera: i16, intangible: i16, out: *mut u32);
    fn oracle_camera_collision(
        which: i32,
        pos: *mut f32,
        offset: f32,
        radius: f32,
        for_camera: i16,
        intangible: i16,
        flags: *mut u32,
    ) -> i32;
    fn oracle_camera_radial(
        input: *const RadialSetup,
        for_camera: i16,
        intangible: i16,
        out: *mut u32,
    );
}

/// Owns the existing process-wide C lock, collision terrain, and trig tables.
/// The wrapped Oracle is private so no other adapter can alter its state.
pub struct CameraOracle {
    _oracle: Oracle,
}

impl CameraOracle {
    pub fn new(stream: &[i16], tables: &TrigTables) -> Self {
        let oracle = Oracle::load(stream);
        oracle.set_trig(tables.sine_table(), tables.arctan_table());
        Self { _oracle: oracle }
    }

    pub fn float(
        &self,
        which: CameraFloat,
        current: f32,
        target: f32,
        amount: f32,
        status: i16,
    ) -> (f32, bool) {
        let mut out = 0.0;
        // SAFETY: scalar arguments, valid output, and the process-wide lock.
        let result =
            unsafe { oracle_camera_float(which as i32, current, target, amount, status, &mut out) };
        (out, result != 0)
    }

    pub fn angle(
        &self,
        which: CameraAngle,
        current: i16,
        target: i16,
        amount: i16,
        status: i16,
    ) -> (i16, bool) {
        let mut out = 0;
        // SAFETY: scalar arguments, valid output, and the process-wide lock.
        let result =
            unsafe { oracle_camera_angle(which as i32, current, target, amount, status, &mut out) };
        (out, result != 0)
    }

    /// Exact native words: vector components as bits, and the return word.
    /// AnglesAndDistances instead returns pitch, yaw, full and horizontal distances.
    pub fn vectors(
        &self,
        which: CameraVector,
        a: [f32; 3],
        b: [f32; 3],
        amount: [f32; 3],
        angle: i16,
        status: i16,
    ) -> [u32; 4] {
        let mut out = [0; 4];
        // SAFETY: all arrays have the fixed lengths C reads/writes; locked.
        unsafe {
            oracle_camera_vectors(
                which as i32,
                a.as_ptr(),
                b.as_ptr(),
                amount.as_ptr(),
                angle,
                status,
                out.as_mut_ptr(),
            )
        };
        out
    }

    pub fn vec3s(&self, mut current: [i16; 3], target: [i16; 3], divisor: [i16; 3]) -> [i16; 3] {
        // SAFETY: three-element arrays, valid output, process-wide lock.
        unsafe { oracle_camera_vec3s(current.as_mut_ptr(), target.as_ptr(), divisor.as_ptr()) };
        current
    }

    pub fn buttons(&self, current: u16, pressed: u16, down: u16) -> u16 {
        // SAFETY: scalar arguments, process-wide lock.
        unsafe { oracle_camera_buttons(current, pressed, down) as u16 }
    }

    pub fn geometry(&self, pos: [f32; 3], flags: CollisionFlags) -> [u32; 9] {
        let mut out = [0; 9];
        // SAFETY: fixed-size arrays, loaded terrain, process-wide lock.
        unsafe {
            oracle_camera_geometry(
                pos.as_ptr(),
                flags.checking_for_camera.into(),
                flags.find_floor_include_surface_intangible.into(),
                out.as_mut_ptr(),
            )
        };
        out
    }

    /// Native wall correction; the owner terrain is static, with no vanish cap.
    pub fn walls(
        &self,
        pos: [f32; 3],
        offset: f32,
        radius: f32,
        flags: CollisionFlags,
    ) -> ([f32; 3], i32, [u32; 2]) {
        self.collision(0, pos, offset, radius, flags)
    }

    pub fn resolve_geometry(&self, pos: [f32; 3], flags: CollisionFlags) -> ([f32; 3], [u32; 2]) {
        let (pos, _, flags) = self.collision(1, pos, 0.0, 100.0, flags);
        (pos, flags)
    }

    fn collision(
        &self,
        which: i32,
        mut pos: [f32; 3],
        offset: f32,
        radius: f32,
        flags: CollisionFlags,
    ) -> ([f32; 3], i32, [u32; 2]) {
        let mut out = [0; 2];
        // SAFETY: fixed-size arrays, loaded terrain, process-wide lock.
        let count = unsafe {
            oracle_camera_collision(
                which,
                pos.as_mut_ptr(),
                offset,
                radius,
                flags.checking_for_camera.into(),
                flags.find_floor_include_surface_intangible.into(),
                out.as_mut_ptr(),
            )
        };
        (pos, count, out)
    }

    pub fn radial(
        &self,
        state: RadialState,
        mario: [f32; 3],
        action: u32,
        geometry: PlayerGeometry,
        flags: CollisionFlags,
    ) -> [u32; 10] {
        assert_eq!(
            action & ACT_FLAG_ON_POLE,
            0,
            "native pole path needs objects"
        );
        let input = RadialSetup {
            mario,
            action,
            center: [state.center_x, state.center_z],
            area: state.area,
            offset_yaw: state.mode_offset_yaw,
            lakitu_dist: state.lakitu_dist,
            lakitu_pitch: state.lakitu_pitch,
            unused: 0,
            floor_height: geometry.floor_height,
        };
        let mut out = [0; 10];
        // SAFETY: matched repr(C) record, fixed output, supported action, lock.
        unsafe {
            oracle_camera_radial(
                &input,
                flags.checking_for_camera.into(),
                flags.find_floor_include_surface_intangible.into(),
                out.as_mut_ptr(),
            )
        };
        out
    }
}
