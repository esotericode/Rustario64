//! Mario's object node in the render pass, translated from pinned CC0
//! src/game/rendering_graph_node.c (geo_process_object, the node processors
//! his model reaches, geo_process_held_object) and src/game/mario_misc.c (his
//! geo callbacks).
//!
//! The pass is authoritative for Mario: it advances his animation, his
//! callbacks write his body state (geo_mario_tilt_torso and
//! geo_mario_head_rotation reset the torso and head angles outside the
//! actions that keep them, geo_mario_hand_foot_scaler counts the punch
//! state down) and geo_switch_mario_hand_grab_pos writes the held object's
//! last position (HOLP), which dropping and throwing read. That position
//! comes from the camera-space matrix stack, so the traversal keeps the
//! original matrix arithmetic, levels of detail (from the 16.16 fixed-point
//! matrix the original reads) and switch selections. Callbacks also write
//! graph nodes (rotation and scale nodes, the held-object node's offset,
//! the wings' active flags); those writes persist in [`MarioGraphState`] as
//! they do in the original nodes. Display lists are not built here; the
//! shadow node changes nothing the simulation keeps, so only its children
//! are processed.
use super::{
    MarioState, ObjectId, StepWorld, animation::update_animation_frame, constants::*,
    tick::RenderedFrame,
};
use crate::{
    content::animation::{
        ANIM_FLAG_6, ANIM_FLAG_HOR_TRANS, ANIM_FLAG_VERT_TRANS, Animation, MarioAnimations,
        ObjectAnimations,
    },
    import::geo::{GRAPH_RENDER_ACTIVE as NODE_ACTIVE, GeoLayout, GeoNode, GeoNodeKind},
    simulation::{
        math::{
            Mat4, TrigTables, fixed_point_integer, get_pos_from_transform_mtx, mtxf_billboard,
            mtxf_mul, mtxf_rotate_xyz_and_translate, mtxf_rotate_zxy_and_translate,
            mtxf_scale_vec3f, mtxf_translate,
        },
        object::{
            AnimInfo, AnimRef, Object, ThrowMatrix, object,
            render::{ObjectModels, VisibleObject, obj_is_in_view, process_model, selected_cases},
        },
    },
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Mario's native geo callbacks (pinned src/game/mario_misc.c), as roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum MarioCallback {
    /// geo_mirror_mario_backface_culling: only acts for the castle mirror's Mario.
    MirrorBackfaceCulling,
    /// geo_mirror_mario_set_alpha: the vanish cap's alpha and layer.
    MirrorSetAlpha,
    /// geo_switch_mario_stand_run: full detail while stationary, LOD otherwise.
    SwitchStandRun,
    /// geo_switch_mario_cap_effect: normal, vanish, metal or metal-vanish body.
    SwitchCapEffect,
    /// geo_switch_mario_cap_on_off: cap on or off, and the wings' visibility.
    SwitchCapOnOff,
    /// geo_switch_mario_eyes: blinking or a fixed eye state.
    SwitchEyes,
    /// geo_switch_mario_hand: fists, open, peace sign, holding a cap.
    SwitchHand,
    /// geo_mario_head_rotation: the head turn while reading or in water.
    HeadRotation,
    /// geo_mario_tilt_torso: the torso tilt while walking and sliding.
    TiltTorso,
    /// geo_mario_rotate_wing_cap_wings: the wings' flap.
    RotateWingCapWings,
    /// geo_mario_hand_foot_scaler: the punch and kick scale.
    HandFootScaler,
    /// geo_move_mario_part_from_parent: positions an object Mario carries
    /// through his prevObj (the burning-Mario flames; none in the port).
    MovePartFromParent,
    /// geo_switch_mario_hand_grab_pos: where a held object sits, and the HOLP.
    HandGrabPos,
}

/// MODEL_MARIO's graph as the simulation needs it: the decoded geo layout
/// (the main scripts' `mario_geo`, or an authored stand-in) and the roles
/// of its native callbacks by address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarioModel {
    pub layout: GeoLayout,
    pub callbacks: BTreeMap<u32, MarioCallback>,
}

impl MarioModel {
    /// The callback role of a SwitchCase, Generated or HeldObject node.
    pub fn role(&self, node: usize) -> Option<MarioCallback> {
        let address = match self.layout.nodes[node].kind {
            GeoNodeKind::SwitchCase { callback, .. }
            | GeoNodeKind::Generated { callback, .. }
            | GeoNodeKind::HeldObject { callback, .. } => callback,
            _ => return None,
        };
        self.callbacks.get(&address).copied()
    }
}

