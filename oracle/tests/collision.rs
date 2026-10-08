//! Bitwise differential tests: Rust collision port vs the pinned decomp C.
//! Streams are independently authored by a seeded generator (no ROM content);
//! the ignored test repeats the comparison on the owner's BOB collision.
use rustario64::{
    import::collision,
    simulation::collision::{CollisionFlags, CollisionWorld, NUM_CELLS, WallCollisionData},
};
use rustario64_oracle::Oracle;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() as f32 / u32::MAX as f32)
    }
}

const HAS_FORCE: [i16; 7] = [0x04, 0x0E, 0x24, 0x25, 0x27, 0x2C, 0x2D];

/// One authored collision stream exercising the loader's and queries' cases.
fn authored_stream(seed: u64) -> Vec<i16> {
    let mut rng = Lcg(seed);
    let mut vertices: Vec<[i16; 3]> = vec![];
    let mut vertex = |v: [i32; 3]| {
        vertices.push(v.map(|c| c as i16));
        (vertices.len() - 1) as i16
    };
    // Groups of (surface type, triangles).
    let mut groups: Vec<(i16, Vec<[i16; 3]>)> = vec![];
    // A bumpy floor grid spanning cell borders, wound counter-clockwise from above.
    let mut floor = vec![];
    for gz in 0..6 {
        for gx in 0..6 {
            let x0 = -3000 + gx * 1000 + rng.range(-40, 40);
            let z0 = -3000 + gz * 1000 + rng.range(-40, 40);
            let h = |rng: &mut Lcg| rng.range(-200, 600);
            let a = vertex([x0, h(&mut rng), z0]);
            let b = vertex([x0, h(&mut rng), z0 + 1000]);
            let c = vertex([x0 + 1000, h(&mut rng), z0]);
            let d = vertex([x0 + 1000, h(&mut rng), z0 + 1000]);
            floor.push([a, b, c]);
            floor.push([c, b, d]);
        }
    }
    groups.push((0x0000, floor));
    // Random triangles of mixed orientation, including steep slopes and walls.
    let mut mixed = vec![];
    for _ in 0..60 {
        let cx = rng.range(-7800, 7800);
        let cz = rng.range(-7800, 7800);
        let cy = rng.range(-1000, 4000);
        let size = rng.range(80, 1800);
        let p = |rng: &mut Lcg| {
            [
                cx + rng.range(-size, size),
                cy + rng.range(-size, size),
                cz + rng.range(-size, size),
            ]
        };
        let (a, b, c) = (p(&mut rng), p(&mut rng), p(&mut rng));
        mixed.push([vertex(a), vertex(b), vertex(c)]);
    }
    groups.push((0x0001, mixed));
    // Axis-aligned walls: X-normal walls use the X projection, Z-normal do not.
    let mut walls = vec![];
    for i in 0..8 {
        let x = -2500 + i * 700;
        let z = rng.range(-2000, 2000);
        let (y0, y1) = (rng.range(-300, 100), rng.range(400, 1500));
        let (a, b, c, d) = (
            vertex([x, y0, z]),
            vertex([x, y1, z]),
            vertex([x, y0, z + 900]),
            vertex([x, y1, z + 900]),
        );
        if i % 2 == 0 {
            walls.push([a, b, c]);
            walls.push([c, b, d]);
        } else {
            walls.push([a, c, b]);
            walls.push([c, d, b]);
        }
        let (e, f, g, h) = (
            vertex([z, y0, x]),
            vertex([z, y1, x]),
            vertex([z + 900, y0, x]),
            vertex([z + 900, y1, x]),
        );
        walls.push([e, g, f]);
        walls.push([g, h, f]);
    }
    groups.push((0x0000, walls.clone()));
    groups.push((0x007B, walls[..4].to_vec()));
    groups.push((0x0072, walls[4..8].to_vec()));
    // Ceilings: downward-facing triangles above the floor grid.
    let mut ceilings = vec![];
    for _ in 0..12 {
        let x0 = rng.range(-3000, 2000);
        let z0 = rng.range(-3000, 2000);
        let y = rng.range(300, 1600);
        let (a, b, c) = (
            vertex([x0, y, z0]),
            vertex([x0 + 1200, y + rng.range(-100, 100), z0]),
            vertex([x0, y + rng.range(-100, 100), z0 + 1200]),
        );
        ceilings.push([a, b, c]);
    }
    groups.push((0x0000, ceilings));
    // Stacked intangible and camera-only floors over part of the grid.
    let mut layered = vec![];
    for i in 0..4 {
        let y = 900 + i * 150;
        let (a, b, c) = (
            vertex([-2500, y, -2500]),
            vertex([-2500, y, 1500]),
            vertex([1500, y, -2500]),
        );
        layered.push([a, b, c]);
    }
    groups.push((0x0012, layered[..2].to_vec()));
    groups.push((0x0076, layered[2..3].to_vec()));
    groups.push((0x0072, layered[3..].to_vec()));
    // Degenerate (collinear) triangles must be skipped without allocation.
    let (a, b, c) = (vertex([0, 0, 0]), vertex([10, 0, 10]), vertex([20, 0, 20]));
    groups.push((0x0000, vec![[a, b, c], [a, a, b]]));
    // Surfaces far outside the level box exercise s16 cell-index wraparound.
    let (a, b, c) = (
        vertex([30000, 0, 30000]),
        vertex([30000, 0, 31000]),
        vertex([31000, 0, 30000]),
    );
    let (d, e, f) = (
        vertex([-32000, 50, -30000]),
        vertex([-32000, 50, 8100]),
        vertex([8100, 50, -30000]),
    );
    groups.push((0x0000, vec![[a, b, c], [d, e, f]]));
    // Force-bearing types carry a fourth word.
    let mut forced = vec![];
    for _ in 0..3 {
        let x0 = rng.range(-6000, 6000);
        let z0 = rng.range(-6000, 6000);
        let (a, b, c) = (
            vertex([x0, -500, z0]),
            vertex([x0, -500, z0 + 800]),
            vertex([x0 + 800, -500, z0]),
        );
        forced.push([a, b, c]);
    }
    groups.push((0x000E, forced.clone()));
    groups.push((0x002C, forced));

    let mut s: Vec<i16> = vec![0x40, vertices.len() as i16];
    for v in &vertices {
        s.extend_from_slice(v);
    }
    for (surface, triangles) in &groups {
        s.push(*surface);
        s.push(triangles.len() as i16);
        let force = HAS_FORCE.contains(surface);
        for (i, t) in triangles.iter().enumerate() {
            s.extend_from_slice(t);
            if force {
                s.push((i as i16 + 1) * 37 - 60);
            }
        }
    }
    s.push(0x41);
    // Specials of every record width: 0x79 (0 extra), 0x00 (1), 0x83 (2), 0x1E (3).
    s.extend_from_slice(&[0x43, 4]);
    s.extend_from_slice(&[0x79, 100, 0, 100]);
    s.extend_from_slice(&[0x00, 200, 0, 200, 64]);
    s.extend_from_slice(&[0x83, 300, 0, 300, 64, 5]);
    s.extend_from_slice(&[0x1E, 400, 0, 400, 1, 2, 3]);
    // Water (value < 50) and gas (value >= 50, multiple of 10) boxes.
    s.extend_from_slice(&[0x44, 3]);
    s.extend_from_slice(&[0, -1500, -1500, 1500, 1500, -250]);
    s.extend_from_slice(&[1, -6000, -6000, -4000, -4000, 120]);
    s.extend_from_slice(&[50, -1000, -1000, 4000, 4000, 300]);
    s.push(0x42);
    s
}

