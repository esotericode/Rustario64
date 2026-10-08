//! Original static collision loading and floor/ceiling/wall/environment queries.
//!
//! Translated from pinned CC0 n64decomp/sm64 src/engine/surface_load.c and
//! src/engine/surface_collision.c (see PROVENANCE.md). Integer widths, casts,
//! f32 operation order, double-precision thresholds, s16 wraparound, partition
//! ordering, and first-match quirks are preserved deliberately; do not "fix"
//! them here. Intentional gameplay changes belong in separate optional modes.
//!
//! Known boundary: C `(s16)f32` is reproduced as truncation to s32 followed by
//! 16-bit wraparound. For |value| >= 2^31 or NaN the N64 conversion is not
//! reproduced (Rust saturates); such positions are outside supported coverage.
use crate::content::CollisionMesh;
use std::fmt;

pub const LEVEL_BOUNDARY_MAX: i16 = 0x2000;
pub const CELL_SIZE: i16 = 0x400;
pub const NUM_CELLS: usize = 16;
pub const NUM_CELLS_INDEX: i16 = 15;
pub const CELL_HEIGHT_LIMIT: f32 = 20000.0;
pub const FLOOR_LOWER_LIMIT: f32 = -11000.0;
/// alloc_surface_pools: 2300 surfaces and 7000 partition nodes.
pub const SURFACE_POOL_SIZE: usize = 2300;
pub const SURFACE_NODE_POOL_SIZE: usize = 7000;

pub const SURFACE_FLAG_DYNAMIC: i8 = 1 << 0;
pub const SURFACE_FLAG_NO_CAM_COLLISION: i8 = 1 << 1;
pub const SURFACE_FLAG_X_PROJECTION: i8 = 1 << 3;

pub const SURFACE_INTANGIBLE: i16 = 0x0012;
pub const SURFACE_CAMERA_BOUNDARY: i16 = 0x0072;
pub const SURFACE_NO_CAM_COLLISION: i16 = 0x0076;
pub const SURFACE_NO_CAM_COLLISION_77: i16 = 0x0077;
pub const SURFACE_NO_CAM_COL_VERY_SLIPPERY: i16 = 0x0078;
pub const SURFACE_SWITCH: i16 = 0x007A;
pub const SURFACE_VANISH_CAP_WALLS: i16 = 0x007B;

const FLOORS: usize = 0;
const CEILS: usize = 1;
const WALLS: usize = 2;

/// Index of a surface in allocation order (the original pool order).
pub type SurfaceIndex = u16;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub surface_type: i16,
    pub force: i16,
    pub flags: i8,
    pub room: i8,
    pub lower_y: i16,
    pub upper_y: i16,
    pub vertex1: [i16; 3],
    pub vertex2: [i16; 3],
    pub vertex3: [i16; 3],
    pub normal: [f32; 3],
    pub origin_offset: f32,
    /// Owning object for dynamic surfaces (stable object ID, not a pointer).
    pub object: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollisionError {
    SurfacePoolOverflow,
    NodePoolOverflow,
    VertexIndex { triangle: usize },
}

impl fmt::Display for CollisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SurfacePoolOverflow => write!(f, "surface pool ({SURFACE_POOL_SIZE}) exhausted"),
            Self::NodePoolOverflow => {
                write!(f, "surface node pool ({SURFACE_NODE_POOL_SIZE}) exhausted")
            }
            Self::VertexIndex { triangle } => {
                write!(f, "triangle {triangle} references a missing vertex")
            }
        }
    }
}

impl std::error::Error for CollisionError {}

/// Global switches the original reads while querying collision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CollisionFlags {
    /// gCheckingSurfaceCollisionsForCamera.
    pub checking_for_camera: bool,
    /// gFindFloorIncludeSurfaceIntangible; find_floor clears it after use.
    pub find_floor_include_surface_intangible: bool,
}

/// struct WallCollisionData. Only the first `num_walls` entries of `walls` are
/// written by a query, as in the original.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallCollisionData {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub offset_y: f32,
    pub radius: f32,
    pub num_walls: i16,
    pub walls: [Option<SurfaceIndex>; 4],
}