/// What Mario's callbacks wrote into his graph nodes, which persist from one
/// frame to the next as the original nodes do, plus the scaler's
/// function-local counter. A fresh level load rebuilds the nodes from the
/// layout (empty maps); the counter is the static's boot value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarioGraphState {
    /// GraphNodeRotation.rotation, by node.
    pub rotations: BTreeMap<usize, [i16; 3]>,
    /// GraphNodeScale.scale, by node.
    pub scales: BTreeMap<usize, f32>,
    /// GraphNodeHeldObject.translation, by node.
    pub held_translations: BTreeMap<usize, [i16; 3]>,
    /// GRAPH_RENDER_ACTIVE of the wing nodes geo_switch_mario_cap_on_off sets.
    pub active: BTreeMap<usize, bool>,
    /// geo_mario_hand_foot_scaler's sMarioAttackAnimCounter.
    pub attack_anim_counter: i16,
}

/// What the camera node above GEO_RENDER_OBJ gives the objects' pass: its
/// transform (`object::render::camera_matrix`), the perspective node's field
/// of view, and the mode of the area's camera (geo_mario_head_rotation
/// reads `gCurGraphNodeCamera->config.camera->mode`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderView {
    pub camera: Mat4,
    pub fov: f32,
    pub camera_mode: u8,
}

/// gMarioAttackScaleAnimation.
const ATTACK_SCALE: [i8; 18] = [
    10, 12, 16, 24, 10, 10, 10, 14, 20, 30, 10, 10, 10, 16, 20, 26, 26, 20,
];
/// gMarioBlinkAnimation.
const BLINK: [i8; 7] = [1, 2, 1, 0, 1, 2, 1];

const ANIM_TYPE_NONE: u8 = 0;
const ANIM_TYPE_TRANSLATION: u8 = 1;
const ANIM_TYPE_VERTICAL_TRANSLATION: u8 = 2;
const ANIM_TYPE_LATERAL_TRANSLATION: u8 = 3;
const ANIM_TYPE_NO_TRANSLATION: u8 = 4;
const ANIM_TYPE_ROTATION: u8 = 5;

/// The gCurrAnim* globals geo_set_animation_globals sets.
#[derive(Clone, Copy)]
struct AnimGlobals<'a> {
    kind: u8,
    frame: i16,
    multiplier: f32,
    /// The next index pair (gCurrAnimAttribute).
    attribute: usize,
    data: Option<&'a Animation>,
}

impl AnimGlobals<'_> {
    /// gCurrAnimData[retrieve_animation_index(gCurrAnimFrame, &gCurrAnimAttribute)].
    fn next(&mut self) -> i16 {
        let data = self
            .data
            .expect("an animated part reads animation data with none set");
        let value = data.value(self.attribute, i32::from(self.frame));
        self.attribute += 1;
        value
    }
}

/// The animation `anim.curAnim` points to, borrowed for the world's
/// lifetime rather than the world's borrow.
fn resolve<'a>(
    anim: &AnimInfo,
    anims: &'a MarioAnimations,
    dma: Option<u16>,
    object_anims: &'a ObjectAnimations,
) -> &'a Animation {
    match anim.cur_anim {
        Some(AnimRef::MarioDmaBuffer) => anims
            .get(dma.expect("curAnim points at a DMA buffer that was never loaded"))
            .expect("DMA buffer entry outside the animation table"),
        Some(AnimRef::Object(address)) => object_anims
            .get(address)
            .unwrap_or_else(|| panic!("curAnim 0x{address:08X} was not imported")),
        None => panic!("curAnim is NULL (the original dereferences it)"),
    }
}

/// geo_set_animation_globals: advance `o`'s frame (once per area update)
/// and set the globals the animated parts read.
fn geo_set_animation_globals<'a>(
    o: &mut Object,
    w: &StepWorld<'_>,
    anims: &'a MarioAnimations,
    object_anims: &'a ObjectAnimations,
) -> AnimGlobals<'a> {
    update_animation_frame(o, w);
    let anim = resolve(&o.gfx.anim, anims, w.anim_dma_loaded, object_anims);
    let kind = if anim.flags & ANIM_FLAG_HOR_TRANS != 0 {
        ANIM_TYPE_VERTICAL_TRANSLATION
    } else if anim.flags & ANIM_FLAG_VERT_TRANS != 0 {
        ANIM_TYPE_LATERAL_TRANSLATION
    } else if anim.flags & ANIM_FLAG_6 != 0 {
        ANIM_TYPE_NO_TRANSLATION
    } else {
        ANIM_TYPE_TRANSLATION
    };
    // gCurrAnimEnabled (ANIM_FLAG_5) only matters to the shadow node.
    AnimGlobals {
        kind,
        frame: o.gfx.anim.anim_frame,
        multiplier: if anim.y_trans_divisor == 0 {
            1.0
        } else {
            f32::from(o.gfx.anim.anim_y_trans) / f32::from(anim.y_trans_divisor)
        },
        attribute: 0,
        data: Some(anim),
    }
}

