//! Mario's model drawn from completed ticks, following the original render
//! pass (pinned CC0 src/game/rendering_graph_node.c and mario_misc.c).
//! geo_process_object places his object (position and angles, or the
//! floor-alignment matrix, then scale); geo_set_animation_globals selects how
//! the first animated part reads translation; the traversal runs his switch,
//! rotation and scale callbacks from his body state and applies each animated
//! part's animation values (geo_process_animated_part).
//!
//! Presentation reads the simulation and never writes it, so the callbacks'
//! own writes into the body state are not made: the torso and head angles are
//! drawn as zero where the callbacks would reset them, and the punch-scale
//! countdown runs here, from the tick on which a punch sets it.
//!
//! Display lists are built once per distinct draw list (the switch
//! configuration and level of detail), skinned on the CPU once per tick, and
//! interpolated between the last two ticks for each displayed frame.
use crate::{
    content::ImportIssue,
    content::{
        animation::{
            ANIM_FLAG_5, ANIM_FLAG_6, ANIM_FLAG_HOR_TRANS, ANIM_FLAG_VERT_TRANS, Animation,
            MarioAnimations,
        },
        visual::{LAYER_OPAQUE, LAYER_TRANSPARENT, SkinnedModel, VisualModel, VisualVertex},
    },
    import::{
        geo::{GRAPH_RENDER_ACTIVE, GeoNodeKind},
        gfx::{IDENTITY, Mat4, mat_mul},
        mario::{Draw, MarioCallback, MarioModelSource},
    },
    simulation::{
        mario::{MarioBodyState, MarioState, StepWorld, constants::*, tick::RenderedFrame},
        math::TrigTables,
    },
};
use std::collections::HashMap;

/// What drawing Mario reads from one completed tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarioPose {
    /// The render pass processed his object and it is not invisible.
    pub visible: bool,
    /// The floor-alignment matrix that replaced his position and angles.
    pub throw_matrix: Option<Mat4>,
    pub position: [f32; 3],
    pub angle: [i16; 3],
    pub scale: [f32; 3],
    /// The animation in the DMA buffer, with the frame the pass drew.
    pub animation: Option<AnimationPose>,
    pub body: MarioBodyState,
    pub area_update_counter: u16,
    /// The area camera is in C-Up mode, and the head rotation the camera
    /// gives Mario there (gPlayerCameraState's headRotation).
    pub camera_c_up: bool,
    pub head_rotation: [i16; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationPose {
    /// The animation table entry in the DMA buffer.
    pub entry: u16,
    pub frame: i16,
    pub y_trans: i16,
}

impl MarioPose {
    /// The pose after a tick whose render pass reported `rendered`.
    pub fn capture(m: &MarioState, w: &StepWorld<'_>, rendered: RenderedFrame) -> Self {
        let gfx = &m.obj.gfx;
        Self {
            visible: rendered.processed && gfx.node_flags & GRAPH_RENDER_INVISIBLE == 0,
            throw_matrix: rendered.throw_matrix.map(|i| w.floor_align_matrix[i]),
            position: gfx.pos,
            angle: gfx.angle,
            scale: gfx.scale,
            animation: gfx
                .anim
                .cur_anim
                .and(w.anim_dma_loaded)
                .map(|entry| AnimationPose {
                    entry,
                    frame: gfx.anim.anim_frame,
                    y_trans: gfx.anim.anim_y_trans,
                }),
            body: m.body,
            area_update_counter: w.area_update_counter,
            camera_c_up: i16::from(w.camera.mode) == CAMERA_MODE_C_UP,
            head_rotation: m.camera_status.head_rotation,
        }
    }
}

/// mtxf_rotate_zxy_and_translate.
fn rotate_zxy_and_translate(trig: &TrigTables, t: [f32; 3], r: [i16; 3]) -> Mat4 {
    let [rx, ry, rz] = r.map(i32::from);
    let (sx, cx) = (trig.sins(rx), trig.coss(rx));
    let (sy, cy) = (trig.sins(ry), trig.coss(ry));
    let (sz, cz) = (trig.sins(rz), trig.coss(rz));
    [
        [
            cy * cz + sx * sy * sz,
            cx * sz,
            -sy * cz + sx * cy * sz,
            0.0,
        ],
        [
            -cy * sz + sx * sy * cz,
            cx * cz,
            sy * sz + sx * cy * cz,
            0.0,
        ],
        [cx * sy, -sx, cx * cy, 0.0],
        [t[0], t[1], t[2], 1.0],
    ]
}

/// mtxf_rotate_xyz_and_translate.
fn rotate_xyz_and_translate(trig: &TrigTables, t: [f32; 3], r: [i16; 3]) -> Mat4 {
    let [rx, ry, rz] = r.map(i32::from);
    let (sx, cx) = (trig.sins(rx), trig.coss(rx));
    let (sy, cy) = (trig.sins(ry), trig.coss(ry));
    let (sz, cz) = (trig.sins(rz), trig.coss(rz));
    [
        [cy * cz, cy * sz, -sy, 0.0],
        [sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, sx * cy, 0.0],
        [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy, 0.0],
        [t[0], t[1], t[2], 1.0],
    ]
}

/// mtxf_scale_vec3f: scales the first three rows.
fn scale_rows(m: &Mat4, s: [f32; 3]) -> Mat4 {
    let mut out = *m;
    for (row, factor) in out.iter_mut().zip(s) {
        for value in row.iter_mut() {
            *value *= factor;
        }
    }
    out
}

/// gMarioBlinkAnimation.
const BLINK: [u8; 7] = [1, 2, 1, 0, 1, 2, 1];
/// gMarioAttackScaleAnimation: tenths, per hand/foot parameter and countdown.
const ATTACK_SCALE: [u8; 18] = [
    10, 12, 16, 24, 10, 10, 10, 14, 20, 30, 10, 10, 10, 16, 20, 26, 26, 20,
];

/// The punch-scale countdown that geo_mario_hand_foot_scaler keeps in the body
/// state: it starts when a tick sets a new punch state and drops by one per
/// drawn frame (one per tick at the original frame rate) until zero.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct PunchScale {
    seen: u8,
    current: u8,
}

