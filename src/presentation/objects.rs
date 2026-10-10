//! ROM object meshes drawn from completed frames. The authoritative render
//! traversal already selected switches and visibility and advanced
//! animations; this drawer only reads those results.
//!
//! Models without animated parts (coins, sparkles, explosions, smoke) are
//! built once per selected switch configuration with their transforms baked,
//! then placed per object. Models with animated parts (Bob-ombs) are built
//! once per draw list with each vertex's bone, posed per completed frame from
//! the object's ROM animation as geo_process_animated_part does, and
//! interpolated per displayed frame. Billboards use the displayed camera's
//! unrolled axes: an object billboard as geo_process_object's mtxf_billboard
//! with roll 0, a GEO_BILLBOARD part as geo_process_billboard (its parents'
//! rotation and scale are replaced, its position kept).
use crate::{
    content::{
        animation::{
            ANIM_FLAG_6, ANIM_FLAG_HOR_TRANS, ANIM_FLAG_VERT_TRANS, NO_OBJECT_ANIMATIONS,
            ObjectAnimations,
        },
        visual::{LAYER_COUNT, VisualModel, VisualVertex},
    },
    import::{
        ImportError, Result,
        geo::{self, GRAPH_RENDER_ACTIVE, GeoLayout, GeoNodeKind},
        gfx::{self, IDENTITY},
        model,
        objects::{ModelRegistration, ObjectContent},
        segments::Segments,
    },
    presentation::mario::{
        AnimCursor, AnimType, quantize_normal, rotate_xyz_and_translate, rotate_zxy_and_translate,
        scale_rows,
    },
    simulation::{
        math::{Mat4, TrigTables, mtxf_rotate_zxy_and_translate, mtxf_scale_vec3f},
        object::render::VisibleObject,
    },
};
use std::collections::BTreeMap;

/// World-space axes of the presentation camera, without screen roll.
#[derive(Debug, Clone, Copy)]
pub struct BillboardBasis {
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub toward: [f32; 3],
}

/// A level's own models: the segments its script loaded, its model
/// registrations (which replace the main scripts' for the same ID) and the
/// object animations its behaviors use.
#[derive(Clone, Copy)]
pub struct LevelModels<'a> {
    pub segments: &'a Segments,
    pub registrations: &'a [(u16, ModelRegistration)],
    pub animations: &'a ObjectAnimations,
}

/// One display list of a skinned build: layer, address, bone.
type SkinnedDraw = (u8, u32, u16);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum BuildKey {
    Static {
        model: u16,
        cases: Vec<(usize, usize)>,
    },
    Skinned {
        model: u16,
        draws: Vec<SkinnedDraw>,
    },
}

struct Build {
    key: BuildKey,
    model: VisualModel,
    /// Each vertex's bone, per batch (skinned builds only).
    bones: Option<Vec<Vec<u16>>>,
}

/// One object's completed frame: its build and, for skinned builds, the
/// world matrix of each bone (a geo node, or the object's own after them).
#[derive(Debug, Clone)]
struct Posed {
    object: VisibleObject,
    build: usize,
    bones: Vec<Mat4>,
    /// Bones whose rotation faces the camera (GEO_BILLBOARD parts).
    billboards: Vec<bool>,
}

/// One uploaded template, repeated for every instance selecting this build.
pub struct ObjectFrame<'a> {
    pub build: usize,
    pub template: &'a VisualModel,
    pub vertices: Vec<Vec<VisualVertex>>,
}

pub struct ObjectDrawer<'a> {
    content: &'a ObjectContent,
    level: Option<LevelModels<'a>>,
    trig: &'a TrigTables,
    builds: Vec<Build>,
    layouts: BTreeMap<u16, GeoLayout>,
    previous: Vec<Posed>,
    current: Vec<Posed>,
}

/// A skinned model's render traversal for one object.
struct ObjectWalk<'a> {
    layout: &'a GeoLayout,
    trig: &'a TrigTables,
    cases: &'a [(usize, usize)],
    anim: Option<AnimCursor<'a>>,
    bones: Vec<Mat4>,
    billboards: Vec<bool>,
    draws: Vec<SkinnedDraw>,
    unsupported: Option<String>,
}