/// One traversal of Mario's model.
struct Pass<'p, 'a> {
    model: &'p MarioModel,
    view: &'p RenderView,
    trig: &'a TrigTables,
    models: &'a ObjectModels,
    anims: &'a MarioAnimations,
    object_anims: &'a ObjectAnimations,
    anim: AnimGlobals<'a>,
    /// Mario's object matrix (`*gCurGraphNodeObject->throwMatrix`).
    object_matrix: Mat4,
    /// The object drawn in his hand this frame.
    held: Option<ObjectId>,
}

impl Pass<'_, '_> {
    /// geo_process_node_and_siblings over a parent's children.
    fn children(&mut self, m: &mut MarioState, w: &mut StepWorld<'_>, node: usize, top: &Mat4) {
        let model = self.model;
        let children = &model.layout.nodes[node].children;
        for i in 0..children.len() {
            let child = children[i];
            let next = children[(i + 1) % children.len()];
            self.node(m, w, child, next, top);
        }
    }

    fn active(&self, w: &StepWorld<'_>, n: usize, node: &GeoNode) -> bool {
        w.mario_graph
            .active
            .get(&n)
            .copied()
            .unwrap_or(node.flags & NODE_ACTIVE != 0)
    }

    /// One node of geo_process_node_and_siblings; `next` is its next sibling.
    fn node(
        &mut self,
        m: &mut MarioState,
        w: &mut StepWorld<'_>,
        n: usize,
        next: usize,
        top: &Mat4,
    ) {
        let model = self.model;
        let node = &model.layout.nodes[n];
        if !self.active(w, n, node) {
            return;
        }
        if node.flags & GRAPH_RENDER_CHILDREN_FIRST as u16 != 0 {
            self.children(m, w, n, top);
            return;
        }
        let trig = self.trig;
        match node.kind {
            GeoNodeKind::LevelOfDetail {
                min_distance,
                max_distance,
            } => {
                // The integer part of the fixed-point stack top's z.
                let distance = (-i32::from(fixed_point_integer(top[3][2]))) as i16;
                if min_distance <= distance && distance < max_distance {
                    self.children(m, w, n, top);
                }
            }
            GeoNodeKind::SwitchCase { num_cases, .. } => {
                let selected = self.switch_case(m, w, n, num_cases);
                // geo_process_switch walks the circular sibling list.
                let children = &node.children;
                if !children.is_empty() {
                    let index = if selected > 0 {
                        selected as usize % children.len()
                    } else {
                        0
                    };
                    // Only the selected child is processed; its next
                    // sibling is still the following case.
                    let child = children[index];
                    let next = children[(index + 1) % children.len()];
                    self.node(m, w, child, next, top);
                }
            }
            GeoNodeKind::TranslationRotation {
                translation,
                rotation,
                ..
            } => {
                let local =
                    mtxf_rotate_zxy_and_translate(trig, translation.map(f32::from), rotation);
                self.children(m, w, n, &mtxf_mul(&local, top));
            }
            GeoNodeKind::Translation { translation, .. } => {
                let local = mtxf_rotate_zxy_and_translate(trig, translation.map(f32::from), [0; 3]);
                self.children(m, w, n, &mtxf_mul(&local, top));
            }
            GeoNodeKind::Rotation { rotation, .. } => {
                let rotation = w.mario_graph.rotations.get(&n).copied().unwrap_or(rotation);
                let local = mtxf_rotate_zxy_and_translate(trig, [0.0; 3], rotation);
                self.children(m, w, n, &mtxf_mul(&local, top));
            }
            GeoNodeKind::Scale { scale, .. } => {
                // geo_layout's cur_geo_cmd_u32(0x04) / 65536.0f.
                let scale = w
                    .mario_graph
                    .scales
                    .get(&n)
                    .copied()
                    .unwrap_or(scale as f32 / 65536.0);
                self.children(m, w, n, &mtxf_scale_vec3f(top, [scale; 3]));
            }
            GeoNodeKind::AnimatedPart { translation, .. } => {
                let matrix = self.animated_part(translation, top);
                self.children(m, w, n, &matrix);
            }
            GeoNodeKind::Generated { param, .. } => {
                self.generated(m, w, n, next, param);
                self.children(m, w, n, top);
            }
            GeoNodeKind::HeldObject { param, offset, .. } => {
                self.held_object(m, w, n, param, offset, top);
                self.children(m, w, n, top);
            }
            // Start, display lists, culling radius and the shadow (whose
            // animation reads leave the attribute where they found it and
            // whose writes are drawing state): their children only.
            GeoNodeKind::Start
            | GeoNodeKind::DisplayList { .. }
            | GeoNodeKind::CullingRadius { .. }
            | GeoNodeKind::Shadow { .. } => self.children(m, w, n, top),
            ref other => panic!("Mario's model reaches a {other:?} node, which is not audited"),
        }
    }

