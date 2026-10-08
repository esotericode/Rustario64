//! Mario's physics steps, translated from pinned CC0 n64decomp/sm64
//! src/game/mario_step.c plus the mario.c helpers it calls
//! (mario_set_forward_vel, resolve_and_return_wall_collisions, vec3f_find_ceil,
//! mario_get_terrain_sound_addend). f32 operation order, s16 angle wraparound,
//! and the documented glitches are preserved.
//!
//! Not yet ported: mario_update_quicksand and mario_push_off_steep_floor (they
//! change actions), bully collision helpers, and sound playback (play_sound has
//! no gameplay state).
use super::{MarioState, StepWorld, SurfaceRef, constants::*};
use crate::simulation::collision::{SurfaceIndex, WallCollisionData};

const MOVING_SAND_SPEEDS: [i16; 4] = [12, 8, 4, 0];

/// sTerrainSounds[terrain type][floor sound type], from pinned mario.c.
const TERRAIN_SOUNDS: [[i8; 6]; 7] = [
    [
        SOUND_TERRAIN_DEFAULT,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_GRASS,
        SOUND_TERRAIN_GRASS,
        SOUND_TERRAIN_GRASS,
        SOUND_TERRAIN_DEFAULT,
    ],
    [
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_GRASS,
        SOUND_TERRAIN_GRASS,
    ],
    [
        SOUND_TERRAIN_SNOW,
        SOUND_TERRAIN_ICE,
        SOUND_TERRAIN_SNOW,
        SOUND_TERRAIN_ICE,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_STONE,
    ],
    [
        SOUND_TERRAIN_SAND,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_SAND,
        SOUND_TERRAIN_SAND,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_STONE,
    ],
    [
        SOUND_TERRAIN_SPOOKY,
        SOUND_TERRAIN_SPOOKY,
        SOUND_TERRAIN_SPOOKY,
        SOUND_TERRAIN_SPOOKY,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_STONE,
    ],
    [
        SOUND_TERRAIN_DEFAULT,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_GRASS,
        SOUND_TERRAIN_ICE,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_ICE,
    ],
    [
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_STONE,
        SOUND_TERRAIN_ICE,
        SOUND_TERRAIN_ICE,
    ],
];

fn floor_of(m: &MarioState) -> SurfaceRef {
    // Every caller dereferences m->floor; the original would crash on NULL.
    m.floor
        .expect("Mario has no referenced floor (the original dereferences NULL)")
}

/// mario_set_forward_vel.
pub fn mario_set_forward_vel(m: &mut MarioState, w: &StepWorld<'_>, forward_vel: f32) {
    m.forward_vel = forward_vel;
    m.slide_vel_x = w.trig.sins(i32::from(m.face_angle[1])) * m.forward_vel;
    m.slide_vel_z = w.trig.coss(i32::from(m.face_angle[1])) * m.forward_vel;
    m.vel[0] = m.slide_vel_x;
    m.vel[2] = m.slide_vel_z;
}

/// resolve_and_return_wall_collisions: pushes `pos` out of walls and returns the
/// last referenced wall. Mario is the current object, so the vanish cap decides
/// whether vanish-cap walls are passable.
pub fn resolve_and_return_wall_collisions(
    m: &MarioState,
    w: &StepWorld<'_>,
    pos: &mut [f32; 3],
    offset: f32,
    radius: f32,
) -> Option<SurfaceRef> {
    let mut data = WallCollisionData::new(*pos, offset, radius);
    let pass_vanish_walls = m.flags & MARIO_VANISH_CAP != 0;
    let mut wall = None;
    if w.collision
        .find_wall_collisions(&mut data, w.collision_flags, pass_vanish_walls)
        != 0
    {
        wall = data.walls[data.num_walls as usize - 1].map(SurfaceRef::Collision);
    }
    *pos = [data.x, data.y, data.z];
    wall
}

/// vec3f_find_ceil: searches from 80 units above `height`.
pub fn vec3f_find_ceil(
    w: &StepWorld<'_>,
    pos: [f32; 3],
    height: f32,
) -> (f32, Option<SurfaceIndex>) {
    w.collision
        .find_ceil(pos[0], height + 80.0, pos[2], w.collision_flags)
}