impl PunchScale {
    fn advance(&mut self, punch_state: u8) {
        if punch_state != self.seen {
            self.seen = punch_state;
            self.current = punch_state;
        }
        if self.current & 0x3F > 0 {
            self.current -= 1;
        }
    }

    /// geo_mario_hand_foot_scaler's scale for parameter `part`.
    fn scale(&self, part: i16) -> f32 {
        if i32::from(part) != i32::from(self.current >> 6) {
            return 1.0;
        }
        let index = part as usize * 6 + usize::from(self.current & 0x3F);
        ATTACK_SCALE
            .get(index)
            .map_or(1.0, |&s| f32::from(s) / 10.0)
    }
}

/// geo_set_animation_globals' gCurrAnimType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnimType {
    Translation,
    LateralTranslation,
    VerticalTranslation,
    NoTranslation,
    Rotation,
}

struct AnimCursor<'a> {
    animation: &'a Animation,
    frame: i32,
    multiplier: f32,
    kind: AnimType,
    /// Attribute (index pair) to read next.
    attribute: usize,
}

impl AnimCursor<'_> {
    /// retrieve_animation_index and the value it selects. Reads the original
    /// would make outside the tables (a negative frame, more parts than
    /// attributes) draw as zero instead.
    fn next(&mut self) -> i16 {
        let attribute = self.attribute;
        self.attribute += 1;
        let index = &self.animation.index;
        let (Some(&frames), Some(&offset)) =
            (index.get(attribute * 2), index.get(attribute * 2 + 1))
        else {
            return 0;
        };
        let (frames, offset) = (i32::from(frames), i32::from(offset));
        let at = if self.frame < frames {
            offset + self.frame
        } else {
            offset + frames - 1
        };
        usize::try_from(at)
            .ok()
            .and_then(|at| self.animation.values.get(at))
            .copied()
            .unwrap_or(0)
    }
}

/// geo_process_shadow: the object's position plus lateral animation offset,
/// rotated by its yaw. The floor alignment matrix does not move the shadow.
pub fn shadow_origin(
    pose: &MarioPose,
    anims: &MarioAnimations,
    trig: &TrigTables,
    child_scale: f32,
) -> [f32; 3] {
    let mut position = pose.position;
    if let Some(a) = pose
        .animation
        .and_then(|a| anims.get(a.entry).map(|animation| (a, animation)))
    {
        let (a, animation) = a;
        if animation.flags & (ANIM_FLAG_5 | ANIM_FLAG_HOR_TRANS) == 0
            && (animation.flags & ANIM_FLAG_VERT_TRANS != 0 || animation.flags & ANIM_FLAG_6 == 0)
        {
            let mut cursor = AnimCursor {
                animation,
                frame: i32::from(a.frame),
                attribute: 0,
                kind: AnimType::Translation,
                multiplier: if animation.y_trans_divisor == 0 {
                    1.0
                } else {
                    f32::from(a.y_trans) / f32::from(animation.y_trans_divisor)
                },
            };
            let x = f32::from(cursor.next()) * cursor.multiplier * child_scale;
            cursor.attribute += 1;
            let z = f32::from(cursor.next()) * cursor.multiplier * child_scale;
            let (sin, cos) = (
                trig.sins(i32::from(pose.angle[1])),
                trig.coss(i32::from(pose.angle[1])),
            );
            position[0] += x * cos + z * sin;
            position[2] += -x * sin + z * cos;
        }
    }
    position
}

/// A callback's effect on the next sibling (node->next).
#[derive(Debug, Clone, Copy)]
enum Pending {
    Rotation([i16; 3]),
    /// Only the first rotation component; the rest keep the node's values.
    RotationX(i16),
    Scale(f32),
}

/// One render traversal of Mario's geo layout.
struct Walk<'a> {
    source: &'a MarioModelSource,
    trig: &'a TrigTables,
    pose: &'a MarioPose,
    /// Discrete geometry choices for both endpoints. A blink or LOD change
    /// selects today's mesh, but must not stop the skeleton's motion.
    selection: &'a MarioPose,
    punch: PunchScale,
    lod_distance: i16,
    anim: Option<AnimCursor<'a>>,
    /// World matrix per geo node, plus the object's at `object_bone`.
    matrices: Vec<Mat4>,
    draws: Vec<Draw>,
}