    /// geo_process_animated_part.
    fn animated_part(&mut self, translation: [i16; 3], top: &Mat4) -> Mat4 {
        let a = &mut self.anim;
        let mut t = translation.map(f32::from);
        let mut r = [0i16; 3];
        match a.kind {
            ANIM_TYPE_TRANSLATION => {
                t[0] += f32::from(a.next()) * a.multiplier;
                t[1] += f32::from(a.next()) * a.multiplier;
                t[2] += f32::from(a.next()) * a.multiplier;
                a.kind = ANIM_TYPE_ROTATION;
            }
            ANIM_TYPE_LATERAL_TRANSLATION => {
                t[0] += f32::from(a.next()) * a.multiplier;
                a.attribute += 1;
                t[2] += f32::from(a.next()) * a.multiplier;
                a.kind = ANIM_TYPE_ROTATION;
            }
            ANIM_TYPE_VERTICAL_TRANSLATION => {
                a.attribute += 1;
                t[1] += f32::from(a.next()) * a.multiplier;
                a.attribute += 1;
                a.kind = ANIM_TYPE_ROTATION;
            }
            ANIM_TYPE_NO_TRANSLATION => {
                a.attribute += 3;
                a.kind = ANIM_TYPE_ROTATION;
            }
            _ => {}
        }
        if a.kind == ANIM_TYPE_ROTATION {
            r = [a.next(), a.next(), a.next()];
        }
        mtxf_mul(&mtxf_rotate_xyz_and_translate(self.trig, t, r), top)
    }

    /// A switch node's callback: the case it selects.
    fn switch_case(
        &mut self,
        m: &mut MarioState,
        w: &mut StepWorld<'_>,
        n: usize,
        num_cases: i16,
    ) -> i16 {
        let role = self.model.role(n);
        // The callbacks index gBodyStates by numCases; Mario's are 0.
        let body_index = || {
            assert!(
                num_cases == 0,
                "a Mario switch reads gBodyStates[{num_cases}] (Luigi's)"
            );
        };
        let body = &m.body;
        match role {
            Some(MarioCallback::SwitchStandRun) => {
                body_index();
                i16::from(body.action & ACT_FLAG_STATIONARY == 0)
            }
            Some(MarioCallback::SwitchCapEffect) => {
                body_index();
                body.model_state >> 8
            }
            Some(MarioCallback::SwitchCapOnOff) => {
                body_index();
                let on = body.cap_state & 2 != 0;
                // Every translation-rotation sibling's active flag.
                let parent = self
                    .model
                    .layout
                    .nodes
                    .iter()
                    .position(|p| p.children.contains(&n));
                if let Some(parent) = parent {
                    for &sibling in &self.model.layout.nodes[parent].children {
                        if sibling != n
                            && matches!(
                                self.model.layout.nodes[sibling].kind,
                                GeoNodeKind::TranslationRotation { .. }
                            )
                        {
                            w.mario_graph.active.insert(sibling, on);
                        }
                    }
                }
                i16::from(body.cap_state) & 1
            }
            Some(MarioCallback::SwitchEyes) => {
                body_index();
                if body.eye_state == 0 {
                    let frame = ((i32::from(num_cases) * 32 + i32::from(w.area_update_counter))
                        >> 1)
                        & 0x1F;
                    if frame < 7 {
                        i16::from(BLINK[frame as usize])
                    } else {
                        0
                    }
                } else {
                    i16::from(body.eye_state) - 1
                }
            }
            Some(MarioCallback::SwitchHand) => {
                // gBodyStates[0]; numCases is the hand (0 right, 1 left).
                let hand = body.hand_state;
                i16::from(if hand == MARIO_HAND_FISTS {
                    i8::from(body.action & ACT_FLAG_SWIMMING_OR_FLYING != 0)
                } else if num_cases == 0 {
                    if hand < 5 { hand } else { MARIO_HAND_OPEN }
                } else if hand < 2 {
                    hand
                } else {
                    MARIO_HAND_FISTS
                })
            }
            other => panic!("Mario switch node {n} has callback {other:?}, which is not audited"),
        }
    }

