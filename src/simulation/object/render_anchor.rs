//! The animated object matrix stack and geo_update_held_mario_pos, translated
//! from pinned CC0 rendering_graph_node.c, king_bobomb.inc.c and
//! object_helpers.c. The anchor's position is simulation state: its next
//! behavior frame positions the Mario held by the boss.
use super::{
    Object,
    render::{ObjectModel, RenderNodeKind},
};
use crate::{
    import::geo::{GRAPH_RENDER_ACTIVE as NODE_ACTIVE, GeoNodeKind},
    simulation::{
        mario::{
            StepWorld,
            constants::*,
            render::{AnimGlobals, geo_set_animation_globals},
        },
        math::{
            Mat4, get_pos_from_transform_mtx, mtxf_mul, mtxf_rotate_zxy_and_translate,
            mtxf_scale_vec3f,
        },
    },
};

pub(crate) fn process(
    o: &mut Object,
    w: &mut StepWorld<'_>,
    camera: &Mat4,
    matrix: &Mat4,
    model: &ObjectModel,
    move_anchor: bool,
) {
    // update_animation_frame is idempotent within one area update.
    let mut anim = geo_set_animation_globals(o, w, w.anims, w.object_anims);
    let pass = AnchorPass {
        camera,
        model,
        move_anchor,
    };
    pass.node(o, w, matrix, &mut anim, model.root);
}

struct AnchorPass<'p> {
    camera: &'p Mat4,
    model: &'p ObjectModel,
    move_anchor: bool,
}
impl AnchorPass<'_> {
    fn children(
        &self,
        o: &mut Object,
        w: &mut StepWorld<'_>,
        top: &Mat4,
        anim: &mut AnimGlobals<'_>,
        n: usize,
    ) {
        for &child in &self.model.nodes[n].children {
            self.node(o, w, top, anim, child);
        }
    }

    fn node(
        &self,
        o: &mut Object,
        w: &mut StepWorld<'_>,
        top: &Mat4,
        anim: &mut AnimGlobals<'_>,
        n: usize,
    ) {
        let model = self.model;
        let layout = model.anchor_layout.as_ref().expect("anchor layout");
        let geo = &layout.nodes[n];
        if geo.flags & NODE_ACTIVE == 0 {
            return;
        }
        if geo.flags & GRAPH_RENDER_CHILDREN_FIRST as u16 != 0 {
            self.children(o, w, top, anim, n);
            return;
        }
        let next = match geo.kind {
            GeoNodeKind::AnimatedPart { translation, .. } => {
                anim.animated_part(w.trig, translation, top)
            }
            GeoNodeKind::TranslationRotation {
                translation,
                rotation,
                ..
            } => {
                let local =
                    mtxf_rotate_zxy_and_translate(w.trig, translation.map(f32::from), rotation);
                mtxf_mul(&local, top)
            }
            GeoNodeKind::Translation { translation, .. } => {
                let local =
                    mtxf_rotate_zxy_and_translate(w.trig, translation.map(f32::from), [0; 3]);
                mtxf_mul(&local, top)
            }
            GeoNodeKind::Rotation { rotation, .. } => {
                let local = mtxf_rotate_zxy_and_translate(w.trig, [0.0; 3], rotation);
                mtxf_mul(&local, top)
            }
            GeoNodeKind::Scale { scale, .. } => mtxf_scale_vec3f(top, [scale as f32 / 65536.0; 3]),
            GeoNodeKind::Billboard { .. } => {
                // King's billboards contain only display lists. Their matrices
                // affect drawing; none feeds an animation or simulated callback.
                assert!(
                    geo.children.iter().all(|i| matches!(
                        layout.nodes[*i].kind,
                        GeoNodeKind::DisplayList { .. }
                    ) && layout.nodes[*i].children.is_empty()),
                    "billboard has simulated descendants"
                );
                *top
            }
            GeoNodeKind::Generated { .. } => {
                match model.nodes[n].kind {
                    RenderNodeKind::HeldMarioPos if self.move_anchor => {
                        update_anchor(o, w, top, self.camera)
                    }
                    RenderNodeKind::HeldMarioPos => {}
                    RenderNodeKind::LayerTransparency => {}
                    _ => panic!("unaudited animated object callback"),
                }
                *top
            }
            GeoNodeKind::SwitchCase { .. } => {
                let RenderNodeKind::AnimStateSwitch { num_cases } = model.nodes[n].kind else {
                    panic!("unaudited animated object switch")
                };
                if o.raw.s32(O_ANIM_STATE) >= i32::from(num_cases) {
                    o.raw.set_s32(O_ANIM_STATE, 0);
                }
                if !geo.children.is_empty() {
                    let selected = o.raw.s32(O_ANIM_STATE) as i16;
                    let index = if selected > 0 {
                        selected as usize % geo.children.len()
                    } else {
                        0
                    };
                    self.node(o, w, top, anim, geo.children[index]);
                }
                return;
            }
            GeoNodeKind::Start
            | GeoNodeKind::Shadow { .. }
            | GeoNodeKind::DisplayList { .. }
            | GeoNodeKind::CullingRadius { .. } => *top,
            ref kind => panic!("unaudited animated object node: {kind:?}"),
        };
        self.children(o, w, &next, anim, n);
    }
}

fn update_anchor(o: &Object, w: &mut StepWorld<'_>, matrix: &Mat4, camera: &Mat4) {
    let Some(id) = o.prev_obj else {
        return;
    };
    assert_ne!(Some(id), w.objects.mario, "anchor is not Mario's object");
    // create_transformation_from_matrices: remove the camera transform with
    // the original row-dot products and operation order, including scale.
    let mut transform = [[0.0f32; 4]; 4];
    for row in 0..3 {
        for col in 0..3 {
            transform[row][col] = matrix[row][0] * camera[col][0]
                + matrix[row][1] * camera[col][1]
                + matrix[row][2] * camera[col][2];
        }
    }
    transform[3][..3].copy_from_slice(&get_pos_from_transform_mtx(matrix, camera));
    transform[3][3] = 1.0;
    let anchor = w.objects.slot_mut(id);
    let x = anchor.raw.f32(O_PARENT_RELATIVE_POS_X);
    let y = anchor.raw.f32(O_PARENT_RELATIVE_POS_Y);
    let z = anchor.raw.f32(O_PARENT_RELATIVE_POS_Z);
    // obj_update_pos_from_parent_transformation, then obj_set_gfx_pos_from_pos.
    for (axis, field) in [O_POS_X, O_POS_Y, O_POS_Z].into_iter().enumerate() {
        let pos = x * transform[0][axis]
            + y * transform[1][axis]
            + z * transform[2][axis]
            + transform[3][axis];
        anchor.raw.set_f32(field, pos);
    }
    anchor.gfx.pos = [
        anchor.raw.f32(O_POS_X),
        anchor.raw.f32(O_POS_Y) + anchor.raw.f32(O_GRAPH_Y_OFFSET),
        anchor.raw.f32(O_POS_Z),
    ];
}