impl Walk<'_> {
    fn select(&self, role: Option<MarioCallback>, param: i16, children: usize) -> usize {
        let body = &self.selection.body;
        let case = match role {
            Some(MarioCallback::SwitchStandRun) => {
                usize::from(body.action & ACT_FLAG_STATIONARY == 0)
            }
            Some(MarioCallback::SwitchCapEffect) => (body.model_state >> 8) as usize,
            Some(MarioCallback::SwitchCapOnOff) => (body.cap_state & 1) as usize,
            Some(MarioCallback::SwitchEyes) => {
                if body.eye_state == 0 {
                    let frame = ((i32::from(param) * 32
                        + i32::from(self.selection.area_update_counter))
                        >> 1)
                        & 0x1F;
                    BLINK.get(frame as usize).copied().unwrap_or(0) as usize
                } else {
                    (body.eye_state - 1) as usize
                }
            }
            Some(MarioCallback::SwitchHand) => {
                let hand = body.hand_state;
                (if hand == MARIO_HAND_FISTS {
                    i8::from(body.action & ACT_FLAG_SWIMMING_OR_FLYING != 0)
                } else if param == 0 {
                    if hand < 5 { hand } else { MARIO_HAND_OPEN }
                } else if hand < 2 {
                    hand
                } else {
                    MARIO_HAND_FISTS
                }) as usize
            }
            // The initial case (none of Mario's switches lacks a callback).
            _ => 0,
        };
        case.min(children.saturating_sub(1))
    }

    /// A generated node's callback: its display list or its effect on the
    /// next sibling.
    fn generated(&mut self, role: Option<MarioCallback>, param: i16) -> Option<Pending> {
        let body = self.pose.body;
        match role? {
            MarioCallback::MirrorSetAlpha => {
                let alpha = if body.model_state & 0x100 != 0 {
                    (body.model_state & 0xFF) as u8
                } else {
                    255
                };
                let layer = if alpha == 255 {
                    LAYER_OPAQUE
                } else {
                    LAYER_TRANSPARENT
                };
                self.draws.push(Draw::EnvColor {
                    layer,
                    color: [255, 255, 255, alpha],
                });
                None
            }
            MarioCallback::TiltTorso => {
                let walking = matches!(
                    body.action,
                    ACT_BUTT_SLIDE | ACT_HOLD_BUTT_SLIDE | ACT_WALKING | ACT_RIDING_SHELL_GROUND
                );
                let t = if walking { body.torso_angle } else { [0; 3] };
                Some(Pending::Rotation([t[1], t[2], t[0]]))
            }
            MarioCallback::HeadRotation => {
                // In C-Up the head follows the camera's look. The original
                // leaves the node's middle angle as last written there; it is
                // drawn as zero, what the other branches leave it before C-Up.
                let rotation = if self.pose.camera_c_up {
                    let h = self.pose.head_rotation;
                    [h[1], 0, h[0]]
                } else if body.action & ACT_FLAG_WATER_OR_TEXT != 0 {
                    let h = body.head_angle;
                    [h[1], h[2], h[0]]
                } else {
                    [0; 3]
                };
                Some(Pending::Rotation(rotation))
            }
            MarioCallback::RotateWingCapWings => {
                let counter = i32::from(self.pose.area_update_counter);
                let rotation = if body.wing_flutter == 0 {
                    (self.trig.coss((counter & 0xF) << 12) + 1.0) * 4096.0
                } else {
                    (self.trig.coss((counter & 7) << 13) + 1.0) * 6144.0
                } as i16;
                Some(Pending::RotationX(if param & 1 == 0 {
                    rotation.wrapping_neg()
                } else {
                    rotation
                }))
            }
            MarioCallback::HandFootScaler => Some(Pending::Scale(self.punch.scale(param))),
            // The mirror's culling and held-object placement do nothing
            // without the mirror room or a held object.
            _ => None,
        }
    }

    /// geo_process_animated_part's translation and rotation.
    fn animated_part(&mut self, translation: [i16; 3]) -> ([f32; 3], [i16; 3]) {
        let mut t = translation.map(f32::from);
        let mut r = [0; 3];
        let Some(c) = self.anim.as_mut() else {
            return (t, r);
        };
        match c.kind {
            AnimType::Translation => {
                for value in &mut t {
                    *value += f32::from(c.next()) * c.multiplier;
                }
                c.kind = AnimType::Rotation;
            }
            AnimType::LateralTranslation => {
                t[0] += f32::from(c.next()) * c.multiplier;
                c.attribute += 1;
                t[2] += f32::from(c.next()) * c.multiplier;
                c.kind = AnimType::Rotation;
            }
            AnimType::VerticalTranslation => {
                c.attribute += 1;
                t[1] += f32::from(c.next()) * c.multiplier;
                c.attribute += 1;
                c.kind = AnimType::Rotation;
            }
            AnimType::NoTranslation => {
                c.attribute += 3;
                c.kind = AnimType::Rotation;
            }
            AnimType::Rotation => {}
        }
        r = [c.next(), c.next(), c.next()];
        (t, r)
    }

    /// geo_process_node_and_siblings over `nodes` (one node for a switch).
    fn siblings(&mut self, nodes: &[usize], parent: &Mat4, bone: u16) {
        let geo = &self.source.geo;
        // geo_switch_mario_cap_on_off sets every translation-rotation
        // sibling's active flag before they are drawn.
        let wings = nodes.iter().find_map(|&n| {
            (self.source.callback(n) == Some(MarioCallback::SwitchCapOnOff))
                .then_some(self.selection.body.cap_state & 2 != 0)
        });
        let mut pending = None;
        for &n in nodes {
            let node = &geo.nodes[n];
            let wing = matches!(node.kind, GeoNodeKind::TranslationRotation { .. });
            let active = if wing {
                wings.unwrap_or(node.flags & GRAPH_RENDER_ACTIVE != 0)
            } else {
                node.flags & GRAPH_RENDER_ACTIVE != 0
            };
            let effect = pending.take();
            if active {
                pending = self.node(n, parent, bone, effect);
            }
        }
    }

    fn display_list(&mut self, layer: u8, address: Option<u32>, bone: u16) {
        if let Some(address) = address {
            self.draws.push(Draw::List {
                layer,
                address,
                bone,
            });
        }
    }

    /// One node; returns a callback's effect on the next sibling.
    fn node(
        &mut self,
        n: usize,
        parent: &Mat4,
        bone: u16,
        effect: Option<Pending>,
    ) -> Option<Pending> {
        let source = self.source;
        let node = &source.geo.nodes[n];
        let children = &node.children;
        let this = n as u16;
        match node.kind {
            GeoNodeKind::SwitchCase { num_cases, .. } => {
                let case = self.select(source.callback(n), num_cases, children.len());
                if let Some(&child) = children.get(case) {
                    self.siblings(&[child], parent, bone);
                }
            }
            GeoNodeKind::Generated { param, .. } => {
                let effect = self.generated(source.callback(n), param);
                self.siblings(children, parent, bone);
                return effect;
            }
            GeoNodeKind::LevelOfDetail {
                min_distance,
                max_distance,
            } => {
                if min_distance <= self.lod_distance && self.lod_distance < max_distance {
                    self.siblings(children, parent, bone);
                }
            }
            GeoNodeKind::Scale {
                layer,
                scale,
                display_list,
            } => {
                let factor = match effect {
                    Some(Pending::Scale(factor)) => factor,
                    _ => scale as f32 / 65536.0,
                };
                let m = scale_rows(parent, [factor; 3]);
                self.matrices[n] = m;
                self.display_list(layer, display_list, this);
                self.siblings(children, &m, this);
            }
            GeoNodeKind::Rotation {
                layer,
                rotation,
                display_list,
            } => {
                let rotation = match effect {
                    Some(Pending::Rotation(r)) => r,
                    Some(Pending::RotationX(x)) => [x, rotation[1], rotation[2]],
                    _ => rotation,
                };
                let m = mat_mul(
                    &rotate_zxy_and_translate(self.trig, [0.0; 3], rotation),
                    parent,
                );
                self.matrices[n] = m;
                self.display_list(layer, display_list, this);
                self.siblings(children, &m, this);
            }
            GeoNodeKind::TranslationRotation {
                layer,
                translation,
                rotation,
                display_list,
            } => {
                let local =
                    rotate_zxy_and_translate(self.trig, translation.map(f32::from), rotation);
                let m = mat_mul(&local, parent);
                self.matrices[n] = m;
                self.display_list(layer, display_list, this);
                self.siblings(children, &m, this);
            }
            GeoNodeKind::Translation {
                layer,
                translation,
                display_list,
            } => {
                let local = rotate_zxy_and_translate(self.trig, translation.map(f32::from), [0; 3]);
                let m = mat_mul(&local, parent);
                self.matrices[n] = m;
                self.display_list(layer, display_list, this);
                self.siblings(children, &m, this);
            }
            GeoNodeKind::AnimatedPart {
                layer,
                translation,
                display_list,
            } => {
                let (t, r) = self.animated_part(translation);
                let m = mat_mul(&rotate_xyz_and_translate(self.trig, t, r), parent);
                self.matrices[n] = m;
                self.display_list(layer, display_list, this);
                self.siblings(children, &m, this);
            }
            GeoNodeKind::DisplayList {
                layer,
                display_list,
            } => {
                self.display_list(layer, Some(display_list), bone);
                self.siblings(children, parent, bone);
            }
            // Held objects are drawn by their own objects; there are none yet.
            GeoNodeKind::HeldObject { .. } => {}
            // Grouping, shadow (not drawn yet) and the rest: children only.
            _ => self.siblings(children, parent, bone),
        }
        None
    }
}