    /// A generated node's callback (its effect on the next sibling or on the
    /// body state). None of them builds a display list outside the mirror.
    fn generated(
        &mut self,
        m: &mut MarioState,
        w: &mut StepWorld<'_>,
        n: usize,
        next: usize,
        param: i16,
    ) {
        let graph = &mut w.mario_graph;
        match self.model.role(n) {
            // gCurGraphNodeObject is never the mirror's Mario here.
            Some(MarioCallback::MirrorBackfaceCulling | MarioCallback::MirrorSetAlpha) => {}
            Some(MarioCallback::MovePartFromParent) => {
                assert!(
                    m.obj.prev_obj.is_none(),
                    "geo_move_mario_part_from_parent moves Mario's prevObj, which is not ported"
                );
            }
            Some(MarioCallback::TiltTorso) => {
                assert!(
                    param == 0,
                    "geo_mario_tilt_torso reads gBodyStates[{param}]"
                );
                let body = &mut m.body;
                if !matches!(
                    body.action,
                    ACT_BUTT_SLIDE | ACT_HOLD_BUTT_SLIDE | ACT_WALKING | ACT_RIDING_SHELL_GROUND
                ) {
                    body.torso_angle = [0; 3];
                }
                let t = body.torso_angle;
                graph.rotations.insert(next, [t[1], t[2], t[0]]);
            }
            Some(MarioCallback::HeadRotation) => {
                assert!(
                    param == 0,
                    "geo_mario_head_rotation reads gBodyStates[{param}]"
                );
                let body = &mut m.body;
                if i16::from(self.view.camera_mode) == CAMERA_MODE_C_UP {
                    let head = m.camera_status.head_rotation;
                    let rotation = graph.rotations.entry(next).or_insert_with(|| {
                        match self.model.layout.nodes[next].kind {
                            GeoNodeKind::Rotation { rotation, .. } => rotation,
                            _ => [0; 3],
                        }
                    });
                    rotation[0] = head[1];
                    rotation[2] = head[0];
                } else if body.action & ACT_FLAG_WATER_OR_TEXT != 0 {
                    let h = body.head_angle;
                    graph.rotations.insert(next, [h[1], h[2], h[0]]);
                } else {
                    body.head_angle = [0; 3];
                    graph.rotations.insert(next, [0; 3]);
                }
            }
            Some(MarioCallback::RotateWingCapWings) => {
                let index = param >> 1;
                assert!(
                    index == 0,
                    "geo_mario_rotate_wing_cap_wings reads gBodyStates[{index}]"
                );
                let counter = i32::from(w.area_update_counter);
                let rot_x = if m.body.wing_flutter == 0 {
                    (self.trig.coss((counter & 0xF) << 12) + 1.0) * 4096.0
                } else {
                    (self.trig.coss((counter & 7) << 13) + 1.0) * 6144.0
                } as i16;
                let rotation = graph.rotations.entry(next).or_insert_with(|| {
                    match self.model.layout.nodes[next].kind {
                        GeoNodeKind::Rotation { rotation, .. } => rotation,
                        _ => [0; 3],
                    }
                });
                rotation[0] = if param & 1 == 0 {
                    rot_x.wrapping_neg()
                } else {
                    rot_x
                };
            }
            Some(MarioCallback::HandFootScaler) => {
                // gBodyStates[0]; the parameter is the hand or foot.
                let body = &mut m.body;
                let mut scale = 1.0;
                if i32::from(param) == i32::from(body.punch_state >> 6) {
                    if graph.attack_anim_counter != w.area_update_counter as i16
                        && body.punch_state & 0x3F > 0
                    {
                        body.punch_state -= 1;
                        graph.attack_anim_counter = w.area_update_counter as i16;
                    }
                    let index = param as usize * 6 + usize::from(body.punch_state & 0x3F);
                    let tenths = *ATTACK_SCALE.get(index).unwrap_or_else(|| {
                        panic!("gMarioAttackScaleAnimation[{index}] reads past the table")
                    });
                    scale = f32::from(tenths) / 10.0;
                }
                graph.scales.insert(next, scale);
            }
            other => {
                panic!("Mario generated node {n} has callback {other:?}, which is not audited")
            }
        }
    }