impl ObjectWalk<'_> {
    fn draw(&mut self, layer: u8, address: Option<u32>, bone: u16) {
        if let Some(address) = address {
            self.draws.push((layer, address, bone));
        }
    }

    /// geo_process_animated_part's translation and rotation.
    fn animated_part(&mut self, translation: [i16; 3]) -> ([f32; 3], [i16; 3]) {
        let mut t = translation.map(f32::from);
        let Some(c) = self.anim.as_mut() else {
            return (t, [0; 3]);
        };
        match c.kind {
            AnimType::Translation => {
                for value in &mut t {
                    *value += f32::from(c.next()) * c.multiplier;
                }
            }
            AnimType::LateralTranslation => {
                t[0] += f32::from(c.next()) * c.multiplier;
                c.attribute += 1;
                t[2] += f32::from(c.next()) * c.multiplier;
            }
            AnimType::VerticalTranslation => {
                c.attribute += 1;
                t[1] += f32::from(c.next()) * c.multiplier;
                c.attribute += 1;
            }
            AnimType::NoTranslation => c.attribute += 3,
            AnimType::Rotation => {}
        }
        c.kind = AnimType::Rotation;
        (t, [c.next(), c.next(), c.next()])
    }

    fn siblings(&mut self, nodes: &[usize], parent: &Mat4, bone: u16) {
        for &n in nodes {
            if self.layout.nodes[n].flags & GRAPH_RENDER_ACTIVE != 0 {
                self.node(n, parent, bone);
            }
        }
    }

    fn node(&mut self, n: usize, parent: &Mat4, bone: u16) {
        let layout = self.layout;
        let node = &layout.nodes[n];
        let children = node.children.as_slice();
        let this = n as u16;
        let place = |walk: &mut Self, local: Mat4, layer: u8, dl: Option<u32>| {
            let m = gfx::mat_mul(&local, parent);
            walk.bones[n] = m;
            walk.draw(layer, dl, this);
            walk.siblings(children, &m, this);
        };
        match node.kind {
            GeoNodeKind::SwitchCase { .. } => {
                let child = self
                    .cases
                    .iter()
                    .find(|(switch, _)| *switch == n)
                    .map(|(_, child)| *child)
                    .or_else(|| children.first().copied());
                if let Some(child) = child {
                    self.siblings(&[child], parent, bone);
                }
            }
            GeoNodeKind::Scale {
                layer,
                scale,
                display_list,
            } => {
                let m = scale_rows(parent, [scale as f32 / 65536.0; 3]);
                self.bones[n] = m;
                self.draw(layer, display_list, this);
                self.siblings(children, &m, this);
            }
            GeoNodeKind::AnimatedPart {
                layer,
                translation,
                display_list,
            } => {
                let (t, r) = self.animated_part(translation);
                place(
                    self,
                    rotate_xyz_and_translate(self.trig, t, r),
                    layer,
                    display_list,
                );
            }
            GeoNodeKind::TranslationRotation {
                layer,
                translation,
                rotation,
                display_list,
            } => {
                let local =
                    rotate_zxy_and_translate(self.trig, translation.map(f32::from), rotation);
                place(self, local, layer, display_list);
            }
            GeoNodeKind::Translation {
                layer,
                translation,
                display_list,
            } => {
                let local = rotate_zxy_and_translate(self.trig, translation.map(f32::from), [0; 3]);
                place(self, local, layer, display_list);
            }
            GeoNodeKind::Rotation {
                layer,
                rotation,
                display_list,
            } => {
                let local = rotate_zxy_and_translate(self.trig, [0.0; 3], rotation);
                place(self, local, layer, display_list);
            }
            GeoNodeKind::Billboard {
                layer,
                translation,
                display_list,
            } => {
                // mtxf_billboard keeps only the transformed position; the
                // rotation is the camera's, applied when a frame is drawn.
                let [x, y, z] = translation.map(f32::from);
                let p = parent;
                let mut m = IDENTITY;
                m[3] = [
                    p[0][0] * x + p[1][0] * y + p[2][0] * z + p[3][0],
                    p[0][1] * x + p[1][1] * y + p[2][1] * z + p[3][1],
                    p[0][2] * x + p[1][2] * y + p[2][2] * z + p[3][2],
                    1.0,
                ];
                self.bones[n] = m;
                self.billboards[n] = true;
                self.draw(layer, display_list, this);
                for &child in children {
                    let kind = &layout.nodes[child].kind;
                    if !matches!(kind, GeoNodeKind::DisplayList { .. }) {
                        self.unsupported = Some(format!("a {kind:?} under a billboard part"));
                    }
                }
                self.siblings(children, &m, this);
            }
            GeoNodeKind::DisplayList {
                layer,
                display_list,
            } => {
                self.draw(layer, Some(display_list), bone);
                self.siblings(children, parent, bone);
            }
            GeoNodeKind::Root { .. }
            | GeoNodeKind::Start
            | GeoNodeKind::CullingRadius { .. }
            | GeoNodeKind::ObjectParent => self.siblings(children, parent, bone),
            // Object shadows are not drawn yet.
            GeoNodeKind::Shadow { .. } => self.siblings(children, parent, bone),
            ref other => {
                self.unsupported = Some(format!("{other:?}"));
            }
        }
    }
}

