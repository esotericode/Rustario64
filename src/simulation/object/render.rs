//! The render pass's writes to object state, translated from pinned CC0
//! src/game/rendering_graph_node.c (geo_process_object, obj_is_in_view,
//! geo_process_switch, geo_process_node_and_siblings) and
//! src/game/object_helpers.c (geo_switch_anim_state).
//!
//! The render pass is not presentation-only: a model's switch callback
//! `geo_switch_anim_state` resets an object's oAnimState when it reaches the
//! switch's case count, and it only runs while the object is inside the
//! camera's view; and geo_set_animation_globals advances an animated
//! object's frame (which behaviors read) whenever its node is processed. So the simulation runs the pass's object traversal each
//! frame from the authoritative camera (Lakitu's graph camera and field of
//! view), with the original matrices. Matrices, display lists and the rest of
//! the drawing stay in presentation.
use super::{Object, ObjectId, ThrowMatrix};
use crate::simulation::{
    camera::system::GraphCamera,
    mario::{StepWorld, constants::*, render::MarioModel},
    math::{
        Mat4, TrigTables, mtxf_billboard, mtxf_identity, mtxf_lookat, mtxf_mul,
        mtxf_rotate_zxy_and_translate, mtxf_scale_vec3f,
    },
};
use std::collections::BTreeMap;

/// A model node's role in the render pass's object traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderNodeKind {
    /// No callback: drawing only.
    Plain,
    /// A GEO_SWITCH_CASE whose callback is geo_switch_anim_state.
    AnimStateSwitch { num_cases: i16 },
    /// GEO_CULLING_RADIUS (read by obj_is_in_view when it is the root).
    CullingRadius { radius: i16 },
    /// A node whose callback or traversal the port has not audited.
    Unaudited { what: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderNode {
    pub kind: RenderNodeKind,
    pub children: Vec<usize>,
}

/// The authoritative view of a model's geo layout: its node tree from the
/// root (gLoadedGraphNodes[model]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectModel {
    pub nodes: Vec<RenderNode>,
    pub root: usize,
}

/// gLoadedGraphNodes as the simulation needs it: which model IDs are
/// loaded, and the traversal of those whose objects the port spawns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectModels {
    loaded: BTreeMap<u16, Option<ObjectModel>>,
    /// MODEL_MARIO's graph, which the render pass traverses for Mario.
    mario: Option<MarioModel>,
}

/// No models: for worlds that never spawn objects.
pub static NO_MODELS: ObjectModels = ObjectModels {
    loaded: BTreeMap::new(),
    mario: None,
};

impl ObjectModels {
    /// Register a loaded model ID, with its traversal if it is known.
    pub fn insert(&mut self, model: u16, traversal: Option<ObjectModel>) {
        self.loaded.insert(model, traversal);
    }

    pub fn is_loaded(&self, model: u16) -> bool {
        self.loaded.contains_key(&model)
    }

    pub fn traversal(&self, model: u16) -> Option<&ObjectModel> {
        self.loaded.get(&model).and_then(Option::as_ref)
    }

    pub fn ids(&self) -> impl Iterator<Item = u16> + '_ {
        self.loaded.keys().copied()
    }

    /// Load MODEL_MARIO's graph (registering the model ID).
    pub fn set_mario_model(&mut self, model: MarioModel) {
        self.loaded.entry(MODEL_MARIO as u16).or_insert(None);
        self.mario = Some(model);
    }

    pub fn mario_model(&self) -> Option<&MarioModel> {
        self.mario.as_ref()
    }
}

/// The camera node's transform above GEO_RENDER_OBJ: geo_process_camera's
/// mtxf_lookat from Lakitu's position to his focus (the camera node's roll
/// stays 0; rollScreen only rotates the projection), applied to
/// geo_process_root's identity.
pub fn camera_matrix(trig: &TrigTables, camera: &GraphCamera) -> Mat4 {
    let look = mtxf_lookat(trig, camera.pos, camera.focus, 0);
    mtxf_mul(&look, &mtxf_identity())
}