    /// geo_process_held_object with geo_switch_mario_hand_grab_pos.
    fn held_object(
        &mut self,
        m: &mut MarioState,
        w: &mut StepWorld<'_>,
        n: usize,
        player: u8,
        offset: [i16; 3],
        top: &Mat4,
    ) {
        assert!(
            self.model.role(n) == Some(MarioCallback::HandGrabPos),
            "a held-object node without geo_switch_mario_hand_grab_pos"
        );
        assert!(player == 0, "a held-object node of player {player} (Luigi)");
        // GEO_CONTEXT_RENDER: which object, and where it sits in the hand.
        let Some(held) = m.held_obj else {
            return;
        };
        let translation = match m.body.grab_pos {
            GRAB_POS_LIGHT_OBJ if m.action & ACT_FLAG_THROWING != 0 => Some([50, 0, 0]),
            GRAB_POS_LIGHT_OBJ => Some([50, 0, 110]),
            GRAB_POS_HEAVY_OBJ => Some([145, -173, 180]),
            GRAB_POS_BOWSER => Some([80, -270, 1260]),
            _ => None,
        };
        if let Some(translation) = translation {
            w.mario_graph.held_translations.insert(n, translation);
        }
        let translation = w
            .mario_graph
            .held_translations
            .get(&n)
            .copied()
            .unwrap_or(offset);
        let o = object(&w.objects, &m.obj, held);
        let Some(model) = o.gfx.shared_child else {
            return;
        };
        let mat = mtxf_translate(translation.map(|v| f32::from(v) / 4.0));
        let mut matrix = self.object_matrix;
        matrix[3][0] = top[3][0];
        matrix[3][1] = top[3][1];
        matrix[3][2] = top[3][2];
        let matrix = mtxf_scale_vec3f(&mtxf_mul(&mat, &matrix), o.gfx.scale);
        // GEO_CONTEXT_HELD_OBJ: the HOLP, in world coordinates.
        m.body.held_obj_last_position = get_pos_from_transform_mtx(&matrix, &self.view.camera);
        // The held object's own subtree, with its own animation globals.
        let saved = self.anim;
        self.anim.kind = ANIM_TYPE_NONE;
        let mut o = std::mem::take(w.objects.slot_mut(held));
        if o.gfx.anim.cur_anim.is_some() {
            self.anim = geo_set_animation_globals(&mut o, w, self.anims, self.object_anims);
        }
        let traversal = self
            .models
            .traversal(model)
            .unwrap_or_else(|| panic!("model {model}'s render traversal was not imported"));
        process_model(&mut o, traversal, traversal.root);
        *w.objects.slot_mut(held) = o;
        self.anim = saved;
        self.held = Some(held);
    }
}

/// geo_process_object for Mario's node under the camera node, with his
/// model's traversal when he is in view and his model is loaded.
pub fn render_mario(m: &mut MarioState, w: &mut StepWorld<'_>, view: &RenderView) -> RenderedFrame {
    let throw_matrix = match m.obj.gfx.throw_matrix {
        Some(ThrowMatrix::FloorAlign(index)) => Some(index),
        Some(ThrowMatrix::Terrain(_)) => {
            panic!("Mario's throw matrix is always a floor-align matrix")
        }
        None => None,
    };
    if m.obj.gfx.node_flags & GRAPH_RENDER_ACTIVE == 0 {
        m.obj.gfx.throw_matrix = None;
        return RenderedFrame::default();
    }
    if m.obj.gfx.area_index != w.area_index {
        return RenderedFrame::default();
    }
    let trig = w.trig;
    let placed = match throw_matrix {
        Some(index) => mtxf_mul(&w.floor_align_matrix[index], &view.camera),
        None if m.obj.gfx.node_flags & GRAPH_RENDER_BILLBOARD != 0 => {
            mtxf_billboard(trig, &view.camera, m.obj.gfx.pos, 0)
        }
        None => mtxf_mul(
            &mtxf_rotate_zxy_and_translate(trig, m.obj.gfx.pos, m.obj.gfx.angle),
            &view.camera,
        ),
    };
    let object_matrix = mtxf_scale_vec3f(&placed, m.obj.gfx.scale);
    let (models, anims, object_anims) = (w.models, w.anims, w.object_anims);
    let anim = m
        .obj
        .gfx
        .anim
        .cur_anim
        .is_some()
        .then(|| geo_set_animation_globals(&mut m.obj, w, anims, object_anims));
    let mut held = None;
    if obj_is_in_view(trig, &m.obj, models, &object_matrix, view.fov)
        && let Some(shared) = m.obj.gfx.shared_child
        && let Some(model) = models.mario_model()
    {
        assert!(
            i32::from(shared) == MODEL_MARIO,
            "Mario's object draws model {shared}"
        );
        let anim = anim
            .expect("Mario's model drawn without an animation reads the previous object's globals");
        let root = model.layout.root.expect("Mario's model has no root");
        let mut pass = Pass {
            model,
            view,
            trig,
            models,
            anims,
            object_anims,
            anim,
            object_matrix,
            held: None,
        };
        pass.node(m, w, root, root, &object_matrix);
        held = pass.held;
    }
    m.obj.gfx.throw_matrix = None;
    RenderedFrame {
        processed: true,
        throw_matrix,
        held,
    }
}

