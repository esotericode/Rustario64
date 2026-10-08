//! Minimal column-major f32 matrices for presentation only. Gameplay never uses
//! these: original math (sine tables, f32 operation order) belongs in simulation.

/// Column-major 4x4 matrix, matching WGSL `mat4x4<f32>` memory layout.
pub type Mat4 = [[f32; 4]; 4];

pub const IDENTITY: Mat4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];
    for (col, out_col) in out.iter_mut().enumerate() {
        for (row, value) in out_col.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[k][row] * b[col][k]).sum();
        }
    }
    out
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt();
    if length > 0.0 {
        [v[0] / length, v[1] / length, v[2] / length]
    } else {
        v
    }
}

/// Right-handed view matrix looking from `eye` toward `target`.
pub fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> Mat4 {
    let f = normalize(sub(target, eye));
    let s = normalize(cross(f, up));
    let u = cross(s, f);
    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0],
    ]
}

/// Right-handed perspective projection to wgpu's 0..1 depth range.
pub fn perspective(fov_y_degrees: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fov_y_degrees.to_radians() / 2.0).tan();
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, near * far / (near - far), 0.0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(m: &Mat4, v: [f32; 4]) -> [f32; 4] {
        let mut out = [0.0; 4];
        for (row, value) in out.iter_mut().enumerate() {
            *value = (0..4).map(|k| m[k][row] * v[k]).sum();
        }
        out
    }

    #[test]
    fn view_and_projection_map_near_and_far_planes_to_wgpu_depth() {
        let view = look_at([0.0, 0.0, 10.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let proj = perspective(45.0, 4.0 / 3.0, 1.0, 100.0);
        let vp = mul(&proj, &view);
        let near = apply(&vp, [0.0, 0.0, 9.0, 1.0]);
        let far = apply(&vp, [0.0, 0.0, -90.0, 1.0]);
        assert!((near[2] / near[3]).abs() < 1e-6);
        assert!((far[2] / far[3] - 1.0).abs() < 1e-5);
        let right = apply(&view, [1.0, 0.0, 0.0, 1.0]);
        assert!((right[0] - 1.0).abs() < 1e-6 && (right[2] + 10.0).abs() < 1e-6);
        assert_eq!(mul(&IDENTITY, &vp), vp);
    }
}