fn find_floor(w: &mut StepWorld<'_>, pos: [f32; 3]) -> (f32, Option<SurfaceRef>) {
    let (height, floor) = w
        .collision
        .find_floor(pos[0], pos[1], pos[2], &mut w.collision_flags);
    (height, floor.map(SurfaceRef::Collision))
}

/// mario_get_terrain_sound_addend.
pub fn mario_get_terrain_sound_addend(m: &MarioState, w: &StepWorld<'_>) -> u32 {
    let terrain_type = usize::from(w.area_terrain_type & TERRAIN_MASK);
    let mut ret = i32::from(SOUND_TERRAIN_DEFAULT) << 16;
    if let Some(floor) = m.floor {
        let floor_type = w.surface(floor).surface_type;
        if w.level_num != LEVEL_LLL && m.floor_height < (i32::from(m.water_level) - 10) as f32 {
            ret = i32::from(SOUND_TERRAIN_WATER) << 16;
        } else if (0x21..0x28).contains(&floor_type) {
            // SURFACE_IS_QUICKSAND
            ret = i32::from(SOUND_TERRAIN_SAND) << 16;
        } else {
            let floor_sound_type = match floor_type {
                SURFACE_NOT_SLIPPERY
                | SURFACE_HARD
                | SURFACE_HARD_NOT_SLIPPERY
                | SURFACE_SWITCH => 1,
                SURFACE_SLIPPERY | SURFACE_HARD_SLIPPERY | SURFACE_NO_CAM_COL_SLIPPERY => 2,
                SURFACE_VERY_SLIPPERY
                | SURFACE_ICE
                | SURFACE_HARD_VERY_SLIPPERY
                | SURFACE_NOISE_VERY_SLIPPERY_73
                | SURFACE_NOISE_VERY_SLIPPERY_74
                | SURFACE_NOISE_VERY_SLIPPERY
                | SURFACE_NO_CAM_COL_VERY_SLIPPERY => 3,
                SURFACE_NOISE_DEFAULT => 4,
                SURFACE_NOISE_SLIPPERY => 5,
                _ => 0,
            };
            let row = TERRAIN_SOUNDS
                .get(terrain_type)
                .expect("terrain type 7 reads past sTerrainSounds in the original");
            ret = i32::from(row[floor_sound_type]) << 16;
        }
    }
    ret as u32
}

/// mario_bonk_reflection (sound effects are presentation-only and omitted).
pub fn mario_bonk_reflection(m: &mut MarioState, w: &StepWorld<'_>, negate_speed: bool) {
    if let Some(wall) = m.wall {
        let normal = w.surface(wall).normal;
        let wall_angle = w.trig.atan2s(normal[2], normal[0]);
        m.face_angle[1] = wall_angle.wrapping_sub(m.face_angle[1].wrapping_sub(wall_angle));
    }
    if negate_speed {
        mario_set_forward_vel(m, w, -m.forward_vel);
    } else {
        m.face_angle[1] = m.face_angle[1].wrapping_add(i16::MIN);
    }
}

/// mario_update_moving_sand.
pub fn mario_update_moving_sand(m: &mut MarioState, w: &StepWorld<'_>) -> bool {
    let floor = w.surface(floor_of(m));
    if matches!(
        floor.surface_type,
        SURFACE_DEEP_MOVING_QUICKSAND
            | SURFACE_SHALLOW_MOVING_QUICKSAND
            | SURFACE_MOVING_QUICKSAND
            | SURFACE_INSTANT_MOVING_QUICKSAND
    ) {
        let push_angle = (i32::from(floor.force) << 8) as i16;
        let push_speed = f32::from(
            *usize::try_from(i32::from(floor.force) >> 8)
                .ok()
                .and_then(|i| MOVING_SAND_SPEEDS.get(i))
                .expect("moving-sand force index outside the original table"),
        );
        m.vel[0] += push_speed * w.trig.sins(i32::from(push_angle));
        m.vel[2] += push_speed * w.trig.coss(i32::from(push_angle));
        return true;
    }
    false
}