/// The object the last render pass drew in Mario's hand, for presentation:
/// geo_process_held_object's matrix in world coordinates, which is Mario's
/// object matrix (his placement and scale) with the hand position that pass
/// wrote, the HOLP, as its translation; the drawer applies the held object's
/// own scale. Nothing in the simulation reads it.
pub fn held_visible_object(
    m: &MarioState,
    w: &StepWorld<'_>,
    rendered: &RenderedFrame,
) -> Option<VisibleObject> {
    let id = rendered.held?;
    let o = object(&w.objects, &m.obj, id);
    let model = o.gfx.shared_child?;
    let placed = match rendered.throw_matrix {
        Some(index) => w.floor_align_matrix[index],
        None => mtxf_rotate_zxy_and_translate(w.trig, m.obj.gfx.pos, m.obj.gfx.angle),
    };
    let mut matrix = mtxf_scale_vec3f(&placed, m.obj.gfx.scale);
    let holp = m.body.held_obj_last_position;
    matrix[3] = [holp[0], holp[1], holp[2], 1.0];
    let mut cases = vec![];
    if let Some(traversal) = w.models.traversal(model) {
        selected_cases(o, traversal, traversal.root, &mut cases);
    }
    Some(VisibleObject {
        id,
        generation: w.objects.generation(id),
        behavior: o.behavior,
        model,
        pos: o.gfx.pos,
        angle: o.gfx.angle,
        scale: o.gfx.scale,
        billboard: false,
        throw_matrix: Some(matrix),
        animation: match o.gfx.anim.cur_anim {
            Some(AnimRef::Object(address)) => {
                Some((address, o.gfx.anim.anim_frame, o.gfx.anim.anim_y_trans))
            }
            _ => None,
        },
        cases,
    })
}

/// Builds a GeoLayout node by node, flags as process_geo_layout sets them.
struct LayoutBuilder {
    layout: GeoLayout,
}

impl LayoutBuilder {
    fn add(&mut self, parent: Option<usize>, kind: GeoNodeKind) -> usize {
        let index = self.layout.nodes.len();
        let flags = (u16::from(kind.layer()) << 8) | NODE_ACTIVE;
        self.layout.nodes.push(GeoNode {
            source_address: 0x1700_0000 + index as u32 * 8,
            flags,
            kind,
            children: vec![],
            views: vec![],
        });
        match parent {
            Some(parent) => self.layout.nodes[parent].children.push(index),
            None => self.layout.root = Some(index),
        }
        index
    }

    fn part(&mut self, parent: usize, translation: [i16; 3], drawn: bool) -> usize {
        self.add(
            Some(parent),
            GeoNodeKind::AnimatedPart {
                layer: 1,
                translation,
                display_list: drawn.then_some(0x0400_0000),
            },
        )
    }

    fn list(&mut self, parent: usize) -> usize {
        self.add(
            Some(parent),
            GeoNodeKind::DisplayList {
                layer: 1,
                display_list: 0x0400_0000,
            },
        )
    }

    fn generated(&mut self, parent: usize, role: MarioCallback, param: i16) -> usize {
        self.add(
            Some(parent),
            GeoNodeKind::Generated {
                param,
                callback: authored_callback(role),
            },
        )
    }

    fn switch(&mut self, parent: usize, role: MarioCallback, num_cases: i16) -> usize {
        self.add(
            Some(parent),
            GeoNodeKind::SwitchCase {
                num_cases,
                callback: authored_callback(role),
            },
        )
    }

    fn rotation(&mut self, parent: usize) -> usize {
        self.add(
            Some(parent),
            GeoNodeKind::Rotation {
                layer: 0,
                rotation: [0; 3],
                display_list: None,
            },
        )
    }

    fn scaled_list(&mut self, parent: usize, param: i16) {
        self.generated(parent, MarioCallback::HandFootScaler, param);
        let scale = self.add(
            Some(parent),
            GeoNodeKind::Scale {
                layer: 0,
                scale: 0x10000,
                display_list: None,
            },
        );
        self.list(scale);
    }

    fn held(&mut self, parent: usize) {
        self.add(
            Some(parent),
            GeoNodeKind::HeldObject {
                param: 0,
                offset: [0; 3],
                callback: authored_callback(MarioCallback::HandGrabPos),
            },
        );
    }

    /// A hand switch and its five cases (fist, open, peace, two caps).
    fn hand(&mut self, parent: usize, side: i16, length: i16) {
        let switch = self.switch(parent, MarioCallback::SwitchHand, side);
        let fist = self.part(switch, [length, 0, 0], false);
        self.scaled_list(fist, side);
        if side == 0 {
            self.held(fist);
        }
        let open = self.part(switch, [length, 0, 0], true);
        if side == 0 {
            self.held(open);
        }
        self.part(switch, [length, 0, 0], true);
        self.part(switch, [length, 0, 0], true);
        let wings = self.part(switch, [length, 0, 0], true);
        self.list(wings);
    }

    /// The face: head rotation, cap on or off, the eyes, and the wings.
    fn face(&mut self, head: usize) {
        self.generated(head, MarioCallback::HeadRotation, 0);
        let turn = self.rotation(head);
        let cap = self.switch(turn, MarioCallback::SwitchCapOnOff, 0);
        for _ in 0..2 {
            let eyes = self.switch(cap, MarioCallback::SwitchEyes, 0);
            for _ in 0..3 {
                self.list(eyes);
            }
        }
        for side in 0..2 {
            self.generated(turn, MarioCallback::RotateWingCapWings, side);
            let wing = self.rotation(turn);
            self.add(
                Some(wing),
                GeoNodeKind::TranslationRotation {
                    layer: 4,
                    translation: [142, -51, -126 + side * 252],
                    rotation: [22, -40, -135],
                    display_list: Some(0x0400_0000),
                },
            );
        }
    }