/// One tick's skinned result.
#[derive(Debug, Clone)]
struct Posed {
    build: usize,
    /// World-space vertices per batch of the build.
    vertices: Vec<Vec<VisualVertex>>,
    epoch: u64,
    pose: MarioPose,
    punch: PunchScale,
}

/// What to draw for one displayed frame.
pub struct MarioFrame<'a> {
    /// Stable per distinct draw list; the build's materials and textures.
    pub build: usize,
    pub template: &'a VisualModel,
    /// World-space vertices per batch of `template`.
    pub vertices: Vec<Vec<VisualVertex>>,
}

/// Builds, poses and interpolates Mario's model from tick poses.
pub struct MarioDrawer<'a> {
    source: &'a MarioModelSource,
    trig: &'a TrigTables,
    anims: &'a MarioAnimations,
    keys: HashMap<Vec<Draw>, usize>,
    builds: Vec<SkinnedModel>,
    /// Issues reported while building, with the draw list's build index.
    pub issues: Vec<(usize, ImportIssue)>,
    punch: PunchScale,
    epoch: u64,
    previous: Option<Posed>,
    current: Option<Posed>,
}

/// A unit normal as the signed bytes a lit vertex carries in its color.
fn quantize_normal(n: [f32; 3], color: &mut [u8; 4]) {
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if length > 0.0 {
        for (byte, value) in color.iter_mut().zip(n) {
            *byte = ((value / length * 127.0).round() as i8) as u8;
        }
    }
}