fn rust_world(stream: &[i16]) -> CollisionWorld {
    let bytes: Vec<u8> = stream.iter().flat_map(|w| w.to_be_bytes()).collect();
    let (mesh, consumed) = collision::decode(&bytes).expect("authored stream decodes");
    assert_eq!(consumed, bytes.len());
    CollisionWorld::load_area_terrain(&mesh).expect("loads")
}

fn compare_loaded(world: &CollisionWorld, oracle: &Oracle) {
    assert_eq!(
        world.surfaces().len(),
        oracle.surface_count(),
        "surface count"
    );
    assert_eq!(world.node_count(), oracle.node_count(), "node count");
    for (i, s) in world.surfaces().iter().enumerate() {
        let c = oracle.surface(i);
        let ours = (
            s.surface_type,
            s.force,
            s.flags,
            s.room,
            s.lower_y,
            s.upper_y,
            [s.vertex1, s.vertex2, s.vertex3],
            s.normal.map(f32::to_bits),
            s.origin_offset.to_bits(),
        );
        let theirs = (
            c.surface_type,
            c.force,
            c.flags,
            c.room,
            c.lower_y,
            c.upper_y,
            c.vertices,
            c.normal.map(f32::to_bits),
            c.origin_offset.to_bits(),
        );
        assert_eq!(ours, theirs, "surface {i}");
    }
    for cz in 0..NUM_CELLS {
        for cx in 0..NUM_CELLS {
            for kind in 0..3 {
                for dynamic in [false, true] {
                    assert_eq!(
                        world.cell_list(dynamic, cx, cz, kind),
                        oracle.cell_list(dynamic, cx, cz, kind).as_slice(),
                        "cell ({cx}, {cz}) kind {kind} dynamic {dynamic}"
                    );
                }
            }
        }
    }
}