impl WallCollisionData {
    pub fn new(position: [f32; 3], offset_y: f32, radius: f32) -> Self {
        Self {
            x: position[0],
            y: position[1],
            z: position[2],
            offset_y,
            radius,
            num_walls: 0,
            walls: [None; 4],
        }
    }
}

/// C `(s16)value` / `(TerrainData)value` for an f32, as compiled for the N64.
pub fn f32_to_s16(value: f32) -> i16 {
    (value as i32) as i16
}

type Partition = Vec<[Vec<SurfaceIndex>; 3]>;

fn empty_partition() -> Partition {
    (0..NUM_CELLS * NUM_CELLS)
        .map(|_| [vec![], vec![], vec![]])
        .collect()
}

fn surface_has_force(surface_type: i16) -> bool {
    matches!(surface_type, 0x04 | 0x0E | 0x24 | 0x25 | 0x27 | 0x2C | 0x2D)
}

fn surf_has_no_cam_collision(surface_type: i16) -> i8 {
    match surface_type {
        SURFACE_NO_CAM_COLLISION
        | SURFACE_NO_CAM_COLLISION_77
        | SURFACE_NO_CAM_COL_VERY_SLIPPERY
        | SURFACE_SWITCH => SURFACE_FLAG_NO_CAM_COLLISION,
        _ => 0,
    }
}

fn min_3(a0: i16, a1: i16, a2: i16) -> i16 {
    let mut a = a0;
    if a1 < a {
        a = a1;
    }
    if a2 < a {
        a = a2;
    }
    a
}

fn max_3(a0: i16, a1: i16, a2: i16) -> i16 {
    let mut a = a0;
    if a1 > a {
        a = a1;
    }
    if a2 > a {
        a = a2;
    }
    a
}

fn lower_cell_index(coord: i16) -> i16 {
    let mut coord = coord.wrapping_add(LEVEL_BOUNDARY_MAX);
    if coord < 0 {
        coord = 0;
    }
    let mut index = coord / CELL_SIZE;
    if coord % CELL_SIZE < 50 {
        index -= 1;
    }
    if index < 0 {
        index = 0;
    }
    index
}

fn upper_cell_index(coord: i16) -> i16 {
    let mut coord = coord.wrapping_add(LEVEL_BOUNDARY_MAX);
    if coord < 0 {
        coord = 0;
    }
    let mut index = coord / CELL_SIZE;
    if coord % CELL_SIZE > CELL_SIZE - 50 {
        index += 1;
    }
    if index > NUM_CELLS_INDEX {
        index = NUM_CELLS_INDEX;
    }
    index
}

/// read_surface_data: integer cross product, f32 magnitude, double reciprocal.
pub fn read_surface_data(v1: [i16; 3], v2: [i16; 3], v3: [i16; 3]) -> Option<Surface> {
    let [x1, y1, z1] = v1.map(i32::from);
    let [x2, y2, z2] = v2.map(i32::from);
    let [x3, y3, z3] = v3.map(i32::from);
    let cross = |a: i32, b: i32, c: i32, d: i32| a.wrapping_mul(b).wrapping_sub(c.wrapping_mul(d));
    // (v2 - v1) x (v3 - v2), in s32, then converted to f32.
    let mut nx = cross(y2 - y1, z3 - z2, z2 - z1, y3 - y2) as f32;
    let mut ny = cross(z2 - z1, x3 - x2, x2 - x1, z3 - z2) as f32;
    let mut nz = cross(x2 - x1, y3 - y2, y2 - y1, x3 - x2) as f32;
    let mag = (nx * nx + ny * ny + nz * nz).sqrt();
    let mut min_y = y1;
    if y2 < min_y {
        min_y = y2;
    }
    if y3 < min_y {
        min_y = y3;
    }
    let mut max_y = y1;
    if y2 > max_y {
        max_y = y2;
    }
    if y3 > max_y {
        max_y = y3;
    }
    if f64::from(mag) < 0.0001 {
        return None;
    }
    let mag = (1.0f64 / f64::from(mag)) as f32;
    nx *= mag;
    ny *= mag;
    nz *= mag;
    Some(Surface {
        surface_type: 0,
        force: 0,
        flags: 0,
        room: 0,
        lower_y: (min_y - 5) as i16,
        upper_y: (max_y + 5) as i16,
        vertex1: v1,
        vertex2: v2,
        vertex3: v3,
        normal: [nx, ny, nz],
        origin_offset: -(nx * x1 as f32 + ny * y1 as f32 + nz * z1 as f32),
        object: None,
    })
}

