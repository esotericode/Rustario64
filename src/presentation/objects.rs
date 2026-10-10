//! ROM coin/sparkle meshes drawn from completed frames. The authoritative
//! render traversal already selected switches and visibility; this drawer
//! only reads those results. Billboards use the displayed camera's unrolled
//! axes (geo_process_object calls mtxf_billboard with roll 0).
use crate::{
    content::visual::{VisualModel, VisualVertex},
    import::{ImportError, Result, geo, model, objects::ObjectContent},
    simulation::{
        math::{TrigTables, mtxf_rotate_zxy_and_translate, mtxf_scale_vec3f},
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BuildKey {
    model: u16,
    cases: Vec<(usize, usize)>,
}

struct Build {
    key: BuildKey,
    model: VisualModel,
}

/// One uploaded template, repeated for every instance selecting this build.
pub struct ObjectFrame<'a> {
    pub build: usize,
    pub template: &'a VisualModel,
    pub vertices: Vec<Vec<VisualVertex>>,
}

pub struct ObjectDrawer<'a> {
    content: &'a ObjectContent,
    trig: &'a TrigTables,
    builds: Vec<Build>,
    previous: Vec<VisibleObject>,
    current: Vec<VisibleObject>,
}

impl<'a> ObjectDrawer<'a> {
    pub fn new(content: &'a ObjectContent, trig: &'a TrigTables) -> Self {
        Self {
            content,
            trig,
            builds: vec![],
            previous: vec![],
            current: vec![],
        }
    }

    /// Cache each selected display list once, then retain the two endpoints.
    /// Import errors leave the previous successful snapshot intact.
    pub fn update(&mut self, objects: Vec<VisibleObject>) -> Result<()> {
        for o in &objects {
            let key = BuildKey {
                model: o.model,
                cases: o.cases.clone(),
            };
            if self.builds.iter().any(|b| b.key == key) {
                continue;
            }
            let registration = self.content.main_models.get(&o.model).ok_or_else(|| {
                ImportError::new("object drawing", usize::from(o.model), "model not imported")
            })?;
            if !registration.geometry_layout {
                return Err(ImportError::new(
                    "object drawing",
                    registration.pointer as usize,
                    "display-list-only object models are not supported",
                ));
            }
            let layout = geo::decode(&self.content.segments, registration.pointer)?;
            let built = model::build_selected(&self.content.segments, &layout, true, &|node| {
                o.cases
                    .iter()
                    .find(|(n, _)| *n == node)
                    .map(|(_, child)| *child)
            })?;
            // Coin GEO_SHADOW is the only intentionally deferred drawing node.
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
            });
        }
        self.previous = std::mem::replace(&mut self.current, objects);
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
            let previous = self.previous.iter().find(|p| {
                p.id == current.id
                    && p.generation == current.generation
                    && p.behavior == current.behavior
                    && p.model == current.model
                    && p.billboard == current.billboard
            });
            let pose = blend(previous, current, alpha, interpolation);
            let key = BuildKey {
                model: current.model,
                cases: current.cases.clone(),
            };
            let Some(build) = self.builds.iter().position(|b| b.key == key) else {
                continue;
            };
            let template = &self.builds[build].model;
            let frame = frames.entry(build).or_insert_with(|| ObjectFrame {
                build,
                template,
                vertices: vec![vec![]; template.batches.len()],
            });
            let placed = if pose.billboard {
                [
                    [basis.right[0], basis.right[1], basis.right[2], 0.0],
                    [basis.up[0], basis.up[1], basis.up[2], 0.0],
                    [basis.toward[0], basis.toward[1], basis.toward[2], 0.0],
                    [pose.pos[0], pose.pos[1], pose.pos[2], 1.0],
                ]
            } else {
                mtxf_rotate_zxy_and_translate(self.trig, pose.pos, pose.angle)
            };
            let matrix = mtxf_scale_vec3f(&placed, pose.scale);
            for (out, batch) in frame.vertices.iter_mut().zip(&template.batches) {
                out.extend(batch.vertices.iter().map(|v| {
                    let mut v = *v;
                    v.position = std::array::from_fn(|j| {
                        v.position[0] * matrix[0][j]
                            + v.position[1] * matrix[1][j]
                            + v.position[2] * matrix[2][j]
                            + matrix[3][j]
                    });
                    v
                }));
            }
        }
        frames.into_values().collect()
    }
}

fn blend(
    previous: Option<&VisibleObject>,
    current: &VisibleObject,
    alpha: f32,
    enabled: bool,
) -> VisibleObject {
    let mut pose = current.clone();
    if let Some(previous) = previous.filter(|_| enabled && alpha.is_finite()) {
        let alpha = alpha.clamp(0.0, 1.0);
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