#[derive(Default, Debug)]
struct Hits {
    comparisons: usize,
    floors: usize,
    ceilings: usize,
    walls: usize,
    intangible_retries: usize,
    water: usize,
    gas: usize,
}

/// Compare every query at one point and count non-trivial results.
fn compare_point(
    world: &CollisionWorld,
    oracle: &Oracle,
    p: [f32; 3],
    radius: f32,
    hits: &mut Hits,
) {
    let mut n = 0;
    for for_camera in [false, true] {
        for include in [false, true] {
            let mut flags = CollisionFlags {
                checking_for_camera: for_camera,
                find_floor_include_surface_intangible: include,
            };
            let (h, floor) = world.find_floor(p[0], p[1], p[2], &mut flags);
            let (ch, cfloor, cafter) = oracle.find_floor(p, include, for_camera);
            hits.floors += usize::from(floor.is_some());
            if !include && floor.is_some() {
                let mut with = CollisionFlags {
                    checking_for_camera: for_camera,
                    find_floor_include_surface_intangible: true,
                };
                hits.intangible_retries +=
                    usize::from(world.find_floor(p[0], p[1], p[2], &mut with).1 != floor);
            }
            assert_eq!(
                (
                    h.to_bits(),
                    floor,
                    flags.find_floor_include_surface_intangible
                ),
                (ch.to_bits(), cfloor, cafter),
                "find_floor {p:?} camera {for_camera} intangible {include}"
            );
            n += 1;
        }
        let flags = CollisionFlags {
            checking_for_camera: for_camera,
            find_floor_include_surface_intangible: false,
        };
        let (h, ceil) = world.find_ceil(p[0], p[1], p[2], flags);
        let (ch, cceil) = oracle.find_ceil(p, for_camera);
        hits.ceilings += usize::from(ceil.is_some());
        assert_eq!(
            (h.to_bits(), ceil),
            (ch.to_bits(), cceil),
            "find_ceil {p:?}"
        );
        n += 1;
        for pass_vanish in [false, true] {
            for offset_y in [0.0, 60.0, 150.0] {
                let mut data = WallCollisionData::new(p, offset_y, radius);
                let collisions = world.find_wall_collisions(&mut data, flags, pass_vanish);
                hits.walls += usize::from(collisions > 0);
                let c = oracle.find_walls(p, offset_y, radius, for_camera, pass_vanish);
                let ours = (
                    [data.x, data.y, data.z].map(f32::to_bits),
                    collisions,
                    data.num_walls,
                    &data.walls[..data.num_walls as usize],
                );
                let theirs = (
                    c.position.map(f32::to_bits),
                    c.collisions,
                    c.num_walls,
                    &c.walls[..c.num_walls as usize],
                );
                assert_eq!(ours, theirs, "walls {p:?} r {radius} oy {offset_y}");
                n += 1;
            }
        }
    }
    let water = world.find_water_level(p[0], p[2]);
    assert_eq!(
        water.to_bits(),
        oracle.find_water_level(p[0], p[2]).to_bits()
    );
    let gas = world.find_poison_gas_level(p[0], p[2]);
    assert_eq!(
        gas.to_bits(),
        oracle.find_poison_gas_level(p[0], p[2]).to_bits()
    );
    hits.water += usize::from(water != -11000.0);
    hits.gas += usize::from(gas != -11000.0);
    hits.comparisons += n + 2;
}

fn query_points(world: &CollisionWorld, seed: u64, random: usize) -> Vec<([f32; 3], f32)> {
    let mut rng = Lcg(seed ^ 0x5DEECE66D);
    let mut points = vec![];
    for _ in 0..random {
        points.push((
            [
                rng.f(-9000.0, 9000.0),
                rng.f(-1500.0, 5000.0),
                rng.f(-9000.0, 9000.0),
            ],
            rng.f(0.0, 260.0),
        ));
    }
    // Exactly on, and just beside, every vertex and cell border.
    for s in world.surfaces().iter().take(400) {
        for v in [s.vertex1, s.vertex2, s.vertex3] {
            let [x, y, z] = v.map(f32::from);
            for (dx, dz) in [(0.0, 0.0), (0.5, -0.5), (-0.99, 0.99), (37.0, 0.0)] {
                points.push(([x + dx, y + 20.0, z + dz], 50.0));
                points.push(([x + dx, y - 100.0, z + dz], 120.0));
            }
        }
    }
    // Either side of the 78-unit floor and ceiling buffers, above and below each
    // surface's plane at its centroid.
    for s in world.surfaces().iter().take(400) {
        if s.normal[1] == 0.0 {
            continue;
        }
        let centroid = |axis: usize| {
            (i32::from(s.vertex1[axis]) + i32::from(s.vertex2[axis]) + i32::from(s.vertex3[axis]))
                as f32
                / 3.0
        };
        let (x, z) = (centroid(0), centroid(2));
        let h = -(s.normal[0] * x + s.normal[2] * z + s.origin_offset) / s.normal[1];
        for dy in [-78.5, -77.5, 77.5, 78.5] {
            points.push(([x, h + dy, z], 50.0));
        }
    }
    for border in (-8192..=8192).step_by(1024) {
        let b = border as f32;
        for offset in [-1.0, -0.25, 0.0, 0.25, 1.0, 49.5, 50.5] {
            points.push(([b + offset, 300.0, 123.0], 100.0));
            points.push(([-777.0, 300.0, b + offset], 100.0));
        }
    }
    // Large magnitudes wrap through (s16) casts ("parallel universes").
    for x in [65536.0 + 100.0, -65536.0 - 2000.0, 131072.0 + 4000.0, 1.0e6] {
        points.push(([x, 200.0, 300.0], 80.0));
        points.push(([300.0, 200.0, x], 80.0));
    }
    points
}