/// Static level collision plus a dynamic partition for object surfaces.
#[derive(Debug, Clone)]
pub struct CollisionWorld {
    surfaces: Vec<Surface>,
    static_surfaces: usize,
    nodes: usize,
    static_nodes: usize,
    static_cells: Partition,
    dynamic_cells: Partition,
    /// gEnvironmentRegions: [val, loX, loZ, hiX, hiZ, height] per region. Owned
    /// and mutable because behaviors (e.g. WDW water level) rewrite it.
    pub environment: Option<Vec<[i16; 6]>>,
}

impl CollisionWorld {
    /// load_area_terrain for the surface and environment parts of a collision
    /// stream. Special-object spawning and macro objects belong to object code.
    pub fn load_area_terrain(mesh: &CollisionMesh) -> Result<Self, CollisionError> {
        let mut world = Self {
            surfaces: vec![],
            static_surfaces: 0,
            nodes: 0,
            static_nodes: 0,
            static_cells: empty_partition(),
            dynamic_cells: empty_partition(),
            environment: None,
        };
        let vertex = |triangle: usize, index: u16| {
            mesh.vertices
                .get(usize::from(index))
                .copied()
                .ok_or(CollisionError::VertexIndex { triangle })
        };
        for (i, triangle) in mesh.triangles.iter().enumerate() {
            let [a, b, c] = triangle.indices;
            let Some(mut surface) = read_surface_data(vertex(i, a)?, vertex(i, b)?, vertex(i, c)?)
            else {
                continue;
            };
            surface.room = 0;
            surface.surface_type = triangle.surface;
            surface.flags = surf_has_no_cam_collision(triangle.surface);
            surface.force = if surface_has_force(triangle.surface) {
                triangle.force.unwrap_or(0)
            } else {
                0
            };
            let index = world.alloc(surface)?;
            world.add_surface(index, false)?;
        }
        if !mesh.environment.is_empty() {
            world.environment = Some(mesh.environment.clone());
        }
        world.static_surfaces = world.surfaces.len();
        world.static_nodes = world.nodes;
        Ok(world)
    }

    fn alloc(&mut self, surface: Surface) -> Result<SurfaceIndex, CollisionError> {
        if self.surfaces.len() >= SURFACE_POOL_SIZE {
            return Err(CollisionError::SurfacePoolOverflow);
        }
        self.surfaces.push(surface);
        Ok((self.surfaces.len() - 1) as SurfaceIndex)
    }

    fn add_surface(&mut self, index: SurfaceIndex, dynamic: bool) -> Result<(), CollisionError> {
        let s = self.surfaces[usize::from(index)];
        let min_x = min_3(s.vertex1[0], s.vertex2[0], s.vertex3[0]);
        let min_z = min_3(s.vertex1[2], s.vertex2[2], s.vertex3[2]);
        let max_x = max_3(s.vertex1[0], s.vertex2[0], s.vertex3[0]);
        let max_z = max_3(s.vertex1[2], s.vertex2[2], s.vertex3[2]);
        let (min_cell_x, max_cell_x) = (lower_cell_index(min_x), upper_cell_index(max_x));
        let (min_cell_z, max_cell_z) = (lower_cell_index(min_z), upper_cell_index(max_z));
        for cell_z in min_cell_z..=max_cell_z {
            for cell_x in min_cell_x..=max_cell_x {
                self.add_surface_to_cell(dynamic, cell_x, cell_z, index)?;
            }
        }
        Ok(())
    }