/// mario_update_windy_ground.
pub fn mario_update_windy_ground(m: &mut MarioState, w: &StepWorld<'_>) -> bool {
    let floor = w.surface(floor_of(m));
    if floor.surface_type == SURFACE_HORIZONTAL_WIND {
        let push_angle = (i32::from(floor.force) << 8) as i16;
        let mut push_speed;
        if m.action & ACT_FLAG_MOVING != 0 {
            let push_d_yaw = m.face_angle[1].wrapping_sub(push_angle);
            push_speed = if m.forward_vel > 0.0 {
                -m.forward_vel * 0.5
            } else {
                -8.0
            };
            if push_d_yaw > -0x4000 && push_d_yaw < 0x4000 {
                push_speed *= -1.0;
            }
            push_speed *= w.trig.coss(i32::from(push_d_yaw));
        } else {
            push_speed = 3.2 + (w.global_timer % 4) as f32;
        }
        m.vel[0] += push_speed * w.trig.sins(i32::from(push_angle));
        m.vel[2] += push_speed * w.trig.coss(i32::from(push_angle));
        return true;
    }
    false
}

fn sync_gfx(m: &mut MarioState) {
    m.gfx_pos = m.pos;
    m.gfx_angle = [0, m.face_angle[1], 0];
}

/// stop_and_set_height_to_floor.
pub fn stop_and_set_height_to_floor(m: &mut MarioState, w: &StepWorld<'_>) {
    mario_set_forward_vel(m, w, 0.0);
    m.vel[1] = 0.0;
    m.pos[1] = m.floor_height;
    sync_gfx(m);
}

/// stationary_ground_step.
pub fn stationary_ground_step(m: &mut MarioState, w: &mut StepWorld<'_>) -> u32 {
    let mut step_result = GROUND_STEP_NONE;
    mario_set_forward_vel(m, w, 0.0);
    let mut take_step = mario_update_moving_sand(m, w);
    take_step |= mario_update_windy_ground(m, w);
    if take_step {
        step_result = perform_ground_step(m, w);
    } else {
        m.pos[1] = m.floor_height;
        sync_gfx(m);
    }
    step_result
}

fn water_level(w: &StepWorld<'_>, pos: [f32; 3]) -> f32 {
    w.collision.find_water_level(pos[0], pos[2])
}

fn perform_ground_quarter_step(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    next_pos: &mut [f32; 3],
) -> u32 {
    let _lower_wall = resolve_and_return_wall_collisions(m, w, next_pos, 30.0, 24.0);
    let upper_wall = resolve_and_return_wall_collisions(m, w, next_pos, 60.0, 50.0);
    let (mut floor_height, mut floor) = find_floor(w, *next_pos);
    let (ceil_height, _ceil) = vec3f_find_ceil(w, *next_pos, floor_height);
    let water_level = water_level(w, *next_pos);
    m.wall = upper_wall;
    if floor.is_none() {
        return GROUND_STEP_HIT_WALL_STOP_QSTEPS;
    }
    if m.action & ACT_FLAG_RIDING_SHELL != 0 && floor_height < water_level {
        floor_height = water_level;
        floor = Some(SurfaceRef::WaterPseudoFloor);
        w.water_pseudo_floor_origin_offset = floor_height;
    }
    if next_pos[1] > floor_height + 100.0 {
        if next_pos[1] + 160.0 >= ceil_height {
            return GROUND_STEP_HIT_WALL_STOP_QSTEPS;
        }
        m.pos = *next_pos;
        m.floor = floor;
        m.floor_height = floor_height;
        return GROUND_STEP_LEFT_GROUND;
    }
    if floor_height + 160.0 >= ceil_height {
        return GROUND_STEP_HIT_WALL_STOP_QSTEPS;
    }
    m.pos = [next_pos[0], floor_height, next_pos[2]];
    m.floor = floor;
    m.floor_height = floor_height;
    if let Some(wall) = upper_wall {
        let normal = w.surface(wall).normal;
        let wall_d_yaw = w
            .trig
            .atan2s(normal[2], normal[0])
            .wrapping_sub(m.face_angle[1]);
        if (0x2AAA..=0x5555).contains(&wall_d_yaw) {
            return GROUND_STEP_NONE;
        }
        if (-0x5555..=-0x2AAA).contains(&wall_d_yaw) {
            return GROUND_STEP_NONE;
        }
        return GROUND_STEP_HIT_WALL_CONTINUE_QSTEPS;
    }
    GROUND_STEP_NONE
}

