//! Authored fixtures shared by the full-tick suites: a deterministic
//! generator, computed trig tables, an authored animation table and a
//! collision stream builder. Nothing here is game data.
#![allow(dead_code)]
use rustario64::{
    content::animation::{
        ANIM_FLAG_2, ANIM_FLAG_5, ANIM_FLAG_6, ANIM_FLAG_BACKWARD, ANIM_FLAG_HOR_TRANS,
        ANIM_FLAG_NOLOOP, ANIM_FLAG_VERT_TRANS, Animation, MarioAnimations,
    },
    import::collision,
    simulation::{
        collision::CollisionWorld,
        mario::constants as c,
        math::{ARCTAN_ENTRIES, SINE_ENTRIES, TrigTables},
    },
};

pub(crate) struct Lcg(pub(crate) u64);

impl Lcg {
    pub(crate) fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    pub(crate) fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }
    pub(crate) fn chance(&mut self, one_in: u32) -> bool {
        self.next().is_multiple_of(one_in)
    }
}

/// Computed approximations of the tables' shape; not the game's values.
pub(crate) fn computed_tables() -> TrigTables {
    let sine = (0..SINE_ENTRIES)
        .map(|i| ((i as f64) * std::f64::consts::TAU / 4096.0).sin() as f32)
        .collect();
    let arctan = (0..ARCTAN_ENTRIES)
        .map(|i| ((i as f64 / 1024.0).atan() * 32768.0 / std::f64::consts::PI).round() as u16)
        .collect();
    TrigTables::new(sine, arctan).unwrap()
}

/// An independently authored animation table: one entry per MARIO_ANIM_* ID
/// with varied lengths, loop points, flags and root translations. Not the
/// game's animations, so action timing differs from the game; the
/// comparison only needs both sides to read the same data.
pub(crate) fn authored_animations(seed: u64) -> MarioAnimations {
    let mut rng = Lcg(seed);
    let animations = (0..0xD1)
        .map(|_| {
            let loop_end = rng.range(2, 40) as i16;
            let loop_start = if rng.chance(4) {
                rng.range(0, i32::from(loop_end) - 1) as i16
            } else {
                0
            };
            let start_frame = if rng.chance(6) {
                rng.range(0, i32::from(loop_end) - 1) as i16
            } else {
                0
            };
            let mut flags = 0;
            if rng.chance(4) {
                flags |= ANIM_FLAG_NOLOOP;
            }
            if rng.chance(12) {
                flags |= ANIM_FLAG_BACKWARD;
            }
            if rng.chance(50) {
                flags |= ANIM_FLAG_2;
            }
            flags |= match rng.next() % 6 {
                0 => ANIM_FLAG_HOR_TRANS,
                1 => ANIM_FLAG_VERT_TRANS,
                2 => ANIM_FLAG_6,
                _ => 0,
            };
            if rng.chance(5) {
                flags |= ANIM_FLAG_5;
            }
            let bone_count = 20i16;
            let attributes = 3 + 3 * bone_count as usize;
            let mut index = Vec::with_capacity(attributes * 2);
            let mut values = Vec::new();
            for attribute in 0..attributes {
                let frames = if rng.chance(3) {
                    1
                } else {
                    rng.range(1, i32::from(loop_end))
                };
                index.push(frames as u16);
                index.push(values.len() as u16);
                for _ in 0..frames {
                    values.push(if attribute < 3 {
                        rng.range(-160, 160) as i16
                    } else {
                        rng.range(-0x8000, 0x7FFF) as i16
                    });
                }
            }
            Animation {
                flags,
                y_trans_divisor: 189,
                start_frame,
                loop_start,
                loop_end,
                bone_count,
                index,
                values,
            }
        })
        .collect();
    MarioAnimations { animations }
}

/// Collision stream builder: triangles are wound to face the requested way.
#[derive(Default)]
pub(crate) struct Builder {
    vertices: Vec<[i16; 3]>,
    groups: Vec<(i16, Vec<[i16; 3]>, Vec<i16>)>,
}

impl Builder {
    pub(crate) fn vertex(&mut self, p: [i32; 3]) -> i16 {
        self.vertices.push(p.map(|x| x as i16));
        (self.vertices.len() - 1) as i16
    }