/// obj_is_in_view with the perspective node's field of view.
pub fn obj_is_in_view(
    trig: &TrigTables,
    o: &Object,
    models: &ObjectModels,
    matrix: &Mat4,
    fov: f32,
) -> bool {
    if o.gfx.node_flags & GRAPH_RENDER_INVISIBLE != 0 {
        return false;
    }
    let half_fov = crate::simulation::mario::f32_to_s16((fov / 2.0 + 1.0) * 32768.0 / 180.0 + 0.5);
    let h_screen_edge =
        -matrix[3][2] * trig.sins(i32::from(half_fov)) / trig.coss(i32::from(half_fov));
    let culling_radius = match o.gfx.shared_child.and_then(|m| models.traversal(m)) {
        Some(model) => match model.nodes[model.root].kind {
            RenderNodeKind::CullingRadius { radius } => radius,
            _ => 300,
        },
        None => 300,
    };
    let radius = f32::from(culling_radius);
    if matrix[3][2] > -100.0 + radius {
        return false;
    }
    if matrix[3][2] < -20000.0 - radius {
        return false;
    }
    if matrix[3][0] > h_screen_edge + radius {
        return false;
    }
    if matrix[3][0] < -h_screen_edge - radius {
        return false;
    }
    true
}

/// geo_process_node_and_siblings over a model for `o`: switch callbacks write
/// the object; only the selected case's subtree is processed.
pub(crate) fn process_model(o: &mut Object, model: &ObjectModel, node: usize) {
    let n = &model.nodes[node];
    match &n.kind {
        RenderNodeKind::Plain | RenderNodeKind::CullingRadius { .. } => {
            for &child in &n.children {
                process_model(o, model, child);
            }
        }
        RenderNodeKind::AnimStateSwitch { num_cases } => {
            // geo_switch_anim_state.
            if o.raw.s32(O_ANIM_STATE) >= i32::from(*num_cases) {
                o.raw.set_s32(O_ANIM_STATE, 0);
            }
            let selected = o.raw.s32(O_ANIM_STATE) as i16;
            // geo_process_switch walks the circular sibling list.
            if !n.children.is_empty() {
                let index = if selected > 0 {
                    selected as usize % n.children.len()
                } else {
                    0
                };
                process_model(o, model, n.children[index]);
            }
        }
        RenderNodeKind::Unaudited { what } => {
            panic!("the render pass reaches {what}, which is not audited for object state")
        }
    }
}

/// geo_process_object's placement of an object other than Mario under the
/// camera transform: its throw matrix, a billboard at its position, or its
/// position and angles; then its scale.
pub fn placement(trig: &TrigTables, o: &Object, camera: &Mat4) -> Mat4 {
    let placed = match o.gfx.throw_matrix {
        Some(ThrowMatrix::Terrain(matrix)) => mtxf_mul(&matrix, camera),
        Some(ThrowMatrix::FloorAlign(_)) => {
            panic!("only Mario's object uses the floor-align matrices")
        }
        None if o.gfx.node_flags & GRAPH_RENDER_BILLBOARD != 0 => {
            mtxf_billboard(trig, camera, o.gfx.pos, 0)
        }
        None => {
            let local = mtxf_rotate_zxy_and_translate(trig, o.gfx.pos, o.gfx.angle);
            mtxf_mul(&local, camera)
        }
    };
    mtxf_scale_vec3f(&placed, o.gfx.scale)
}

/// geo_process_object's state changes for one object other than Mario
/// under the camera transform `camera`. Returns the terrain matrix the pass
/// placed the object with (it clears it), for presentation.
pub fn render_object(o: &mut Object, w: &StepWorld<'_>, camera: &Mat4, fov: f32) -> Option<Mat4> {
    if o.gfx.node_flags & GRAPH_RENDER_ACTIVE == 0 {
        o.gfx.throw_matrix = None;
        return None;
    }
    if o.gfx.area_index != w.area_index {
        return None;
    }
    let used = match o.gfx.throw_matrix {
        Some(ThrowMatrix::Terrain(matrix)) => Some(matrix),
        _ => None,
    };
    let matrix = placement(w.trig, o, camera);
    // geo_set_animation_globals, before (and regardless of) the view test.
    if o.gfx.anim.cur_anim.is_some() {
        crate::simulation::mario::animation::update_animation_frame(o, w);
    }
    if obj_is_in_view(w.trig, o, w.models, &matrix, fov)
        && let Some(model) = o.gfx.shared_child
    {
        let traversal = w
            .models
            .traversal(model)
            .unwrap_or_else(|| panic!("model {model}'s render traversal was not imported"));
        process_model(o, traversal, traversal.root);
    }
    o.gfx.throw_matrix = None;
    used
}