/// Skin a build with a traversal's matrices: positions and lit normals.
fn skin(build: &SkinnedModel, matrices: &[Mat4]) -> Vec<Vec<VisualVertex>> {
    build
        .model
        .batches
        .iter()
        .zip(&build.bones)
        .map(|(batch, bones)| {
            let lit = batch.material.lights.is_some();
            batch
                .vertices
                .iter()
                .zip(bones)
                .map(|(v, &bone)| {
                    let m = matrices.get(usize::from(bone)).unwrap_or(&IDENTITY);
                    let [x, y, z] = v.position;
                    let position = [
                        x * m[0][0] + y * m[1][0] + z * m[2][0] + m[3][0],
                        x * m[0][1] + y * m[1][1] + z * m[2][1] + m[3][1],
                        x * m[0][2] + y * m[1][2] + z * m[2][2] + m[3][2],
                    ];
                    let mut color = v.color;
                    if lit {
                        let [nx, ny, nz] = [0, 1, 2].map(|i| f32::from(v.color[i] as i8));
                        let n = [
                            nx * m[0][0] + ny * m[1][0] + nz * m[2][0],
                            nx * m[0][1] + ny * m[1][1] + nz * m[2][1],
                            nx * m[0][2] + ny * m[1][2] + nz * m[2][2],
                        ];
                        quantize_normal(n, &mut color);
                    }
                    VisualVertex {
                        position,
                        uv: v.uv,
                        color,
                    }
                })
                .collect()
        })
        .collect()
}

fn lerp_vertices(
    previous: &[Vec<VisualVertex>],
    current: &[Vec<VisualVertex>],
    alpha: f32,
    lit: impl Fn(usize) -> bool,
) -> Vec<Vec<VisualVertex>> {
    previous
        .iter()
        .zip(current)
        .enumerate()
        .map(|(b, (p, c))| {
            p.iter()
                .zip(c)
                .map(|(p, c)| {
                    let mut v = *c;
                    for i in 0..3 {
                        v.position[i] = p.position[i] + (c.position[i] - p.position[i]) * alpha;
                    }
                    if lit(b) {
                        let n: [f32; 3] = [0, 1, 2].map(|i| {
                            let (a, b) = (f32::from(p.color[i] as i8), f32::from(c.color[i] as i8));
                            a + (b - a) * alpha
                        });
                        quantize_normal(n, &mut v.color);
                    }
                    v
                })
                .collect()
        })
        .collect()
}

impl<'a> MarioDrawer<'a> {
    pub fn new(
        source: &'a MarioModelSource,
        trig: &'a TrigTables,
        anims: &'a MarioAnimations,
    ) -> Self {
        Self {
            source,
            trig,
            anims,
            keys: HashMap::new(),
            builds: vec![],
            issues: vec![],
            punch: PunchScale::default(),
            epoch: 0,
            previous: None,
            current: None,
        }
    }

    /// Forget the previous ticks; the next frames snap (a new level entry).
    pub fn reset(&mut self) {
        self.epoch += 1;
        self.previous = None;
        self.current = None;
        self.punch = PunchScale::default();
    }

    /// Resume from the displayed tick rather than briefly blending backwards.
    pub fn snap(&mut self) {
        self.previous = None;
    }

    /// The draw list and matrices of a render traversal for `pose`, with the
    /// level-of-detail distance (the depth of Mario's origin in front of the
    /// camera; None draws full detail).
    pub fn traverse(&self, pose: &MarioPose, lod_distance: Option<i16>) -> (Vec<Draw>, Vec<Mat4>) {
        self.traverse_selected(pose, pose, self.punch, lod_distance)
    }

    fn traverse_selected(
        &self,
        pose: &MarioPose,
        selection: &MarioPose,
        punch: PunchScale,
        lod_distance: Option<i16>,
    ) -> (Vec<Draw>, Vec<Mat4>) {
        let source = self.source;
        let mut walk = Walk {
            source,
            trig: self.trig,
            pose,
            selection,
            punch,
            lod_distance: lod_distance.unwrap_or(0),
            anim: pose.animation.and_then(|a| {
                let animation = self.anims.get(a.entry)?;
                let flags = animation.flags;
                Some(AnimCursor {
                    animation,
                    frame: i32::from(a.frame),
                    multiplier: if animation.y_trans_divisor == 0 {
                        1.0
                    } else {
                        f32::from(a.y_trans) / f32::from(animation.y_trans_divisor)
                    },
                    kind: if flags & ANIM_FLAG_HOR_TRANS != 0 {
                        AnimType::VerticalTranslation
                    } else if flags & ANIM_FLAG_VERT_TRANS != 0 {
                        AnimType::LateralTranslation
                    } else if flags & ANIM_FLAG_6 != 0 {
                        AnimType::NoTranslation
                    } else {
                        AnimType::Translation
                    },
                    attribute: 0,
                })
            }),
            matrices: vec![IDENTITY; source.geo.nodes.len() + 1],
            draws: vec![],
        };
        let base = pose
            .throw_matrix
            .unwrap_or_else(|| rotate_zxy_and_translate(self.trig, pose.position, pose.angle));
        let object = scale_rows(&base, pose.scale);
        let bone = source.object_bone();
        walk.matrices[usize::from(bone)] = object;
        if let Some(root) = source.geo.root {
            walk.siblings(&[root], &object, bone);
        }
        (walk.draws, walk.matrices)
    }