/// gCurAnimType from an animation's flags (geo_set_animation_globals).
fn anim_type(flags: i16) -> AnimType {
    if flags & ANIM_FLAG_HOR_TRANS != 0 {
        AnimType::VerticalTranslation
    } else if flags & ANIM_FLAG_VERT_TRANS != 0 {
        AnimType::LateralTranslation
    } else if flags & ANIM_FLAG_6 != 0 {
        AnimType::NoTranslation
    } else {
        AnimType::Translation
    }
}

impl<'a> ObjectDrawer<'a> {
    /// A drawer for the main scripts' models only.
    pub fn new(content: &'a ObjectContent, trig: &'a TrigTables) -> Self {
        Self {
            content,
            level: None,
            trig,
            builds: vec![],
            layouts: BTreeMap::new(),
            previous: vec![],
            current: vec![],
        }
    }

    /// A drawer that also knows a level's own models and animations.
    pub fn for_level(
        content: &'a ObjectContent,
        level: LevelModels<'a>,
        trig: &'a TrigTables,
    ) -> Self {
        Self {
            level: Some(level),
            ..Self::new(content, trig)
        }
    }

    /// The registration gLoadedGraphNodes holds for `model` and the segments
    /// its layout lives in: the level's (registered last) or the main scripts'.
    fn registration(&self, model: u16) -> Result<(ModelRegistration, &'a Segments)> {
        if let Some(level) = self.level
            && let Some((_, r)) = level.registrations.iter().rev().find(|(m, _)| *m == model)
        {
            return Ok((*r, level.segments));
        }
        let r = self.content.main_models.get(&model).ok_or_else(|| {
            ImportError::new("object drawing", usize::from(model), "model not imported")
        })?;
        Ok((*r, &self.content.segments))
    }

