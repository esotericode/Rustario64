// Presentation shader for imported Fast3D batches. It evaluates the RDP color
// combiner selectors from the material (G_CCMUX / G_ACMUX numbering in the
// pinned decompilation's gbi.h), Fast3D-style one-light vertex shading, and
// gsSPFogPosition fog. Output never feeds gameplay state.

struct Frame {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    // x: fog near, y: fog far (original frustum), z: 1 when the target is sRGB,
    // w: 1 when fog is enabled in the graphics options.
    params: vec4<f32>,
};

struct Material {
    rgb0: vec4<u32>,
    alpha0: vec4<u32>,
    rgb1: vec4<u32>,
    alpha1: vec4<u32>,
    prim: vec4<f32>,
    env: vec4<f32>,
    fog_color: vec4<f32>,
    ambient: vec4<f32>,
    diffuse: vec4<f32>,
    light_dir: vec4<f32>,
    // x: flag bits (see FLAG_*).
    flags: vec4<u32>,
    // x: fog multiplier, y: fog offset.
    fog: vec4<f32>,
    // Model placement: xyz translation, w yaw in radians (0 faces +Z).
    transform: vec4<f32>,
};

const FLAG_LIT: u32 = 1u;
const FLAG_TEXTURED: u32 = 2u;
const FLAG_TWO_CYCLE: u32 = 4u;
const FLAG_FOG: u32 = 8u;
const FLAG_CUTOUT: u32 = 16u;
const FLAG_TEXGEN: u32 = 32u;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var texture0: texture_2d<f32>;
@group(1) @binding(2) var sampler0: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<u32>,
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) shade: vec4<f32>,
    @location(2) fog: f32,
};

fn signed_byte(v: u32) -> f32 {
    return f32(i32(v) - select(0, 256, v > 127u));
}

// Rotate about +Y so that model +Z turns toward (sin yaw, 0, cos yaw).
fn yaw_rotate(v: vec3<f32>, yaw: f32) -> vec3<f32> {
    let s = sin(yaw);
    let c = cos(yaw);
    return vec3<f32>(v.x * c + v.z * s, v.y, v.z * c - v.x * s);
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world = yaw_rotate(input.position, material.transform.w) + material.transform.xyz;
    let view_position = frame.view * vec4<f32>(world, 1.0);
    out.clip = frame.projection * view_position;
    out.uv = input.uv;
    let flags = material.flags.x;
    if (flags & FLAG_LIT) != 0u {
        let normal_model = vec3<f32>(
            signed_byte(input.color.x), signed_byte(input.color.y), signed_byte(input.color.z));
        let normal_world = yaw_rotate(normal_model, material.transform.w);
        let normal = normalize((frame.view * vec4<f32>(normal_world, 0.0)).xyz);
        // SM64 keeps the view matrix in the modelview stack, so light directions
        // are effectively camera-space.
        let light = normalize(material.light_dir.xyz);
        let lit = material.ambient.rgb + material.diffuse.rgb * max(dot(normal, light), 0.0);
        out.shade = vec4<f32>(min(lit, vec3<f32>(1.0)), f32(input.color.w) / 255.0);
        if (flags & FLAG_TEXGEN) != 0u {
            out.uv = normal.xy * 0.5 + vec2<f32>(0.5);
        }
    } else {
        out.shade = vec4<f32>(input.color) / 255.0;
    }
    // Fog alpha = clamp(z_ndc * multiplier + offset, 0, 255), with z_ndc from an
    // OpenGL-style [-1, 1] projection of the original near/far planes.
    let near = frame.params.x;
    let far = frame.params.y;
    let depth = max(-view_position.z, 1.0);
    let z_ndc = (far + near) / (far - near) - 2.0 * far * near / ((far - near) * depth);
    out.fog = clamp(z_ndc * material.fog.x + material.fog.y, 0.0, 255.0) / 255.0;
    return out;
}

struct Inputs {
    combined: vec4<f32>,
    texel0: vec4<f32>,
    texel1: vec4<f32>,
    prim: vec4<f32>,
    shade: vec4<f32>,
    env: vec4<f32>,
};

fn rgb_a(s: u32, i: Inputs) -> vec3<f32> {
    switch s {
        case 0u: { return i.combined.rgb; }
        case 1u: { return i.texel0.rgb; }
        case 2u: { return i.texel1.rgb; }
        case 3u: { return i.prim.rgb; }
        case 4u: { return i.shade.rgb; }
        case 5u: { return i.env.rgb; }
        case 6u: { return vec3<f32>(1.0); }
        case 7u: { return vec3<f32>(0.5); } // noise
        default: { return vec3<f32>(0.0); }
    }
}