    fn add_surface_to_cell(
        &mut self,
        dynamic: bool,
        cell_x: i16,
        cell_z: i16,
        index: SurfaceIndex,
    ) -> Result<(), CollisionError> {
        if self.nodes >= SURFACE_NODE_POOL_SIZE {
            return Err(CollisionError::NodePoolOverflow);
        }
        self.nodes += 1;
        let surface = &mut self.surfaces[usize::from(index)];
        let normal_y = f64::from(surface.normal[1]);
        let (list_index, sort_dir) = if normal_y > 0.01 {
            (FLOORS, 1i16)
        } else if normal_y < -0.01 {
            (CEILS, -1)
        } else {
            let normal_x = f64::from(surface.normal[0]);
            // Keep the original comparison form (it is false for NaN).
            #[allow(clippy::manual_range_contains)]
            if normal_x < -0.707 || normal_x > 0.707 {
                surface.flags |= SURFACE_FLAG_X_PROJECTION;
            }
            (WALLS, 0)
        };
        // Sorted by the first vertex's height (the original "surface cucking").
        let priority_of = |s: &Surface| s.vertex1[1].wrapping_mul(sort_dir);
        let priority = priority_of(surface);
        let surfaces = &self.surfaces;
        let partition = if dynamic {
            &mut self.dynamic_cells
        } else {
            &mut self.static_cells
        };
        // Cells outside 0..16 are unreachable: the loop bounds keep them in range.
        let cell = &mut partition[cell_z as usize * NUM_CELLS + cell_x as usize][list_index];
        let at = cell
            .iter()
            .position(|&other| priority > priority_of(&surfaces[usize::from(other)]))
            .unwrap_or(cell.len());
        cell.insert(at, index);
        Ok(())
    }

    /// clear_dynamic_surfaces (the caller handles the time-stop condition).
    pub fn clear_dynamic_surfaces(&mut self) {
        self.surfaces.truncate(self.static_surfaces);
        self.nodes = self.static_nodes;
        self.dynamic_cells = empty_partition();
    }

    pub fn surfaces(&self) -> &[Surface] {
        &self.surfaces
    }

    pub fn surface(&self, index: SurfaceIndex) -> &Surface {
        &self.surfaces[usize::from(index)]
    }

    pub fn static_surface_count(&self) -> usize {
        self.static_surfaces
    }

    pub fn node_count(&self) -> usize {
        self.nodes
    }

    /// Ordered surface list for a cell: kind 0 floors, 1 ceilings, 2 walls.
    pub fn cell_list(&self, dynamic: bool, cell_x: usize, cell_z: usize, kind: usize) -> &[u16] {
        let partition = if dynamic {
            &self.dynamic_cells
        } else {
            &self.static_cells
        };
        &partition[cell_z * NUM_CELLS + cell_x][kind]
    }

    fn cell_of(x: i16, z: i16) -> usize {
        let cell = |v: i16| {
            ((i32::from(v) + i32::from(LEVEL_BOUNDARY_MAX)) / i32::from(CELL_SIZE))
                & i32::from(NUM_CELLS_INDEX)
        };
        cell(z) as usize * NUM_CELLS + cell(x) as usize
    }

    fn out_of_bounds(v: i16) -> bool {
        v <= -LEVEL_BOUNDARY_MAX || v >= LEVEL_BOUNDARY_MAX
    }