/// perform_ground_step: four quarter steps scaled by the floor normal's Y.
pub fn perform_ground_step(m: &mut MarioState, w: &mut StepWorld<'_>) -> u32 {
    let mut step_result = GROUND_STEP_NONE;
    for _ in 0..4 {
        let normal_y = w.surface(floor_of(m)).normal[1];
        let mut intended = [
            m.pos[0] + normal_y * (m.vel[0] / 4.0),
            m.pos[1],
            m.pos[2] + normal_y * (m.vel[2] / 4.0),
        ];
        step_result = perform_ground_quarter_step(m, w, &mut intended);
        if step_result == GROUND_STEP_LEFT_GROUND || step_result == GROUND_STEP_HIT_WALL_STOP_QSTEPS
        {
            break;
        }
    }
    m.terrain_sound_addend = mario_get_terrain_sound_addend(m, w);
    sync_gfx(m);
    if step_result == GROUND_STEP_HIT_WALL_CONTINUE_QSTEPS {
        step_result = GROUND_STEP_HIT_WALL;
    }
    step_result
}

/// check_ledge_grab.
pub fn check_ledge_grab(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    wall: SurfaceRef,
    intended_pos: [f32; 3],
    next_pos: [f32; 3],
) -> bool {
    if m.vel[1] > 0.0 {
        return false;
    }
    let displacement_x = next_pos[0] - intended_pos[0];
    let displacement_z = next_pos[2] - intended_pos[2];
    if displacement_x * m.vel[0] + displacement_z * m.vel[2] > 0.0 {
        return false;
    }
    let wall_normal = w.surface(wall).normal;
    let ledge_x = next_pos[0] - wall_normal[0] * 60.0;
    let ledge_z = next_pos[2] - wall_normal[2] * 60.0;
    let (ledge_y, ledge_floor) = find_floor(w, [ledge_x, next_pos[1] + 160.0, ledge_z]);
    if ledge_y - next_pos[1] <= 100.0 {
        return false;
    }
    let ledge_floor = ledge_floor.expect("ledge floor is NULL (the original dereferences it)");
    m.pos = [ledge_x, ledge_y, ledge_z];
    m.floor = Some(ledge_floor);
    m.floor_height = ledge_y;
    let floor_normal = w.surface(ledge_floor).normal;
    m.floor_angle = w.trig.atan2s(floor_normal[2], floor_normal[0]);
    m.face_angle[0] = 0;
    m.face_angle[1] = w
        .trig
        .atan2s(wall_normal[2], wall_normal[0])
        .wrapping_add(i16::MIN);
    true
}

