use rustario64::{
    import::collision,
    simulation::{
        collision::CollisionWorld,
        mario::constants as c,
        math::{ARCTAN_ENTRIES, SINE_ENTRIES, TrigTables},
    },
};
pub(crate) struct Rng(pub(crate) u64);
impl Rng {
    pub(crate) fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    pub(crate) fn f(&mut self, scale: f32) -> f32 {
        (self.next() as f32 / u32::MAX as f32 - 0.5) * 2.0 * scale
    }
    pub(crate) fn vec(&mut self, scale: f32) -> [f32; 3] {
        std::array::from_fn(|_| self.f(scale))
    }
}

/// Computed approximations, not original ROM table values.
pub(crate) fn tables() -> TrigTables {
    TrigTables::new(
        (0..SINE_ENTRIES)
            .map(|i| (i as f64 * std::f64::consts::TAU / 4096.0).sin() as f32)
            .collect(),
        (0..ARCTAN_ENTRIES)
            .map(|i| ((i as f64 / 1024.0).atan() * 32768.0 / std::f64::consts::PI).round() as u16)
            .collect(),
    )
    .unwrap()
}

/// Sloped bands, camera-filtered floors/ceilings, narrow tunnels, intersecting
/// walls and a water region. Every vertex and word is authored here.
pub(crate) fn terrain() -> Vec<i16> {
    let mut vertices = Vec::<[i16; 3]>::new();
    let mut groups = vec![];
    let mut triangle = |surface: i16, points: [[i16; 3]; 3]| {
        let base = vertices.len() as i16;
        vertices.extend(points);
        groups.push([surface, 1, base, base + 1, base + 2]);
    };
    for i in 0..6 {
        let x = -3000 + i * 1000;
        let y = i * 70;
        let floor_type = [
            0,
            c::SURFACE_NO_CAM_COLLISION,
            c::SURFACE_INTANGIBLE,
            c::SURFACE_WALL_MISC,
            c::SURFACE_CAMERA_BOUNDARY,
            0,
        ][i as usize];
        triangle(
            floor_type,
            [[x + 1000, y + 120, -3000], [x, y, -3000], [x, y, 3000]],
        );
        triangle(
            floor_type,
            [
                [x + 1000, y + 120, -3000],
                [x, y, 3000],
                [x + 1000, y + 120, 3000],
            ],
        );
        if floor_type == c::SURFACE_INTANGIBLE {
            // The reference retries 200 below an intangible floor; without a
            // lower floor it retains a height with a NULL pointer and camera.c
            // dereferences it. Keep random comparisons in the defined domain.
            triangle(
                0,
                [[x, -500, -3000], [x, -500, 3000], [x + 1000, -500, -3000]],
            );
            triangle(
                0,
                [
                    [x + 1000, -500, -3000],
                    [x, -500, 3000],
                    [x + 1000, -500, 3000],
                ],
            );
        }
        let ceil_y = y + if i % 2 == 0 { 200 } else { 1100 };
        let ceil_type = if i == 1 {
            c::SURFACE_NO_CAM_COLLISION
        } else {
            0
        };
        triangle(
            ceil_type,
            [
                [x, ceil_y, -3000],
                [x + 1000, ceil_y, -3000],
                [x, ceil_y, 3000],
            ],
        );
        triangle(
            ceil_type,
            [
                [x + 1000, ceil_y, -3000],
                [x + 1000, ceil_y, 3000],
                [x, ceil_y, 3000],
            ],
        );
    }
    // Four intersecting walls exercise last-wall reuse and query-radius clamps.
    for (surface, x) in [
        (0, -50),
        (0, 50),
        (c::SURFACE_CAMERA_BOUNDARY, 100),
        (c::SURFACE_NO_CAM_COLLISION, 150),
    ] {
        triangle(surface, [[x, -500, -700], [x, 1700, -700], [x, -500, 700]]);
        triangle(surface, [[x, -500, 700], [x, 1700, -700], [x, 1700, 700]]);
        triangle(surface, [[-700, -500, x], [-700, 1700, x], [700, -500, x]]);
        triangle(surface, [[700, -500, x], [-700, 1700, x], [700, 1700, x]]);
    }
    let mut stream = vec![0x40, vertices.len() as i16];
    stream.extend(vertices.into_iter().flatten());
    stream.extend(groups.into_iter().flatten());
    stream.extend([0x41, 0x44, 1, 0, -1200, -1800, 900, 1600, 500, 0x42]);
    stream
}

pub(crate) fn world(stream: &[i16]) -> CollisionWorld {
    let bytes: Vec<u8> = stream.iter().flat_map(|x| x.to_be_bytes()).collect();
    let (mesh, _) = collision::decode(&bytes).unwrap();
    CollisionWorld::load_area_terrain(&mesh).unwrap()
}