    /// Pose one completed tick. Building a new draw list's display lists can
    /// fail on malformed data; the error leaves the previous frames in place.
    pub fn update(&mut self, pose: &MarioPose, lod_distance: Option<i16>) -> Result<(), String> {
        self.punch.advance(pose.body.punch_state);
        if !pose.visible {
            self.previous = self.current.take();
            return Ok(());
        }
        let (draws, matrices) = self.traverse(pose, lod_distance);
        let build = match self.keys.get(&draws) {
            Some(&build) => build,
            None => {
                let (model, issues) = self.source.build(&draws).map_err(|e| e.to_string())?;
                let build = self.builds.len();
                self.builds.push(model);
                self.issues.extend(issues.into_iter().map(|i| (build, i)));
                self.keys.insert(draws, build);
                build
            }
        };
        // Use the newly selected geometry at both endpoints. In particular,
        // blink display lists differ, although the head keeps moving. Traversing
        // the previous pose with today's switches also initializes bones that
        // were inactive in yesterday's mesh (hands, cap, LOD, stand/run).
        let mut previous = self.current.take();
        if let Some(old) = previous.as_mut()
            && old.build != build
        {
            let (_, previous_matrices) =
                self.traverse_selected(&old.pose, pose, old.punch, lod_distance);
            old.vertices = skin(&self.builds[build], &previous_matrices);
            old.build = build;
        }
        self.previous = previous;
        self.current = Some(Posed {
            build,
            vertices: skin(&self.builds[build], &matrices),
            epoch: self.epoch,
            pose: *pose,
            punch: self.punch,
        });
        Ok(())
    }