    fn animations(&self) -> &'a ObjectAnimations {
        self.level.map_or(&NO_OBJECT_ANIMATIONS, |l| l.animations)
    }

    /// Pose one object, building its model's display lists the first time
    /// a configuration appears.
    fn pose(&mut self, o: VisibleObject) -> Result<Posed> {
        let (registration, segments) = self.registration(o.model)?;
        if !registration.geometry_layout {
            return Err(ImportError::new(
                "object drawing",
                registration.pointer as usize,
                "display-list-only object models are not supported",
            ));
        }
        if let std::collections::btree_map::Entry::Vacant(entry) = self.layouts.entry(o.model) {
            entry.insert(geo::decode(segments, registration.pointer)?);
        }
        let layout = &self.layouts[&o.model];
        let animated = layout
            .nodes
            .iter()
            .any(|n| matches!(n.kind, GeoNodeKind::AnimatedPart { .. }));
        if !animated {
            let key = BuildKey::Static {
                model: o.model,
                cases: o.cases.clone(),
            };
            let build = match self.builds.iter().position(|b| b.key == key) {
                Some(build) => build,
                None => {
                    let built = model::build_selected(segments, layout, true, &|node| {
                        o.cases
                            .iter()
                            .find(|(n, _)| *n == node)
                            .map(|(_, child)| *child)
                    })?;
                    // GEO_SHADOW is the only intentionally deferred drawing node.
                    if let Some(issue) = built
                        .issues
                        .iter()
                        .find(|i| i.feature != "shadow node not drawn")
                    {
                        return Err(ImportError::new(
                            "object drawing",
                            issue.address as usize,
                            &issue.feature,
                        ));
                    }
                    self.builds.push(Build {
                        key,
                        model: built.model,
                        bones: None,
                    });
                    self.builds.len() - 1
                }
            };
            return Ok(Posed {
                object: o,
                build,
                bones: vec![],
                billboards: vec![],
            });
        }
        // geo_process_object's placement, then the parts.
        let base = o
            .throw_matrix
            .unwrap_or_else(|| mtxf_rotate_zxy_and_translate(self.trig, o.pos, o.angle));
        let object = scale_rows(&base, o.scale);
        let anims = self.animations();
        let mut walk = ObjectWalk {
            layout,
            trig: self.trig,
            cases: &o.cases,
            anim: o.animation.and_then(|(address, frame, y_trans)| {
                let animation = anims.get(address)?;
                Some(AnimCursor {
                    animation,
                    frame: i32::from(frame),
                    multiplier: if animation.y_trans_divisor == 0 {
                        1.0
                    } else {
                        f32::from(y_trans) / f32::from(animation.y_trans_divisor)
                    },
                    kind: anim_type(animation.flags),
                    attribute: 0,
                })
            }),
            bones: vec![IDENTITY; layout.nodes.len() + 1],
            billboards: vec![false; layout.nodes.len() + 1],
            draws: vec![],
            unsupported: None,
        };
        let object_bone = layout.nodes.len() as u16;
        walk.bones[usize::from(object_bone)] = object;
        if let Some(root) = layout.root {
            walk.siblings(&[root], &object, object_bone);
        }
        if let Some(what) = walk.unsupported {
            return Err(ImportError::new(
                "object drawing",
                registration.pointer as usize,
                format!("animated model node not drawn: {what}"),
            ));
        }
        let (draws, bones, billboards) = (walk.draws, walk.bones, walk.billboards);
        let key = BuildKey::Skinned {
            model: o.model,
            draws,
        };
        let build = match self.builds.iter().position(|b| b.key == key) {
            Some(build) => build,
            None => {
                let BuildKey::Skinned { draws, .. } = &key else {
                    unreachable!()
                };
                let mut builder = gfx::Builder::new(segments);
                for layer in 0..LAYER_COUNT as u8 {
                    for &(_, address, bone) in draws.iter().filter(|d| d.0 == layer) {
                        builder.set_bone(bone);
                        builder.run(address, layer, &IDENTITY, true)?;
                    }
                }
                let (skinned, issues) = builder.finish_skinned();
                if let Some(issue) = issues.first() {
                    return Err(ImportError::new(
                        "object drawing",
                        issue.address as usize,
                        &issue.feature,
                    ));
                }
                self.builds.push(Build {
                    key: key.clone(),
                    model: skinned.model,
                    bones: Some(skinned.bones),
                });
                self.builds.len() - 1
            }
        };
        Ok(Posed {
            object: o,
            build,
            bones,
            billboards,
        })
    }

    /// Pose every visible object, building new configurations once, then
    /// retain the two endpoints. Import errors leave the previous
    /// successful snapshot intact.
    pub fn update(&mut self, objects: Vec<VisibleObject>) -> Result<()> {
        let mut posed = Vec::with_capacity(objects.len());
        for o in objects {
            posed.push(self.pose(o)?);
        }
        self.previous = std::mem::replace(&mut self.current, posed);
        Ok(())
    }

    /// Pause/focus/inspection boundaries hold the latest completed positions.
    pub fn snap(&mut self) {
        self.previous.clear();
    }

    /// Level entry clears identities but retains reusable imported builds.
    pub fn reset(&mut self) {
        self.previous.clear();
        self.current.clear();
    }

    pub fn builds(&self) -> usize {
        self.builds.len()
    }

    pub fn frame(
        &self,
        alpha: f32,
        interpolation: bool,
        basis: BillboardBasis,
    ) -> Vec<ObjectFrame<'_>> {
        let mut frames: BTreeMap<usize, ObjectFrame<'_>> = BTreeMap::new();
        for current in &self.current {
            let c = &current.object;
            let previous = self.previous.iter().find(|p| {
                let o = &p.object;
                o.id == c.id
                    && o.generation == c.generation
                    && o.behavior == c.behavior
                    && o.model == c.model
                    && o.billboard == c.billboard
            });
            let build = &self.builds[current.build];
            let template = &build.model;
            let frame = frames.entry(current.build).or_insert_with(|| ObjectFrame {
                build: current.build,
                template,
                vertices: vec![vec![]; template.batches.len()],
            });
            let blend_alpha = (interpolation && alpha.is_finite()).then(|| alpha.clamp(0.0, 1.0));
            match &build.bones {
                None => {
                    let pose = blend(previous.map(|p| &p.object), c, blend_alpha);
                    let placed = if pose.billboard {
                        billboard_matrix(basis, pose.pos)
                    } else {
                        mtxf_rotate_zxy_and_translate(self.trig, pose.pos, pose.angle)
                    };
                    let matrix = mtxf_scale_vec3f(&placed, pose.scale);
                    for (out, batch) in frame.vertices.iter_mut().zip(&template.batches) {
                        out.extend(batch.vertices.iter().map(|v| {
                            let mut v = *v;
                            v.position = transform(&matrix, v.position);
                            v
                        }));
                    }
                }
                Some(bones) => {
                    let matrices = blend_bones(
                        previous.filter(|p| p.build == current.build),
                        current,
                        blend_alpha,
                        basis,
                    );
                    for ((out, batch), batch_bones) in
                        frame.vertices.iter_mut().zip(&template.batches).zip(bones)
                    {
                        let lit = batch.material.lights.is_some();
                        out.extend(batch.vertices.iter().zip(batch_bones).map(|(v, &bone)| {
                            let m = matrices.get(usize::from(bone)).unwrap_or(&IDENTITY);
                            let mut v = *v;
                            v.position = transform(m, v.position);
                            if lit {
                                let [nx, ny, nz] = [0, 1, 2].map(|i| f32::from(v.color[i] as i8));
                                let n = std::array::from_fn(|j| {
                                    nx * m[0][j] + ny * m[1][j] + nz * m[2][j]
                                });
                                quantize_normal(n, &mut v.color);
                            }
                            v
                        }));
                    }
                }
            }
        }
        frames.into_values().collect()
    }
}