    pub(crate) fn triangle(
        &mut self,
        surface: i16,
        p: [[i32; 3]; 3],
        facing: [i32; 3],
        force: Option<i16>,
    ) {
        let sub = |a: [i32; 3], b: [i32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let (u, v) = (sub(p[1], p[0]), sub(p[2], p[1]));
        let n = [
            i64::from(u[1]) * i64::from(v[2]) - i64::from(u[2]) * i64::from(v[1]),
            i64::from(u[2]) * i64::from(v[0]) - i64::from(u[0]) * i64::from(v[2]),
            i64::from(u[0]) * i64::from(v[1]) - i64::from(u[1]) * i64::from(v[0]),
        ];
        let dot: i64 = (0..3).map(|i| n[i] * i64::from(facing[i])).sum();
        let p = if dot < 0 { [p[0], p[2], p[1]] } else { p };
        let tri = p.map(|q| self.vertex(q));
        match self.groups.last_mut() {
            Some((t, tris, forces)) if *t == surface && forces.is_empty() == force.is_none() => {
                tris.push(tri);
                forces.extend(force);
            }
            _ => self
                .groups
                .push((surface, vec![tri], force.into_iter().collect())),
        }
    }

    pub(crate) fn quad(
        &mut self,
        surface: i16,
        p: [[i32; 3]; 4],
        facing: [i32; 3],
        force: Option<i16>,
    ) {
        self.triangle(surface, [p[0], p[1], p[2]], facing, force);
        self.triangle(surface, [p[2], p[1], p[3]], facing, force);
    }

    /// A horizontal rectangle at `y`, facing up (floor) or down (ceiling).
    pub(crate) fn flat(&mut self, surface: i16, x: [i32; 2], z: [i32; 2], y: i32, up: bool) {
        let p = [
            [x[0], y, z[0]],
            [x[0], y, z[1]],
            [x[1], y, z[0]],
            [x[1], y, z[1]],
        ];
        self.quad(surface, p, [0, if up { 1 } else { -1 }, 0], None);
    }

    /// A box with a floor on top and four outward walls.
    pub(crate) fn block(&mut self, top: i16, x: [i32; 2], z: [i32; 2], y: [i32; 2]) {
        self.flat(top, x, z, y[1], true);
        let wall = c::SURFACE_DEFAULT;
        for (xx, dir) in [(x[0], -1), (x[1], 1)] {
            let p = [
                [xx, y[0], z[0]],
                [xx, y[1], z[0]],
                [xx, y[0], z[1]],
                [xx, y[1], z[1]],
            ];
            self.quad(wall, p, [dir, 0, 0], None);
        }
        for (zz, dir) in [(z[0], -1), (z[1], 1)] {
            let p = [
                [x[0], y[0], zz],
                [x[0], y[1], zz],
                [x[1], y[0], zz],
                [x[1], y[1], zz],
            ];
            self.quad(wall, p, [0, 0, dir], None);
        }
    }

    /// A floor rising along +z from `y[0]` at `z[0]` to `y[1]` at `z[1]`.
    /// Floors sort by their first vertex's height, so both triangles start
    /// on the high edge; otherwise the field under the ramp would shadow it.
    pub(crate) fn ramp(&mut self, surface: i16, x: [i32; 2], z: [i32; 2], y: [i32; 2]) {
        let (top0, top1) = ([x[0], y[1], z[1]], [x[1], y[1], z[1]]);
        let (bottom0, bottom1) = ([x[0], y[0], z[0]], [x[1], y[0], z[0]]);
        self.triangle(surface, [top0, top1, bottom0], [0, 1, 0], None);
        self.triangle(surface, [top1, bottom1, bottom0], [0, 1, 0], None);
    }

    pub(crate) fn stream(&self) -> Vec<i16> {
        let mut s: Vec<i16> = vec![0x40, self.vertices.len() as i16];
        for p in &self.vertices {
            s.extend_from_slice(p);
        }
        for (t, tris, forces) in &self.groups {
            s.push(*t);
            s.push(tris.len() as i16);
            for (i, tri) in tris.iter().enumerate() {
                s.extend_from_slice(tri);
                if !forces.is_empty() {
                    s.push(forces[i]);
                }
            }
        }
        s.push(0x41);
        s.push(0x42);
        s
    }
}

pub(crate) fn world(stream: &[i16]) -> CollisionWorld {
    let bytes: Vec<u8> = stream.iter().flat_map(|w| w.to_be_bytes()).collect();
    let (mesh, consumed) = collision::decode(&bytes).unwrap();
    assert_eq!(consumed, bytes.len());
    CollisionWorld::load_area_terrain(&mesh).unwrap()
}

/// The authored behavior segment and model table (simulation::object's
/// ROM-free fixtures), shared for the life of the test process.
pub(crate) static SCRIPTS: std::sync::LazyLock<
    rustario64::simulation::object::script::BehaviorScripts,
> = std::sync::LazyLock::new(rustario64::simulation::object::script::authored_scripts);
pub(crate) static MODELS: std::sync::LazyLock<
    rustario64::simulation::object::render::ObjectModels,
> = std::sync::LazyLock::new(rustario64::simulation::object::render::authored_models);

/// Mario alone, with the authored scripts and models.
pub(crate) fn mario_only() -> rustario64::simulation::mario::tick::LevelObjects<'static> {
    rustario64::simulation::mario::tick::LevelObjects::mario_only(&SCRIPTS, &MODELS)
}