fn rgb_b(s: u32, i: Inputs) -> vec3<f32> {
    switch s {
        case 0u: { return i.combined.rgb; }
        case 1u: { return i.texel0.rgb; }
        case 2u: { return i.texel1.rgb; }
        case 3u: { return i.prim.rgb; }
        case 4u: { return i.shade.rgb; }
        case 5u: { return i.env.rgb; }
        default: { return vec3<f32>(0.0); } // key center, K4, zero
    }
}

fn rgb_c(s: u32, i: Inputs) -> vec3<f32> {
    switch s {
        case 0u: { return i.combined.rgb; }
        case 1u: { return i.texel0.rgb; }
        case 2u: { return i.texel1.rgb; }
        case 3u: { return i.prim.rgb; }
        case 4u: { return i.shade.rgb; }
        case 5u: { return i.env.rgb; }
        case 7u: { return vec3<f32>(i.combined.a); }
        case 8u: { return vec3<f32>(i.texel0.a); }
        case 9u: { return vec3<f32>(i.texel1.a); }
        case 10u: { return vec3<f32>(i.prim.a); }
        case 11u: { return vec3<f32>(i.shade.a); }
        case 12u: { return vec3<f32>(i.env.a); }
        default: { return vec3<f32>(0.0); } // key scale, LOD fractions, K5, zero
    }
}

fn rgb_d(s: u32, i: Inputs) -> vec3<f32> {
    switch s {
        case 0u: { return i.combined.rgb; }
        case 1u: { return i.texel0.rgb; }
        case 2u: { return i.texel1.rgb; }
        case 3u: { return i.prim.rgb; }
        case 4u: { return i.shade.rgb; }
        case 5u: { return i.env.rgb; }
        case 6u: { return vec3<f32>(1.0); }
        default: { return vec3<f32>(0.0); }
    }
}

fn alpha_abd(s: u32, i: Inputs) -> f32 {
    switch s {
        case 0u: { return i.combined.a; }
        case 1u: { return i.texel0.a; }
        case 2u: { return i.texel1.a; }
        case 3u: { return i.prim.a; }
        case 4u: { return i.shade.a; }
        case 5u: { return i.env.a; }
        case 6u: { return 1.0; }
        default: { return 0.0; }
    }
}

fn alpha_c(s: u32, i: Inputs) -> f32 {
    switch s {
        case 1u: { return i.texel0.a; }
        case 2u: { return i.texel1.a; }
        case 3u: { return i.prim.a; }
        case 4u: { return i.shade.a; }
        case 5u: { return i.env.a; }
        default: { return 0.0; } // LOD fractions, zero
    }
}

fn combine(rgb: vec4<u32>, alpha: vec4<u32>, i: Inputs) -> vec4<f32> {
    let color = (rgb_a(rgb.x, i) - rgb_b(rgb.y, i)) * rgb_c(rgb.z, i) + rgb_d(rgb.w, i);
    let a = (alpha_abd(alpha.x, i) - alpha_abd(alpha.y, i)) * alpha_c(alpha.z, i)
        + alpha_abd(alpha.w, i);
    return clamp(vec4<f32>(color, a), vec4<f32>(0.0), vec4<f32>(1.0));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let flags = material.flags.x;
    var texel = vec4<f32>(1.0);
    if (flags & FLAG_TEXTURED) != 0u {
        texel = textureSample(texture0, sampler0, input.uv);
    }
    var inputs = Inputs(vec4<f32>(0.0), texel, texel, material.prim, input.shade, material.env);
    var color = combine(material.rgb0, material.alpha0, inputs);
    if (flags & FLAG_TWO_CYCLE) != 0u {
        inputs.combined = color;
        color = combine(material.rgb1, material.alpha1, inputs);
    }
    if (flags & FLAG_CUTOUT) != 0u && color.a < 0.5 {
        discard;
    }
    if (flags & FLAG_FOG) != 0u && frame.params.w > 0.5 {
        color = vec4<f32>(mix(color.rgb, material.fog_color.rgb, input.fog), color.a);
    }
    if frame.params.z > 0.5 {
        // Original colors are display-referred; undo the target's sRGB encoding.
        color = vec4<f32>(pow(color.rgb, vec3<f32>(2.2)), color.a);
    }
    return color;
}