/// The terrain matrices the last render pass placed objects with, by
/// object. Presentation only: the pass cleared them from the objects.
pub type RenderedMatrices = BTreeMap<ObjectId, Mat4>;

/// The render pass for every object node but Mario's, which
/// `mario::tick::render_mario_object` processes. The original walks
/// gObjParentGraphNode's children in allocation order; no ported write
/// depends on another object, so list order gives the same state.
pub fn render_objects(w: &mut StepWorld<'_>, camera: &GraphCamera) -> RenderedMatrices {
    let matrix = camera_matrix(w.trig, camera);
    let mut ids: Vec<ObjectId> = vec![];
    for list in super::UPDATE_ORDER {
        ids.extend(w.objects.list(list));
    }
    let mut used = RenderedMatrices::new();
    for id in ids {
        if w.objects.mario == Some(id) {
            continue;
        }
        let mut o = std::mem::take(w.objects.slot_mut(id));
        if let Some(m) = render_object(&mut o, w, &matrix, camera.fov) {
            used.insert(id, m);
        }
        *w.objects.slot_mut(id) = o;
    }
    used
}

/// An object the render pass draws this frame, after its writes: where
/// geo_process_object places it and the child each anim-state switch of its
/// model selects. Presentation reads this; nothing in the simulation does.
#[derive(Debug, Clone, PartialEq)]
pub struct VisibleObject {
    pub id: ObjectId,
    pub generation: u64,
    /// The object's behavior script (segmented), to tell a reused slot apart.
    pub behavior: u32,
    pub model: u16,
    pub pos: [f32; 3],
    pub angle: [i16; 3],
    pub scale: [f32; 3],
    /// GRAPH_RENDER_BILLBOARD: drawn facing the camera (mtxf_billboard).
    pub billboard: bool,
    /// The terrain matrix the pass placed the object with, if any.
    pub throw_matrix: Option<Mat4>,
    /// The animation the pass posed the object with: curAnim's segmented
    /// address, the frame and the vertical-translation multiplier.
    pub animation: Option<(u32, i16, i16)>,
    /// (switch node, selected child node), in traversal order.
    pub cases: Vec<(usize, usize)>,
}

pub(crate) fn selected_cases(
    o: &Object,
    model: &ObjectModel,
    node: usize,
    out: &mut Vec<(usize, usize)>,
) {
    let n = &model.nodes[node];
    match &n.kind {
        RenderNodeKind::Plain | RenderNodeKind::CullingRadius { .. } => {
            for &child in &n.children {
                selected_cases(o, model, child, out);
            }
        }
        RenderNodeKind::AnimStateSwitch { num_cases } => {
            let state = o.raw.s32(O_ANIM_STATE);
            let selected = if state >= i32::from(*num_cases) {
                0
            } else {
                state as i16
            };
            if !n.children.is_empty() {
                let index = if selected > 0 {
                    selected as usize % n.children.len()
                } else {
                    0
                };
                out.push((node, n.children[index]));
                selected_cases(o, model, n.children[index], out);
            }
        }
        // The simulation's pass stops at these; so does drawing.
        RenderNodeKind::Unaudited { .. } => {}
    }
}