    /// One body: twenty animated parts in Mario's hierarchy, with his
    /// callbacks. `v` varies the (invented) lengths per body.
    fn body(&mut self, parent: usize, v: i16) {
        let root = self.part(parent, [0, 0, 0], false);
        let butt = self.part(root, [0, 0, 0], true);
        self.generated(butt, MarioCallback::MovePartFromParent, 0);
        self.generated(butt, MarioCallback::TiltTorso, 0);
        let tilt = self.rotation(butt);
        let torso = self.part(tilt, [70 + v, 0, 0], true);
        let head = self.part(torso, [85 + v, 0, 0], false);
        self.face(head);
        let left_shoulder = self.part(torso, [66, -9, 77 + v], false);
        let left_arm = self.part(left_shoulder, [0, 0, 0], true);
        let left_forearm = self.part(left_arm, [63 + v, 0, 0], true);
        self.hand(left_forearm, 1, 58 + v);
        let right_shoulder = self.part(torso, [69, -11, -80 - v], false);
        let right_arm = self.part(right_shoulder, [0, 0, 0], true);
        let right_forearm = self.part(right_arm, [64 - v, 0, 0], true);
        self.hand(right_forearm, 0, 62 - v);
        for side in [1, -1] {
            let hip = self.part(butt, [12, -7, 43 * side], false);
            let thigh = self.part(hip, [0, 0, 0], true);
            let leg = self.part(thigh, [88 + v, 0, 0], true);
            let foot = self.part(leg, [66, 0, 0], side == 1);
            if side == -1 {
                self.scaled_list(foot, 2);
            }
        }
    }

    /// The cap-effect switch over normal, vanish, metal and metal-vanish
    /// bodies.
    fn load_body(&mut self, parent: usize, v: i16) {
        let switch = self.switch(parent, MarioCallback::SwitchCapEffect, 0);
        for effect in 0..4 {
            self.body(switch, v + effect);
        }
    }
}

fn authored_callback(role: MarioCallback) -> u32 {
    0x8027_0000 + role as u32 * 0x40
}

/// An authored model shaped like MODEL_MARIO for ROM-free tests: mario_geo's
/// node hierarchy and callbacks (the shadow, the 0.25 scale, the mirror's
/// callbacks, the stand/run switch over a full-detail body and three ranges
/// of detail, cap effects, twenty animated parts, the torso and head
/// rotations, hands with held-object nodes and the punch scalers). The
/// lengths are invented, not game data, and differ between the bodies so
/// that the level of detail changes the hand position.
pub fn authored_mario_model() -> MarioModel {
    let mut b = LayoutBuilder {
        layout: GeoLayout {
            entry: 0x1700_0000,
            root: None,
            nodes: vec![],
            detached: vec![],
        },
    };
    let shadow = b.add(
        None,
        GeoNodeKind::Shadow {
            shadow_type: 99,
            solidity: 0xB4,
            scale: 100,
        },
    );
    let scale = b.add(
        Some(shadow),
        GeoNodeKind::Scale {
            layer: 0,
            scale: 0x4000,
            display_list: None,
        },
    );
    b.generated(scale, MarioCallback::MirrorBackfaceCulling, 0);
    b.generated(scale, MarioCallback::MirrorSetAlpha, 0);
    let stand_run = b.switch(scale, MarioCallback::SwitchStandRun, 0);
    b.load_body(stand_run, 0);
    let render = b.add(Some(stand_run), GeoNodeKind::Start);
    for (i, (min, max)) in [(-2048, 600), (600, 1600), (1600, 32767)]
        .into_iter()
        .enumerate()
    {
        let range = b.add(
            Some(render),
            GeoNodeKind::LevelOfDetail {
                min_distance: min,
                max_distance: max,
            },
        );
        b.load_body(range, i as i16 * 5);
    }
    b.generated(scale, MarioCallback::MirrorBackfaceCulling, 1);
    let callbacks = [
        MarioCallback::MirrorBackfaceCulling,
        MarioCallback::MirrorSetAlpha,
        MarioCallback::SwitchStandRun,
        MarioCallback::SwitchCapEffect,
        MarioCallback::SwitchCapOnOff,
        MarioCallback::SwitchEyes,
        MarioCallback::SwitchHand,
        MarioCallback::HeadRotation,
        MarioCallback::TiltTorso,
        MarioCallback::RotateWingCapWings,
        MarioCallback::HandFootScaler,
        MarioCallback::MovePartFromParent,
        MarioCallback::HandGrabPos,
    ]
    .into_iter()
    .map(|role| (authored_callback(role), role))
    .collect();
    MarioModel {
        layout: b.layout,
        callbacks,
    }
}