fn perform_air_quarter_step(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    intended_pos: [f32; 3],
    step_arg: u32,
) -> u32 {
    let mut next_pos = intended_pos;
    let upper_wall = resolve_and_return_wall_collisions(m, w, &mut next_pos, 150.0, 50.0);
    let lower_wall = resolve_and_return_wall_collisions(m, w, &mut next_pos, 30.0, 50.0);
    let (mut floor_height, mut floor) = find_floor(w, next_pos);
    let (ceil_height, _ceil) = vec3f_find_ceil(w, next_pos, floor_height);
    let water_level = water_level(w, next_pos);
    m.wall = None;
    if floor.is_none() {
        if next_pos[1] <= m.floor_height {
            m.pos[1] = m.floor_height;
            return AIR_STEP_LANDED;
        }
        m.pos[1] = next_pos[1];
        return AIR_STEP_HIT_WALL;
    }
    if m.action & ACT_FLAG_RIDING_SHELL != 0 && floor_height < water_level {
        floor_height = water_level;
        floor = Some(SurfaceRef::WaterPseudoFloor);
        w.water_pseudo_floor_origin_offset = floor_height;
    }
    if next_pos[1] <= floor_height {
        if ceil_height - floor_height > 160.0 {
            m.pos[0] = next_pos[0];
            m.pos[2] = next_pos[2];
            m.floor = floor;
            m.floor_height = floor_height;
        }
        m.pos[1] = floor_height;
        return AIR_STEP_LANDED;
    }
    if next_pos[1] + 160.0 > ceil_height {
        if m.vel[1] >= 0.0 {
            m.vel[1] = 0.0;
            // Uses the referenced ceiling, not the one just found.
            if step_arg & AIR_STEP_CHECK_HANG != 0
                && m.ceil
                    .is_some_and(|c| w.surface(c).surface_type == SURFACE_HANGABLE)
            {
                return AIR_STEP_GRABBED_CEILING;
            }
            return AIR_STEP_NONE;
        }
        if next_pos[1] <= m.floor_height {
            m.pos[1] = m.floor_height;
            return AIR_STEP_LANDED;
        }
        m.pos[1] = next_pos[1];
        return AIR_STEP_HIT_WALL;
    }
    if step_arg & AIR_STEP_CHECK_LEDGE_GRAB != 0
        && upper_wall.is_none()
        && let Some(lower) = lower_wall
    {
        if check_ledge_grab(m, w, lower, intended_pos, next_pos) {
            return AIR_STEP_GRABBED_LEDGE;
        }
        m.pos = next_pos;
        m.floor = floor;
        m.floor_height = floor_height;
        return AIR_STEP_NONE;
    }
    m.pos = next_pos;
    m.floor = floor;
    m.floor_height = floor_height;
    if let Some(wall) = upper_wall.or(lower_wall) {
        m.wall = Some(wall);
        let surface = w.surface(wall);
        let wall_d_yaw = w
            .trig
            .atan2s(surface.normal[2], surface.normal[0])
            .wrapping_sub(m.face_angle[1]);
        if surface.surface_type == SURFACE_BURNING {
            return AIR_STEP_HIT_LAVA_WALL;
        }
        if !(-0x6000..=0x6000).contains(&wall_d_yaw) {
            m.flags |= MARIO_UNKNOWN_30;
            return AIR_STEP_HIT_WALL;
        }
    }
    AIR_STEP_NONE
}

/// apply_twirl_gravity.
pub fn apply_twirl_gravity(m: &mut MarioState) {
    let mut heaviness = 1.0f32;
    if m.angle_vel[1] > 1024 {
        heaviness = 1024.0 / f32::from(m.angle_vel[1]);
    }
    let terminal_velocity = -75.0 * heaviness;
    m.vel[1] -= 4.0 * heaviness;
    if m.vel[1] < terminal_velocity {
        m.vel[1] = terminal_velocity;
    }
}

/// should_strengthen_gravity_for_jump_ascent.
pub fn should_strengthen_gravity_for_jump_ascent(m: &MarioState) -> bool {
    if m.flags & MARIO_UNKNOWN_08 == 0 {
        return false;
    }
    if m.action & (ACT_FLAG_INTANGIBLE | ACT_FLAG_INVULNERABLE) != 0 {
        return false;
    }
    if m.input & INPUT_A_DOWN == 0 && m.vel[1] > 20.0 {
        return m.action & ACT_FLAG_CONTROL_JUMP_HEIGHT != 0;
    }
    false
}

fn fall(m: &mut MarioState, by: f32, limit: f32) {
    m.vel[1] -= by;
    if m.vel[1] < limit {
        m.vel[1] = limit;
    }
}