/// The objects other than Mario that the render pass at `camera` draws, in
/// `render_objects`' order: the same conditions (active, in the rendered
/// area, `obj_is_in_view` with the original matrices, and the terrain
/// matrices the pass used, from `render_objects`), read without writing.
/// Run after the frame, the switches select what the pass selected.
pub fn visible_objects(
    w: &StepWorld<'_>,
    camera: &GraphCamera,
    matrices: &RenderedMatrices,
) -> Vec<VisibleObject> {
    let matrix = camera_matrix(w.trig, camera);
    let mut out = vec![];
    for list in super::UPDATE_ORDER {
        for id in w.objects.list(list) {
            if w.objects.mario == Some(id) {
                continue;
            }
            let mut o = w.objects.slot(id).clone();
            if o.gfx.node_flags & GRAPH_RENDER_ACTIVE == 0 || o.gfx.area_index != w.area_index {
                continue;
            }
            let billboard = o.gfx.node_flags & GRAPH_RENDER_BILLBOARD != 0;
            let throw_matrix = matrices.get(&id).copied();
            o.gfx.throw_matrix = throw_matrix.map(ThrowMatrix::Terrain);
            let placed = placement(w.trig, &o, &matrix);
            let o = &o;
            let Some(model) = o.gfx.shared_child else {
                continue;
            };
            if !obj_is_in_view(w.trig, o, w.models, &placed, camera.fov) {
                continue;
            }
            let mut cases = vec![];
            if let Some(traversal) = w.models.traversal(model) {
                selected_cases(o, traversal, traversal.root, &mut cases);
            }
            out.push(VisibleObject {
                id,
                generation: w.objects.generation(id),
                behavior: o.behavior,
                model,
                pos: o.gfx.pos,
                angle: o.gfx.angle,
                scale: o.gfx.scale,
                billboard,
                throw_matrix,
                animation: match o.gfx.anim.cur_anim {
                    Some(super::AnimRef::Object(address)) => {
                        Some((address, o.gfx.anim.anim_frame, o.gfx.anim.anim_y_trans))
                    }
                    _ => None,
                },
                cases,
            });
        }
    }
    out
}

/// An authored model table for ROM-free tests: MODEL_MARIO loaded, and the
/// coin, sparkle, Bob-omb, explosion and smoke models with the switch
/// structure of the pinned actors' layouts (a shadow over an 8-case
/// geo_switch_anim_state switch, a 12-case switch, the Bob-omb's 2-case eye
/// switch, the explosion's 9 and the smoke's 7). Not ROM content.
pub fn authored_models() -> ObjectModels {
    let switch = |num_cases: i16, wrapped: bool| {
        let base = usize::from(wrapped);
        let mut nodes = vec![];
        if wrapped {
            nodes.push(RenderNode {
                kind: RenderNodeKind::Plain,
                children: vec![1],
            });
        }
        nodes.push(RenderNode {
            kind: RenderNodeKind::AnimStateSwitch { num_cases },
            children: (base + 1..base + 1 + num_cases as usize).collect(),
        });
        for _ in 0..num_cases {
            nodes.push(RenderNode {
                kind: RenderNodeKind::Plain,
                children: vec![],
            });
        }
        ObjectModel { nodes, root: 0 }
    };
    let mut models = ObjectModels::default();
    models.insert(
        MODEL_MARIO as u16,
        Some(ObjectModel {
            nodes: vec![RenderNode {
                kind: RenderNodeKind::Plain,
                children: vec![],
            }],
            root: 0,
        }),
    );
    models.set_mario_model(crate::simulation::mario::render::authored_mario_model());
    models.insert(MODEL_YELLOW_COIN as u16, Some(switch(8, true)));
    models.insert(MODEL_YELLOW_COIN_NO_SHADOW as u16, Some(switch(8, true)));
    models.insert(MODEL_SPARKLES as u16, Some(switch(12, false)));
    // The Bob-omb's eyes (a 2-case switch under its parts), the explosion's
    // nine frames under a start node, and the smoke's seven.
    models.insert(MODEL_BLACK_BOBOMB as u16, Some(switch(2, true)));
    models.insert(MODEL_EXPLOSION as u16, Some(switch(9, true)));
    models.insert(MODEL_SMOKE as u16, Some(switch(7, false)));
    models
}