    fn find_wall_collisions_from_list(
        &self,
        list: &[SurfaceIndex],
        data: &mut WallCollisionData,
        flags: CollisionFlags,
        pass_vanish_walls: bool,
    ) -> i32 {
        let mut radius = data.radius;
        let x = data.x;
        let y = data.y + data.offset_y;
        let z = data.z;
        let mut collisions = 0;
        if radius > 200.0 {
            radius = 200.0;
        }
        for &index in list {
            let surf = self.surface(index);
            if y < f32::from(surf.lower_y) || y > f32::from(surf.upper_y) {
                continue;
            }
            let offset =
                surf.normal[0] * x + surf.normal[1] * y + surf.normal[2] * z + surf.origin_offset;
            if offset < -radius || offset > radius {
                continue;
            }
            let px = x;
            let pz = z;
            let [y1, y2, y3] = [surf.vertex1[1], surf.vertex2[1], surf.vertex3[1]].map(f32::from);
            let outside = |w1: f32, w2: f32, w3: f32, p: f32, positive: bool| {
                let e1 = (y1 - y) * (w2 - w1) - (w1 - p) * (y2 - y1);
                let e2 = (y2 - y) * (w3 - w2) - (w2 - p) * (y3 - y2);
                let e3 = (y3 - y) * (w1 - w3) - (w3 - p) * (y1 - y3);
                if positive {
                    e1 > 0.0 || e2 > 0.0 || e3 > 0.0
                } else {
                    e1 < 0.0 || e2 < 0.0 || e3 < 0.0
                }
            };
            let skip = if surf.flags & SURFACE_FLAG_X_PROJECTION != 0 {
                let [w1, w2, w3] = [
                    -i32::from(surf.vertex1[2]),
                    -i32::from(surf.vertex2[2]),
                    -i32::from(surf.vertex3[2]),
                ]
                .map(|w| w as f32);
                // The original writes (w - -pz); identical to w + pz in IEEE f32.
                outside(w1, w2, w3, -pz, surf.normal[0] > 0.0)
            } else {
                let [w1, w2, w3] =
                    [surf.vertex1[0], surf.vertex2[0], surf.vertex3[0]].map(f32::from);
                outside(w1, w2, w3, px, surf.normal[2] > 0.0)
            };
            if skip {
                continue;
            }
            if flags.checking_for_camera {
                if surf.flags & SURFACE_FLAG_NO_CAM_COLLISION != 0 {
                    continue;
                }
            } else {
                if surf.surface_type == SURFACE_CAMERA_BOUNDARY {
                    continue;
                }
                if surf.surface_type == SURFACE_VANISH_CAP_WALLS && pass_vanish_walls {
                    continue;
                }
            }
            data.x += surf.normal[0] * (radius - offset);
            data.z += surf.normal[2] * (radius - offset);
            if data.num_walls < 4 {
                data.walls[data.num_walls as usize] = Some(index);
                data.num_walls += 1;
            }
            collisions += 1;
        }
        collisions
    }

    /// find_wall_collisions. `pass_vanish_walls` is true when the current object
    /// can move through grates, or is Mario wearing the vanish cap.
    pub fn find_wall_collisions(
        &self,
        data: &mut WallCollisionData,
        flags: CollisionFlags,
        pass_vanish_walls: bool,
    ) -> i32 {
        let x = f32_to_s16(data.x);
        let z = f32_to_s16(data.z);
        data.num_walls = 0;
        if Self::out_of_bounds(x) || Self::out_of_bounds(z) {
            return 0;
        }
        let cell = Self::cell_of(x, z);
        let mut collisions = 0;
        collisions += self.find_wall_collisions_from_list(
            &self.dynamic_cells[cell][WALLS],
            data,
            flags,
            pass_vanish_walls,
        );
        collisions += self.find_wall_collisions_from_list(
            &self.static_cells[cell][WALLS],
            data,
            flags,
            pass_vanish_walls,
        );
        collisions
    }

    /// f32_find_wall_collision: updates the position in place.
    pub fn f32_find_wall_collision(
        &self,
        position: &mut [f32; 3],
        offset_y: f32,
        radius: f32,
        flags: CollisionFlags,
        pass_vanish_walls: bool,
    ) -> i32 {
        let mut data = WallCollisionData::new(*position, offset_y, radius);
        let collisions = self.find_wall_collisions(&mut data, flags, pass_vanish_walls);
        *position = [data.x, data.y, data.z];
        collisions
    }

    fn lateral(surf: &Surface, x: i32, z: i32) -> [i32; 3] {
        let [x1, z1] = [i32::from(surf.vertex1[0]), i32::from(surf.vertex1[2])];
        let [x2, z2] = [i32::from(surf.vertex2[0]), i32::from(surf.vertex2[2])];
        let [x3, z3] = [i32::from(surf.vertex3[0]), i32::from(surf.vertex3[2])];
        let edge = |za: i32, xb: i32, xa: i32, zb: i32| {
            (za - z)
                .wrapping_mul(xb - xa)
                .wrapping_sub((xa - x).wrapping_mul(zb - za))
        };
        [
            edge(z1, x2, x1, z2),
            edge(z2, x3, x2, z3),
            edge(z3, x1, x3, z1),
        ]
    }

