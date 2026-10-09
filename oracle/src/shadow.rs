//! The original C helper output, before display-list construction. BOB only.
use crate::Oracle;
use rustario64::presentation::shadow::{Shadow, ShadowVertex};
use rustario64::{content::animation::Animation, presentation::mario::MarioPose};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Vertex {
    position: [i16; 3],
    uv: [i16; 2],
    alpha: u8,
}

unsafe extern "C" {
    fn oracle_shadow(
        position: *const f32,
        scale: i16,
        solidity: u8,
        animation: i16,
        frame: i16,
        vertices: *mut Vertex,
        layer: *mut u8,
    ) -> i32;
    fn oracle_shadow_origin(
        position: *const f32,
        yaw: i16,
        flags: i16,
        frame: i16,
        y_trans: i16,
        divisor: i16,
        index: *const u16,
        values: *const i16,
        child_scale: f32,
        out: *mut f32,
    );
}

impl Oracle {
    pub fn shadow_origin(
        &self,
        pose: &MarioPose,
        animation: &Animation,
        child_scale: f32,
    ) -> [f32; 3] {
        let pose_anim = pose.animation.expect("an animation pose");
        // Native table reads require a valid root attribute and frame.
        assert!(animation.index.len() >= 6 && pose_anim.frame >= 0);
        for pair in animation.index[..6].as_chunks::<2>().0 {
            assert!(
                pair[0] > 0
                    && usize::from(pair[0]) + usize::from(pair[1]) <= animation.values.len()
            );
        }
        let mut out = [0.0; 3];
        unsafe {
            oracle_shadow_origin(
                pose.position.as_ptr(),
                pose.angle[1],
                animation.flags,
                pose_anim.frame,
                pose_anim.y_trans,
                animation.y_trans_divisor,
                animation.index.as_ptr(),
                animation.values.as_ptr(),
                child_scale,
                out.as_mut_ptr(),
            )
        };
        out
    }
    pub fn player_shadow(
        &self,
        origin: [f32; 3],
        scale: i16,
        solidity: u8,
        animation: i16,
        frame: i16,
    ) -> Option<Shadow> {
        let mut vertices = [Vertex::default(); 9];
        let mut layer = 0;
        // Oracle holds the process-wide native-state lock for its lifetime.
        let present = unsafe {
            oracle_shadow(
                origin.as_ptr(),
                scale,
                solidity,
                animation,
                frame,
                vertices.as_mut_ptr(),
                &mut layer,
            )
        };
        (present != 0).then(|| Shadow {
            origin,
            layer,
            vertices: vertices.map(|v| ShadowVertex {
                position: v.position,
                uv: v.uv,
                alpha: v.alpha,
            }),
        })
    }
}