/// apply_gravity.
pub fn apply_gravity(m: &mut MarioState) {
    if m.action == ACT_TWIRLING && m.vel[1] < 0.0 {
        apply_twirl_gravity(m);
    } else if m.action == ACT_SHOT_FROM_CANNON {
        fall(m, 1.0, -75.0);
    } else if m.action == ACT_LONG_JUMP
        || m.action == ACT_SLIDE_KICK
        || m.action == ACT_BBH_ENTER_SPIN
    {
        fall(m, 2.0, -75.0);
    } else if m.action == ACT_LAVA_BOOST || m.action == ACT_FALL_AFTER_STAR_GRAB {
        fall(m, 3.2, -65.0);
    } else if m.action == ACT_GETTING_BLOWN {
        let gravity = m.getting_blown_gravity;
        fall(m, gravity, -75.0);
    } else if should_strengthen_gravity_for_jump_ascent(m) {
        m.vel[1] /= 4.0;
    } else if m.action & ACT_FLAG_METAL_WATER != 0 {
        fall(m, 1.6, -16.0);
    } else if m.flags & MARIO_WING_CAP != 0 && m.vel[1] < 0.0 && m.input & INPUT_A_DOWN != 0 {
        m.wing_flutter = true;
        m.vel[1] -= 2.0;
        if m.vel[1] < -37.5 {
            m.vel[1] += 4.0;
            if m.vel[1] > -37.5 {
                m.vel[1] = -37.5;
            }
        }
    } else {
        fall(m, 4.0, -75.0);
    }
}

/// apply_vertical_wind.
pub fn apply_vertical_wind(m: &mut MarioState, w: &StepWorld<'_>) {
    if m.action != ACT_GROUND_POUND {
        let offset_y = m.pos[1] - -1500.0;
        let floor_type = w.surface(floor_of(m)).surface_type;
        if floor_type == SURFACE_VERTICAL_WIND && -3000.0 < offset_y && offset_y < 2000.0 {
            let max_vel_y = if offset_y >= 0.0 {
                10000.0 / (offset_y + 200.0)
            } else {
                50.0
            };
            if m.vel[1] < max_vel_y {
                m.vel[1] += max_vel_y / 8.0;
                if m.vel[1] > max_vel_y {
                    m.vel[1] = max_vel_y;
                }
            }
        }
    }
}

/// perform_air_step.
pub fn perform_air_step(m: &mut MarioState, w: &mut StepWorld<'_>, step_arg: u32) -> u32 {
    let mut step_result = AIR_STEP_NONE;
    m.wall = None;
    for _ in 0..4 {
        let intended = [
            m.pos[0] + m.vel[0] / 4.0,
            m.pos[1] + m.vel[1] / 4.0,
            m.pos[2] + m.vel[2] / 4.0,
        ];
        let quarter = perform_air_quarter_step(m, w, intended, step_arg);
        if quarter != AIR_STEP_NONE {
            step_result = quarter;
        }
        if quarter == AIR_STEP_LANDED
            || quarter == AIR_STEP_GRABBED_LEDGE
            || quarter == AIR_STEP_GRABBED_CEILING
            || quarter == AIR_STEP_HIT_LAVA_WALL
        {
            break;
        }
    }
    if m.vel[1] >= 0.0 {
        m.peak_height = m.pos[1];
    }
    m.terrain_sound_addend = mario_get_terrain_sound_addend(m, w);
    if m.action != ACT_FLYING {
        apply_gravity(m);
    }
    apply_vertical_wind(m, w);
    sync_gfx(m);
    step_result
}

/// set_vel_from_pitch_and_yaw.
pub fn set_vel_from_pitch_and_yaw(m: &mut MarioState, w: &StepWorld<'_>) {
    let (pitch, yaw) = (i32::from(m.face_angle[0]), i32::from(m.face_angle[1]));
    m.vel[0] = m.forward_vel * w.trig.coss(pitch) * w.trig.sins(yaw);
    m.vel[1] = m.forward_vel * w.trig.sins(pitch);
    m.vel[2] = m.forward_vel * w.trig.coss(pitch) * w.trig.coss(yaw);
}

/// set_vel_from_yaw.
pub fn set_vel_from_yaw(m: &mut MarioState, w: &StepWorld<'_>) {
    let yaw = i32::from(m.face_angle[1]);
    m.slide_vel_x = m.forward_vel * w.trig.sins(yaw);
    m.vel[0] = m.slide_vel_x;
    m.vel[1] = 0.0;
    m.slide_vel_z = m.forward_vel * w.trig.coss(yaw);
    m.vel[2] = m.slide_vel_z;
}