    fn excluded(surf: &Surface, flags: CollisionFlags) -> bool {
        if flags.checking_for_camera {
            surf.flags & SURFACE_FLAG_NO_CAM_COLLISION != 0
        } else {
            surf.surface_type == SURFACE_CAMERA_BOUNDARY
        }
    }

    fn plane_height(surf: &Surface, x: i32, z: i32) -> f32 {
        let [nx, ny, nz] = surf.normal;
        -((x as f32) * nx + nz * (z as f32) + surf.origin_offset) / ny
    }

    fn find_ceil_from_list(
        &self,
        list: &[SurfaceIndex],
        x: i32,
        y: i32,
        z: i32,
        height: &mut f32,
        flags: CollisionFlags,
    ) -> Option<SurfaceIndex> {
        for &index in list {
            let surf = self.surface(index);
            // The original stops at the first failing edge; results are identical.
            if Self::lateral(surf, x, z).iter().any(|&e| e > 0) {
                continue;
            }
            if Self::excluded(surf, flags) || surf.normal[1] == 0.0 {
                continue;
            }
            let h = Self::plane_height(surf, x, z);
            if y as f32 - (h - -78.0) > 0.0 {
                continue;
            }
            *height = h;
            return Some(index);
        }
        None
    }

    /// find_ceil: first matching ceiling in partition order, dynamic preferred
    /// only when strictly lower.
    pub fn find_ceil(
        &self,
        x: f32,
        y: f32,
        z: f32,
        flags: CollisionFlags,
    ) -> (f32, Option<SurfaceIndex>) {
        let mut height = CELL_HEIGHT_LIMIT;
        let mut dynamic_height = CELL_HEIGHT_LIMIT;
        let (x, y, z) = (f32_to_s16(x), f32_to_s16(y), f32_to_s16(z));
        if Self::out_of_bounds(x) || Self::out_of_bounds(z) {
            return (height, None);
        }
        let cell = Self::cell_of(x, z);
        let (x, y, z) = (i32::from(x), i32::from(y), i32::from(z));
        let dynamic = self.find_ceil_from_list(
            &self.dynamic_cells[cell][CEILS],
            x,
            y,
            z,
            &mut dynamic_height,
            flags,
        );
        let mut ceil =
            self.find_ceil_from_list(&self.static_cells[cell][CEILS], x, y, z, &mut height, flags);
        if dynamic_height < height {
            ceil = dynamic;
            height = dynamic_height;
        }
        (height, ceil)
    }

    fn find_floor_from_list(
        &self,
        list: &[SurfaceIndex],
        x: i32,
        y: i32,
        z: i32,
        height: &mut f32,
        flags: CollisionFlags,
    ) -> Option<SurfaceIndex> {
        for &index in list {
            let surf = self.surface(index);
            if Self::lateral(surf, x, z).iter().any(|&e| e < 0) {
                continue;
            }
            if Self::excluded(surf, flags) || surf.normal[1] == 0.0 {
                continue;
            }
            let h = Self::plane_height(surf, x, z);
            if y as f32 - (h + -78.0) < 0.0 {
                continue;
            }
            *height = h;
            return Some(index);
        }
        None
    }

