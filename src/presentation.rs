//! Read-only snapshots. Graphics configuration cannot enter the tick scheduler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot {
    pub entity: u32,
    pub epoch: u64,
    pub position: [f32; 3],
    pub yaw: i16,
    pub animation: u16,
    pub discontinuity: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub position: [f32; 3],
    pub yaw: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsOptions {
    pub interpolation: bool,
    pub enhanced_lighting: bool,
    pub dynamic_shadows: bool,
}
impl Default for GraphicsOptions {
    fn default() -> Self {
        Self {
            interpolation: true,
            enhanced_lighting: false,
            dynamic_shadows: false,
        }
    }
}

pub fn interpolate(
    previous: Option<&Snapshot>,
    current: &Snapshot,
    alpha: f32,
    options: GraphicsOptions,
) -> Pose {
    let snap = Pose {
        position: current.position,
        yaw: current.yaw,
    };
    let Some(previous) = previous else {
        return snap;
    };
    if !options.interpolation
        || current.discontinuity
        || previous.entity != current.entity
        || previous.epoch != current.epoch
        || previous.animation != current.animation
        || !alpha.is_finite()
    {
        return snap;
    }
    let alpha = alpha.clamp(0.0, 1.0);
    let mut position = [0.0; 3];
    for (i, value) in position.iter_mut().enumerate() {
        *value = previous.position[i] + (current.position[i] - previous.position[i]) * alpha;
    }
    let delta = current.yaw.wrapping_sub(previous.yaw);
    let yaw = previous.yaw.wrapping_add((f32::from(delta) * alpha) as i16);
    Pose { position, yaw }
}