fn transform(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j] + m[3][j])
}

fn billboard_matrix(basis: BillboardBasis, pos: [f32; 3]) -> Mat4 {
    [
        [basis.right[0], basis.right[1], basis.right[2], 0.0],
        [basis.up[0], basis.up[1], basis.up[2], 0.0],
        [basis.toward[0], basis.toward[1], basis.toward[2], 0.0],
        [pos[0], pos[1], pos[2], 1.0],
    ]
}

/// The bones between the two endpoints (or the current ones), with
/// billboard parts facing the displayed camera.
fn blend_bones(
    previous: Option<&Posed>,
    current: &Posed,
    alpha: Option<f32>,
    basis: BillboardBasis,
) -> Vec<Mat4> {
    current
        .bones
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let m = match (previous.and_then(|p| p.bones.get(i)), alpha) {
                (Some(p), Some(alpha)) => std::array::from_fn(|r| {
                    std::array::from_fn(|k| p[r][k] + (c[r][k] - p[r][k]) * alpha)
                }),
                _ => *c,
            };
            if current.billboards[i] {
                billboard_matrix(basis, [m[3][0], m[3][1], m[3][2]])
            } else {
                m
            }
        })
        .collect()
}

fn blend(
    previous: Option<&VisibleObject>,
    current: &VisibleObject,
    alpha: Option<f32>,
) -> VisibleObject {
    let mut pose = current.clone();
    if let (Some(previous), Some(alpha)) = (previous, alpha) {
        for i in 0..3 {
            pose.pos[i] = previous.pos[i] + (current.pos[i] - previous.pos[i]) * alpha;
            pose.scale[i] = previous.scale[i] + (current.scale[i] - previous.scale[i]) * alpha;
            pose.angle[i] = previous.angle[i].wrapping_add(
                (f32::from(current.angle[i].wrapping_sub(previous.angle[i])) * alpha) as i16,
            );
        }
    }
    pose
}