    /// find_floor, including the SURFACE_INTANGIBLE retry and the clearing of
    /// `find_floor_include_surface_intangible` after a call that honoured it.
    pub fn find_floor(
        &self,
        x: f32,
        y: f32,
        z: f32,
        flags: &mut CollisionFlags,
    ) -> (f32, Option<SurfaceIndex>) {
        let mut height = FLOOR_LOWER_LIMIT;
        let mut dynamic_height = FLOOR_LOWER_LIMIT;
        let (x, y, z) = (f32_to_s16(x), f32_to_s16(y), f32_to_s16(z));
        if Self::out_of_bounds(x) || Self::out_of_bounds(z) {
            return (height, None);
        }
        let cell = Self::cell_of(x, z);
        let (x, y, z) = (i32::from(x), i32::from(y), i32::from(z));
        let query = *flags;
        let dynamic = self.find_floor_from_list(
            &self.dynamic_cells[cell][FLOORS],
            x,
            y,
            z,
            &mut dynamic_height,
            query,
        );
        let list = &self.static_cells[cell][FLOORS];
        let mut floor = self.find_floor_from_list(list, x, y, z, &mut height, query);
        if !flags.find_floor_include_surface_intangible {
            if floor.is_some_and(|f| self.surface(f).surface_type == SURFACE_INTANGIBLE) {
                // (s32)(height - 200.0f); the miss keeps the intangible height.
                let retry_y = (height - 200.0) as i32;
                floor = self.find_floor_from_list(list, x, retry_y, z, &mut height, query);
            }
        } else {
            flags.find_floor_include_surface_intangible = false;
        }
        if dynamic_height > height {
            floor = dynamic;
            height = dynamic_height;
        }
        (height, floor)
    }

    /// find_water_level: first water box (value < 50) strictly containing x/z.
    pub fn find_water_level(&self, x: f32, z: f32) -> f32 {
        for region in self.environment.iter().flatten() {
            let [val, lo_x, lo_z, hi_x, hi_z, level] = *region;
            let (lo_x, lo_z, hi_x, hi_z) = (
                f32::from(lo_x),
                f32::from(lo_z),
                f32::from(hi_x),
                f32::from(hi_z),
            );
            if lo_x < x && x < hi_x && lo_z < z && z < hi_z && val < 50 {
                return f32::from(level);
            }
        }
        FLOOR_LOWER_LIMIT
    }

    /// find_poison_gas_level: first gas box (value >= 50, multiple of 10).
    pub fn find_poison_gas_level(&self, x: f32, z: f32) -> f32 {
        for region in self.environment.iter().flatten() {
            let [val, lo_x, lo_z, hi_x, hi_z, level] = *region;
            if val >= 50 {
                let (lo_x, lo_z, hi_x, hi_z) = (
                    f32::from(lo_x),
                    f32::from(lo_z),
                    f32::from(hi_x),
                    f32::from(hi_z),
                );
                if lo_x < x && x < hi_x && lo_z < z && z < hi_z && val % 10 == 0 {
                    return f32::from(level);
                }
            }
        }
        FLOOR_LOWER_LIMIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_indices_follow_buffer_and_s16_wraparound() {
        assert_eq!(lower_cell_index(-8192), 0);
        assert_eq!(lower_cell_index(-8192 + 1024 + 49), 0);
        assert_eq!(lower_cell_index(-8192 + 1024 + 50), 1);
        assert_eq!(upper_cell_index(-8192 + 1024 - 51), 0);
        assert_eq!(upper_cell_index(-8192 + 1024 - 49), 1);
        assert_eq!(upper_cell_index(8191), 15);
        // 0x7000 + 0x2000 wraps negative and clamps to cell 0.
        assert_eq!(lower_cell_index(0x7000), 0);
        assert_eq!(upper_cell_index(0x7000), 0);
    }

    #[test]
    fn f32_to_s16_truncates_then_wraps() {
        assert_eq!(f32_to_s16(-1.9), -1);
        assert_eq!(f32_to_s16(1.9), 1);
        assert_eq!(f32_to_s16(32768.0), -32768);
        assert_eq!(f32_to_s16(65536.0 + 5.5), 5);
    }

    #[test]
    fn degenerate_triangles_are_skipped_and_offsets_use_first_vertex() {
        assert!(read_surface_data([0, 0, 0], [1, 0, 0], [2, 0, 0]).is_none());
        let s = read_surface_data([0, 10, 0], [0, 10, 100], [100, 10, 0]).unwrap();
        assert_eq!(s.normal, [0.0, 1.0, 0.0]);
        assert_eq!(s.origin_offset, -10.0);
        assert_eq!((s.lower_y, s.upper_y), (5, 15));
    }
}