    /// The model between the last two ticks in the same epoch. Clip changes
    /// are completed poses too: snapping them skips a displayed midpoint for
    /// the whole model, including its position, while the camera keeps moving.
    /// Geometry switches select the current mesh at both endpoints, never
    /// unrelated vertex arrays. Reset, hidden poses and disabled interpolation
    /// still draw without blending.
    pub fn frame(&self, alpha: f32, interpolation: bool) -> Option<MarioFrame<'_>> {
        let current = self.current.as_ref()?;
        let template = &self.builds[current.build].model;
        let vertices = match &self.previous {
            Some(previous)
                if interpolation
                    && alpha.is_finite()
                    && previous.build == current.build
                    && previous.epoch == current.epoch =>
            {
                lerp_vertices(
                    &previous.vertices,
                    &current.vertices,
                    alpha.clamp(0.0, 1.0),
                    |b| template.batches[b].material.lights.is_some(),
                )
            }
            _ => current.vertices.clone(),
        };
        Some(MarioFrame {
            build: current.build,
            template,
            vertices,
        })
    }

    /// The number of distinct draw lists built so far.
    pub fn builds(&self) -> usize {
        self.builds.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punch_scale_counts_down_once_per_tick_from_a_new_state() {
        let mut punch = PunchScale::default();
        // The first punch sets (0 << 6) | 4: the right hand grows, then shrinks.
        let mut scales = vec![];
        for _ in 0..5 {
            punch.advance(4);
            scales.push(punch.scale(0));
        }
        assert_eq!(scales, [2.4, 1.6, 1.2, 1.0, 1.0]);
        // Other parts stay at full size; a kick switches to the foot.
        assert_eq!(punch.scale(1), 1.0);
        punch.advance((2 << 6) | 6);
        assert_eq!(punch.scale(2), 2.0);
    }

    use crate::{
        content::animation::Animation,
        import::{mario::MarioCallback, segments::Segments},
        simulation::math::{ARCTAN_ENTRIES, SINE_ENTRIES},
    };
    use std::collections::BTreeMap;

    const ALPHA: u32 = 0x8000_0001;
    const STAND_RUN: u32 = 0x8000_0002;
    const TILT: u32 = 0x8000_0003;
    const DL: u32 = 0x0400_0030;

    fn words(out: &mut Vec<u8>, words: &[u32]) {
        for w in words {
            out.extend_from_slice(&w.to_be_bytes());
        }
    }

    /// An authored skeleton: a stand/run switch choosing a plain display list
    /// (standing) or a root part with a torso tilt and a child part 100 units
    /// along +X (moving), under a 0.5 scale. The display list is one lit
    /// triangle with an up normal.
    fn source() -> MarioModelSource {
        let mut dl = vec![];
        for p in [[0i16, 0, 0], [10, 0, 0], [0, 10, 0]] {
            for c in p {
                dl.extend_from_slice(&c.to_be_bytes());
            }
            dl.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0x00, 0x7F, 0x00, 0xFF]);
        }
        words(
            &mut dl,
            &[
                0x0420_0030,
                0x0400_0000,
                0xBF00_0000,
                0x0000_0A14,
                0xB800_0000,
                0,
            ],
        );
        let commands: [&[u32]; 20] = [
            &[0x1600_0000, 0x0096_0064], // GEO_SHADOW
            &[0x0400_0000],              // GEO_OPEN_NODE
            &[0x1D00_0000, 0x0000_8000], // GEO_SCALE(0, 0.5)
            &[0x0400_0000],
            &[0x1800_0000, ALPHA],     // GEO_ASM(0, set alpha)
            &[0x0E00_0000, STAND_RUN], // GEO_SWITCH_CASE(0, stand/run)
            &[0x0400_0000],
            &[0x1501_0000, DL],   // GEO_DISPLAY_LIST(1, DL): standing
            &[0x1301_0000, 0, 0], // GEO_ANIMATED_PART(1, 0, 0, 0, NULL): moving
            &[0x0400_0000],
            &[0x1800_0000, TILT], // GEO_ASM(0, tilt torso)
            &[0x1200_0000, 0],    // GEO_ROTATION_NODE(0, 0, 0, 0)
            &[0x0400_0000],
            &[0x1301_0064, 0, DL], // GEO_ANIMATED_PART(1, 100, 0, 0, DL)
            &[0x0500_0000],        // GEO_CLOSE_NODE
            &[0x0500_0000],
            &[0x0500_0000],
            &[0x0500_0000],
            &[0x0500_0000],
            &[0x0100_0000], // GEO_END
        ];
        let mut geo = vec![];
        for command in commands {
            words(&mut geo, command);
        }
        let mut segments = Segments::default();
        segments.insert(0x04, dl).unwrap();
        segments.insert(0x17, geo).unwrap();
        let callbacks = BTreeMap::from([
            (ALPHA, MarioCallback::MirrorSetAlpha),
            (STAND_RUN, MarioCallback::SwitchStandRun),
            (TILT, MarioCallback::TiltTorso),
        ]);
        MarioModelSource::from_segments(segments, 0x1700_0000, callbacks, vec![]).unwrap()
    }

    fn tables() -> TrigTables {
        let sine = (0..SINE_ENTRIES)
            .map(|i| (i as f64 * std::f64::consts::TAU / 4096.0).sin() as f32)
            .collect();
        TrigTables::new(sine, vec![0; ARCTAN_ENTRIES]).unwrap()
    }

    /// Root translation (5, 7, 9) and no rotation; the child turns a quarter
    /// about Y.
    fn animations(flags: i16) -> MarioAnimations {
        MarioAnimations {
            animations: vec![Animation {
                flags,
                y_trans_divisor: 0,
                start_frame: 0,
                loop_start: 0,
                loop_end: 1,
                bone_count: 2,
                index: vec![1, 0, 1, 1, 1, 2, 1, 3, 1, 3, 1, 3, 1, 3, 1, 4, 1, 3],
                values: vec![5, 7, 9, 0, 0x4000],
            }],
        }
    }

    fn pose(action: u32, torso: [i16; 3], position: [f32; 3]) -> MarioPose {
        MarioPose {
            visible: true,
            throw_matrix: None,
            position,
            angle: [0; 3],
            scale: [1.0; 3],
            animation: Some(AnimationPose {
                entry: 0,
                frame: 0,
                y_trans: 0,
            }),
            body: MarioBodyState {
                action,
                torso_angle: torso,
                ..MarioBodyState::default()
            },
            area_update_counter: 0,
            camera_c_up: false,
            head_rotation: [0; 3],
        }
    }

    /// The posed second vertex, (10, 0, 0) in the display list, and the
    /// first vertex's normal bytes.
    fn posed(drawer: &mut MarioDrawer<'_>, pose: &MarioPose) -> ([f32; 3], [u8; 3]) {
        drawer.reset();
        drawer.update(pose, None).unwrap();
        let frame = drawer.frame(1.0, true).unwrap();
        let v = &frame.vertices[0];
        (v[1].position, [v[0].color[0], v[0].color[1], v[0].color[2]])
    }

    fn assert_near(actual: [f32; 3], expected: [f32; 3]) {
        for i in 0..3 {
            assert!(
                (actual[i] - expected[i]).abs() < 1e-3,
                "{actual:?} vs {expected:?}"
            );
        }
    }

    #[test]
    fn traversal_poses_parts_from_the_animation_and_callbacks() {
        let source = source();
        let trig = tables();
        let anims = animations(0);
        let mut drawer = MarioDrawer::new(&source, &trig, &anims);
        let p = [1000.0, 2000.0, 3000.0];
        // Standing: the switch draws the list with the scale node's matrix,
        // after the generated env color, and reads no animation.
        let standing = pose(ACT_IDLE, [0; 3], p);
        let (draws, _) = drawer.traverse(&standing, None);
        assert_eq!(
            draws,
            [
                Draw::EnvColor {
                    layer: LAYER_OPAQUE,
                    color: [255; 4],
                },
                Draw::List {
                    layer: 1,
                    address: DL,
                    bone: 1,
                },
            ]
        );
        assert_near(posed(&mut drawer, &standing).0, [1005.0, 2000.0, 3000.0]);
        // Moving: root translation (5, 7, 9), the child's quarter turn, then
        // the 0.5 scale. A jump zeroes the torso tilt...
        let jump = pose(ACT_JUMP, [0, 0x4000, 0], p);
        let (position, normal) = posed(&mut drawer, &jump);
        assert_near(position, [1052.5, 2003.5, 2999.5]);
        assert_eq!(normal, [0, 127, 0]);
        // ...walking applies it: torso[1] turns the rotation node about X,
        // which also turns the normal from +Y to +Z.
        let walking = pose(ACT_WALKING, [0, 0x4000, 0], p);
        let (position, normal) = posed(&mut drawer, &walking);
        assert_near(position, [1052.5, 2008.5, 3004.5]);
        assert_eq!(normal, [0, 0, 127]);
        // ANIM_FLAG_HOR_TRANS reads only the vertical translation.
        let anims = animations(ANIM_FLAG_HOR_TRANS);
        let mut vertical = MarioDrawer::new(&source, &trig, &anims);
        assert_near(posed(&mut vertical, &jump).0, [1050.0, 2003.5, 2995.0]);
        // ANIM_FLAG_6 reads no translation.
        let anims = animations(ANIM_FLAG_6);
        let mut none = MarioDrawer::new(&source, &trig, &anims);
        assert_near(posed(&mut none, &jump).0, [1050.0, 2000.0, 2995.0]);
    }

    #[test]
    fn frames_interpolate_between_ticks_of_one_build() {
        let source = source();
        let trig = tables();
        let anims = animations(0);
        let mut drawer = MarioDrawer::new(&source, &trig, &anims);
        let a = pose(ACT_JUMP, [0; 3], [0.0; 3]);
        let b = pose(ACT_JUMP, [0; 3], [30.0, 0.0, 0.0]);
        drawer.update(&a, None).unwrap();
        drawer.update(&b, None).unwrap();
        let at = |drawer: &MarioDrawer<'_>, alpha, interpolation| {
            drawer.frame(alpha, interpolation).unwrap().vertices[0][1].position
        };
        assert_near(at(&drawer, 0.5, true), [67.5, 3.5, -0.5]);
        assert_near(at(&drawer, 0.5, false), [82.5, 3.5, -0.5]);
        // A geometry switch uses the new geometry at the previous transform.
        drawer
            .update(&pose(ACT_IDLE, [0; 3], [0.0; 3]), None)
            .unwrap();
        assert_near(at(&drawer, 0.0, true), [35.0, 0.0, 0.0]);
        assert_near(at(&drawer, 0.5, true), [20.0, 0.0, 0.0]);
        assert_eq!(drawer.builds(), 2);
        // Not drawn: no frame.
        let mut hidden = b;
        hidden.visible = false;
        drawer.update(&hidden, None).unwrap();
        assert!(drawer.frame(1.0, true).is_none());
    }

    #[test]
    fn material_and_animation_switches_interpolate_but_reset_snaps() {
        let source = source();
        let trig = tables();
        let mut anims = animations(0);
        anims.animations.push(anims.animations[0].clone());
        anims.animations[1].values = vec![25, 47, -31, 0, 0];
        let mut drawer = MarioDrawer::new(&source, &trig, &anims);
        let a = pose(ACT_JUMP, [0; 3], [0.0; 3]);
        let mut b = pose(ACT_JUMP, [0; 3], [30.0, 0.0, 0.0]);
        // MirrorSetAlpha changes the draw-list key without changing its bones.
        b.body.model_state = 0x180;
        drawer.update(&a, None).unwrap();
        drawer.update(&b, None).unwrap();
        assert_eq!(drawer.builds(), 2);
        let at = |d: &MarioDrawer<'_>, alpha| d.frame(alpha, true).unwrap().vertices[0][1].position;
        assert_near(at(&drawer, 0.5), [67.5, 3.5, -0.5]);
        b.animation.as_mut().unwrap().entry = 1;
        b.position[0] = 60.0;
        drawer.update(&b, None).unwrap();
        // Changing clips is a new completed pose, not a world discontinuity.
        // Both the object's movement and the different joint pose interpolate.
        assert_near(at(&drawer, 0.0), [82.5, 3.5, -0.5]);
        assert_near(at(&drawer, 0.25), [93.75, 8.5, -4.25]);
        assert_near(at(&drawer, 0.5), [105.0, 13.5, -8.0]);
        assert_near(at(&drawer, 1.0), [127.5, 23.5, -15.5]);
        assert_near(
            drawer.frame(0.0, false).unwrap().vertices[0][1].position,
            [127.5, 23.5, -15.5],
        );
        assert_near(at(&drawer, f32::NAN), [127.5, 23.5, -15.5]);
        // A clip switch and a material switch can occur on the same tick.
        drawer.update(&a, None).unwrap();
        assert_near(at(&drawer, 0.0), [127.5, 23.5, -15.5]);
        assert_near(at(&drawer, 0.5), [90.0, 13.5, -8.0]);
        // A clip switch with a different skeleton branch still uses only
        // today's geometry. Its earlier endpoint is posed at the old origin.
        drawer.update(&b, None).unwrap();
        drawer
            .update(&pose(ACT_IDLE, [0; 3], [-30.0, 0.0, 0.0]), None)
            .unwrap();
        assert_near(at(&drawer, 0.0), [65.0, 0.0, 0.0]);
        assert_near(at(&drawer, 0.5), [20.0, 0.0, 0.0]);
        assert_near(at(&drawer, 1.0), [-25.0, 0.0, 0.0]);
        drawer.reset();
        drawer.update(&a, None).unwrap();
        assert_near(at(&drawer, 0.0), [52.5, 3.5, -0.5]);
    }

    #[test]
    fn matrix_helpers_follow_the_original_layouts() {
        let sine = (0..crate::simulation::math::SINE_ENTRIES)
            .map(|i| (i as f64 * std::f64::consts::TAU / 4096.0).sin() as f32)
            .collect();
        let arctan = vec![0; crate::simulation::math::ARCTAN_ENTRIES];
        let trig = TrigTables::new(sine, arctan).unwrap();
        // A quarter turn about Y takes +Z to +X (yaw 0x4000 faces +X).
        let m = rotate_zxy_and_translate(&trig, [1.0, 2.0, 3.0], [0, 0x4000, 0]);
        let z = [m[2][0], m[2][1], m[2][2]];
        assert!((z[0] - 1.0).abs() < 1e-6 && z[1].abs() < 1e-6 && z[2].abs() < 1e-6);
        assert_eq!(m[3], [1.0, 2.0, 3.0, 1.0]);
        // The XYZ order differs from ZXY once two axes turn.
        let a = rotate_xyz_and_translate(&trig, [0.0; 3], [0x1000, 0x2000, 0x3000]);
        let b = rotate_zxy_and_translate(&trig, [0.0; 3], [0x1000, 0x2000, 0x3000]);
        assert!((a[0][1] - b[0][1]).abs() > 0.1);
        let s = scale_rows(&IDENTITY, [0.25; 3]);
        assert_eq!(s[0][0], 0.25);
        assert_eq!(s[3], [0.0, 0.0, 0.0, 1.0]);
    }
}