#[test]
fn authored_streams_load_and_query_identically_to_the_decomp() {
    let mut hits = Hits::default();
    for seed in [1u64, 2, 3, 0xBEEF, 0x5EED_1234] {
        let stream = authored_stream(seed);
        let world = rust_world(&stream);
        let oracle = Oracle::load(&stream);
        compare_loaded(&world, &oracle);
        assert!(world.surfaces().len() > 150);
        for (p, radius) in query_points(&world, seed, 3000) {
            compare_point(&world, &oracle, p, radius, &mut hits);
        }
    }
    println!("{hits:?}");
    assert!(hits.comparisons > 200_000, "{hits:?}");
    // Coverage guard: the queries must actually hit each kind of result.
    assert!(
        hits.floors > 10_000 && hits.ceilings > 1_000 && hits.walls > 1_000,
        "{hits:?}"
    );
    assert!(
        hits.intangible_retries > 10 && hits.water > 100 && hits.gas > 100,
        "{hits:?}"
    );
}

#[test]
fn surface_pool_limits_match_the_original_allocation() {
    // 2300 surfaces in one cell fill the pool exactly; the original prints a
    // debug message at the limit, Rust reports an error beyond it.
    let mut s: Vec<i16> = vec![0x40, 3, 0, 0, 0, 0, 0, 100, 100, 0, 0, 0, 2301];
    for _ in 0..2301 {
        s.extend_from_slice(&[0, 1, 2]);
    }
    s.push(0x42);
    let bytes: Vec<u8> = s.iter().flat_map(|w| w.to_be_bytes()).collect();
    let (mesh, _) = collision::decode(&bytes).unwrap();
    assert!(CollisionWorld::load_area_terrain(&mesh).is_err());
}

#[test]
#[ignore = "requires a privately supplied supported ROM via RUSTARIO64_ROM"]
fn bob_collision_matches_the_decomp_on_a_dense_grid() {
    use rustario64::import::{bob, mio0, rom::Rom, version};
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let range = version::BOB_TERRAIN;
    let terrain = mio0::decode(
        rom.reader().slice(range.start, range.len()).unwrap(),
        version::MAX_SEGMENT_BYTES,
    )
    .unwrap();
    let start = (version::BOB_COLLISION & 0xFFFFFF) as usize;
    let (mesh, consumed) = collision::decode(&terrain[start..]).unwrap();
    let words: Vec<i16> = terrain[start..start + consumed]
        .chunks_exact(2)
        .map(|b| i16::from_be_bytes([b[0], b[1]]))
        .collect();
    assert_eq!(mesh, bob::import(&rom).unwrap().collision);
    let world = CollisionWorld::load_area_terrain(&mesh).unwrap();
    let oracle = Oracle::load(&words);
    compare_loaded(&world, &oracle);
    assert_eq!(world.surfaces().len(), 1060);
    let mut hits = Hits::default();
    for z in (-8200..=8200).step_by(97) {
        for x in (-8200..=8200).step_by(97) {
            for y in [-500.0, 0.0, 400.0, 1200.0, 2600.0, 4300.0] {
                compare_point(&world, &oracle, [x as f32, y, z as f32], 50.0, &mut hits);
            }
        }
    }
    for (p, radius) in query_points(&world, 64, 20000) {
        compare_point(&world, &oracle, p, radius, &mut hits);
    }
    println!("BOB collision comparisons identical: {hits:?}");
    assert!(hits.comparisons > 1_000_000 && hits.floors > 100_000 && hits.walls > 1_000);
}
