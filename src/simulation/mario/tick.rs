//! One frame of the original game loop as it affects Mario, with Mario's
//! object as the only object. Translated from pinned CC0 code:
//! read_controller_inputs (game_init.c), area_update_objects and
//! update_objects (level_update.c, object_list_processor.c), clear_object_collision
//! (object_collision.c), cur_obj_update (behavior_script.c) running bhvMario
//! (behavior_data.c) and bhv_mario_update, update_mario_platform
//! (platform_displacement.c), and the authoritative part of the render pass
//! (geo_process_object and geo_set_animation_globals in rendering_graph_node.c).
//!
//! Outside the tick: the camera update (its yaw is a recorded input), the
//! HUD, warps, particle objects (bhv_mario_update's spawn_particle) and the
//! render pass's presentation-only writes (matrices, torso and head angles,
//! the hand-scale counter).
use super::{
    AnimInfo, MarioObject, MarioState, ObjectFields, SaveInputs, StepWorld, SurfaceRef,
    animation::update_animation_frame,
    constants::*,
    core::{
        SpawnPoint, execute_mario_action, init_mario, init_mario_from_save_file, set_mario_action,
    },
};
use crate::{
    content::{ImportedLevel, animation::MarioAnimations},
    simulation::{TickInput, collision::CollisionWorld, math::TrigTables},
};

/// A level entry as init_level performs it without a warp destination (the
/// branch the level select and demos take), from a fresh boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelEntry {
    /// gMarioSpawnInfo after the level script's MARIO and MARIO_POS commands.
    pub spawn: SpawnPoint,
    pub level_num: i16,
    /// The area's terrain type (m->area->terrainType).
    pub terrain_type: u16,
    /// The area camera's mode and default mode, until the camera is ported.
    pub camera_mode: u8,
    pub camera_def_mode: u8,
    pub save: SaveInputs,
}

impl LevelEntry {
    /// Entering `level_num` from a fresh boot with an empty save file:
    /// gMarioSpawnInfo after the level script's MARIO and MARIO_POS commands
    /// (Mario's behavior parameter is 1; `yaw` in degrees, as in the script),
    /// the area's terrain type, and the area camera's mode, which
    /// create_camera copies from the area's GEO_CAMERA node into both the mode
    /// and the default mode.
    pub fn from_level_script(
        level_num: i16,
        area: u8,
        yaw: i16,
        pos: [i16; 3],
        terrain_type: u16,
        camera_mode: u8,
    ) -> Self {
        Self {
            spawn: SpawnPoint::from_level_script(1, area, yaw, pos),
            level_num,
            terrain_type,
            camera_mode,
            camera_def_mode: camera_mode,
            save: SaveInputs::default(),
        }
    }

    /// An imported level's script start (see `from_level_script`), with the
    /// start area's camera mode from its imported GEO_CAMERA node.
    pub fn script_start(level: &ImportedLevel, camera_mode: i16) -> Result<Self, &'static str> {
        let (area, yaw, pos) = level
            .mario_start
            .ok_or("the level script has no Mario start")?;
        let terrain_type = level
            .areas
            .iter()
            .find(|a| a.id == area)
            .ok_or("Mario's start area was not imported")?
            .terrain_type;
        let camera_mode = u8::try_from(camera_mode).map_err(|_| "camera mode out of range")?;
        Ok(Self::from_level_script(
            i16::from(level.level.0),
            area.0,
            yaw,
            pos,
            terrain_type,
            camera_mode,
        ))
    }
}

/// Enter a level: init_mario_from_save_file (file select), Mario's object
/// spawned from the spawn info in its area, init_mario, then the idle action.
pub fn enter_level<'a>(
    collision: &'a CollisionWorld,
    trig: &'a TrigTables,
    anims: &'a MarioAnimations,
    entry: &LevelEntry,
) -> (MarioState, StepWorld<'a>) {
    let mut w = StepWorld::new(collision, trig, anims);
    w.level_num = entry.level_num;
    w.area_terrain_type = entry.terrain_type;
    w.camera.mode = entry.camera_mode;
    w.camera.def_mode = entry.camera_def_mode;
    w.save = entry.save;
    // load_mario_area renders the spawn's area.
    w.area_index = entry.spawn.area_index;
    let mut m = MarioState::default();
    init_mario_from_save_file(&mut m, &w);
    m.obj = spawn_mario_object(&entry.spawn, w.level_num);
    init_mario(&mut m, &mut w, entry.spawn);
    set_mario_action(&mut m, &mut w, ACT_IDLE, 0);
    (m, w)
}

/// Object flags whose cur_obj_update handling needs other objects or object
/// transforms. bhvMario sets none of them.
const UNSUPPORTED_OBJ_FLAGS: u32 = OBJ_FLAG_UPDATE_GFX_POS_AND_ANGLE
    | OBJ_FLAG_MOVE_XZ_USING_FVEL
    | OBJ_FLAG_MOVE_Y_WITH_TERMINAL_VEL
    | OBJ_FLAG_SET_FACE_YAW_TO_MOVE_YAW
    | OBJ_FLAG_SET_FACE_ANGLE_TO_MOVE_ANGLE
    | OBJ_FLAG_COMPUTE_DIST_TO_MARIO
    | OBJ_FLAG_TRANSFORM_RELATIVE_TO_PARENT
    | OBJ_FLAG_SET_THROW_MATRIX_FROM_TRANSFORM
    | OBJ_FLAG_COMPUTE_ANGLE_TO_MARIO;

/// Mario's object as spawn_objects_from_info creates it from gMarioSpawnInfo:
/// the pool slot as clear_objects left it (geo_reset_object_node), then
/// allocate_object, the spawn parameters and geo_obj_init_spawninfo.
pub fn spawn_mario_object(spawn: &SpawnPoint, level_num: i16) -> MarioObject {
    let mut obj = MarioObject::default();
    // geo_reset_object_node: init_graph_node_object, then inactive.
    obj.gfx.node_flags = GRAPH_RENDER_HAS_ANIMATION;
    obj.gfx.scale = [1.0; 3];
    obj.gfx.anim = AnimInfo {
        anim_accel: 0x10000,
        ..AnimInfo::default()
    };
    // allocate_object.
    obj.active_flags = ACTIVE_FLAG_ACTIVE | ACTIVE_FLAG_UNK8;
    obj.collided_obj_interact_types = 0;
    obj.num_collided_objs = 0;
    obj.raw = ObjectFields::default();
    obj.hitbox_radius = 50.0;
    obj.hitbox_height = 100.0;
    obj.hurtbox_radius = 0.0;
    obj.hurtbox_height = 0.0;
    obj.hitbox_down_offset = 0.0;
    obj.platform = None;
    obj.raw.set_s32(O_INTANGIBLE_TIMER, -1);
    obj.raw.set_s32(O_DAMAGE_OR_COIN_VALUE, 0);
    obj.raw.set_s32(O_HEALTH, 2048);
    obj.raw.set_f32(O_COLLISION_DISTANCE, 1000.0);
    obj.raw.set_f32(
        O_DRAWING_DISTANCE,
        if level_num == LEVEL_TTC {
            2000.0
        } else {
            4000.0
        },
    );
    obj.raw.set_f32(O_DISTANCE_TO_MARIO, 19000.0);
    obj.raw.set_s32(O_ROOM, -1);
    obj.gfx.node_flags &= !GRAPH_RENDER_INVISIBLE;
    obj.gfx.pos = [-10000.0; 3];
    obj.gfx.throw_matrix = None;
    // spawn_objects_from_info.
    obj.raw.set_u32(O_BHV_PARAMS, spawn.behavior_arg);
    obj.raw.set_s32(
        O_BHV_PARAMS2ND_BYTE,
        ((spawn.behavior_arg >> 16) & 0xFF) as i32,
    );
    // geo_obj_init_spawninfo.
    obj.gfx.scale = [1.0; 3];
    obj.gfx.angle = spawn.start_angle;
    obj.gfx.pos = spawn.start_pos.map(f32::from);
    obj.gfx.area_index = spawn.area_index;
    obj.gfx.active_area_index = spawn.active_area_index;
    obj.gfx.throw_matrix = None;
    obj.gfx.anim.cur_anim = None;
    obj.gfx.node_flags |= GRAPH_RENDER_ACTIVE;
    obj.gfx.node_flags &= !GRAPH_RENDER_INVISIBLE;
    obj.gfx.node_flags |= GRAPH_RENDER_HAS_ANIMATION;
    obj.gfx.node_flags &= !GRAPH_RENDER_BILLBOARD;
    for (axis, field) in [O_POS_X, O_POS_Y, O_POS_Z].into_iter().enumerate() {
        obj.raw.set_f32(field, f32::from(spawn.start_pos[axis]));
    }
    for (axis, (face, moving)) in [
        (O_FACE_ANGLE_PITCH, O_MOVE_ANGLE_PITCH),
        (O_FACE_ANGLE_YAW, O_MOVE_ANGLE_YAW),
        (O_FACE_ANGLE_ROLL, O_MOVE_ANGLE_ROLL),
    ]
    .into_iter()
    .enumerate()
    {
        obj.raw.set_s32(face, i32::from(spawn.start_angle[axis]));
        obj.raw.set_s32(moving, i32::from(spawn.start_angle[axis]));
    }
    obj
}

/// bhvMario's commands before BEGIN_LOOP, which run on the first update.
fn bhv_mario_begin(obj: &mut MarioObject) {
    obj.raw.set_s32(O_INTANGIBLE_TIMER, 0);
    obj.raw
        .set_u32(O_FLAGS, obj.raw.u32(O_FLAGS) | OBJ_FLAG_0100);
    obj.raw.set_u32(O_UNK94, obj.raw.u32(O_UNK94) | 0x0001);
    obj.hitbox_radius = 37.0;
    obj.hitbox_height = 160.0;
}

/// copy_mario_state_to_object.
pub fn copy_mario_state_to_object(m: &mut MarioState) {
    let raw = &mut m.obj.raw;
    for (axis, field) in [O_VEL_X, O_VEL_Y, O_VEL_Z].into_iter().enumerate() {
        raw.set_f32(field, m.vel[axis]);
    }
    for (axis, field) in [O_POS_X, O_POS_Y, O_POS_Z].into_iter().enumerate() {
        raw.set_f32(field, m.pos[axis]);
    }
    let angle = m.obj.gfx.angle;
    for (axis, (moving, face)) in [
        (O_MOVE_ANGLE_PITCH, O_FACE_ANGLE_PITCH),
        (O_MOVE_ANGLE_YAW, O_FACE_ANGLE_YAW),
        (O_MOVE_ANGLE_ROLL, O_FACE_ANGLE_ROLL),
    ]
    .into_iter()
    .enumerate()
    {
        raw.set_s32(moving, i32::from(angle[axis]));
        raw.set_s32(face, i32::from(angle[axis]));
    }
    for (axis, field) in [O_ANGLE_VEL_PITCH, O_ANGLE_VEL_YAW, O_ANGLE_VEL_ROLL]
        .into_iter()
        .enumerate()
    {
        raw.set_s32(field, i32::from(m.angle_vel[axis]));
    }
}

/// bhv_mario_update. Particle objects are outside the simulation: the flags
/// stay in oMarioParticleFlags for the presentation's particle system.
pub fn bhv_mario_update(m: &mut MarioState, w: &mut StepWorld<'_>) {
    let particle_flags = execute_mario_action(m, w);
    m.obj.raw.set_u32(O_MARIO_PARTICLE_FLAGS, particle_flags);
    copy_mario_state_to_object(m);
}

/// The action-timer reset cur_obj_update runs before and after the script.
fn reset_timer_on_action_change(obj: &mut MarioObject) {
    let action = obj.raw.s32(O_ACTION);
    if action != obj.raw.s32(O_PREV_ACTION) {
        obj.raw.set_s32(O_TIMER, 0);
        obj.raw.set_s32(O_SUB_ACTION, 0);
        obj.raw.set_s32(O_PREV_ACTION, action);
    }
}

/// cur_obj_update for Mario's object, running bhvMario. The debug-page
/// natives around bhv_mario_update only act with debug pages enabled.
pub fn cur_obj_update_mario(m: &mut MarioState, w: &mut StepWorld<'_>) {
    let obj_flags = m.obj.raw.u32(O_FLAGS) as i16 as u32;
    assert!(
        obj_flags & UNSUPPORTED_OBJ_FLAGS == 0,
        "Mario's object flags {obj_flags:#X} need object helpers that are not ported"
    );
    reset_timer_on_action_change(&mut m.obj);
    if !m.obj.bhv_loop_entered {
        bhv_mario_begin(&mut m.obj);
        m.obj.bhv_loop_entered = true;
    }
    bhv_mario_update(m, w);
    let timer = m.obj.raw.s32(O_TIMER);
    if timer < 0x3FFF_FFFF {
        m.obj.raw.set_s32(O_TIMER, timer + 1);
    }
    reset_timer_on_action_change(&mut m.obj);
    let obj_flags = m.obj.raw.u32(O_FLAGS) as i16 as u32;
    assert!(
        obj_flags & UNSUPPORTED_OBJ_FLAGS == 0,
        "Mario's object flags {obj_flags:#X} need object helpers that are not ported"
    );
    assert!(
        m.obj.raw.s32(O_ROOM) == -1,
        "room visibility needs the room system, which is not ported"
    );
}

/// absf (math_util.h).
fn absf(x: f32) -> f32 {
    if x >= 0.0 { x } else { -x }
}

/// update_mario_platform.
pub fn update_mario_platform(m: &mut MarioState, w: &mut StepWorld<'_>) {
    let mario_x = m.obj.raw.f32(O_POS_X);
    let mario_y = m.obj.raw.f32(O_POS_Y);
    let mario_z = m.obj.raw.f32(O_POS_Z);
    let (floor_height, floor) =
        w.collision
            .find_floor(mario_x, mario_y, mario_z, &mut w.collision_flags);
    let away_from_floor = absf(mario_y - floor_height) >= 4.0;
    let platform = if away_from_floor {
        None
    } else {
        floor
            .and_then(|f| w.surface(SurfaceRef::Collision(f)).object)
            .map(super::ObjectId)
    };
    w.mario_platform = platform;
    m.obj.platform = platform;
}

/// update_objects with Mario's object as the only object.
pub fn update_objects(m: &mut MarioState, w: &mut StepWorld<'_>) {
    w.collision_flags.checking_for_camera = false;
    // clear_dynamic_surfaces and update_terrain_objects: no surface objects.
    assert!(
        w.mario_platform.is_none(),
        "platform displacement needs objects, which are not simulated yet"
    );
    // detect_object_collisions: clear_object_collision on Mario's list; there
    // is nothing to collide with.
    m.obj.num_collided_objs = 0;
    m.obj.collided_obj_interact_types = 0;
    let intangible = m.obj.raw.s32(O_INTANGIBLE_TIMER);
    if intangible > 0 {
        m.obj.raw.set_s32(O_INTANGIBLE_TIMER, intangible - 1);
    }
    // update_non_terrain_objects: update_objects_starting_at marks the object
    // animated, then updates it.
    m.obj.gfx.node_flags |= GRAPH_RENDER_HAS_ANIMATION;
    cur_obj_update_mario(m, w);
    update_mario_platform(m, w);
}

/// What the render pass changes in Mario's object: geo_process_node_and_siblings
/// and geo_process_object. Advancing the animation is authoritative (actions
/// read the frame); the matrices and camera-relative position are not.
pub fn render_mario_object(obj: &mut MarioObject, w: &StepWorld<'_>) {
    if obj.gfx.node_flags & GRAPH_RENDER_ACTIVE == 0 {
        obj.gfx.throw_matrix = None;
        return;
    }
    if obj.gfx.area_index == w.area_index {
        update_animation_frame(obj, w);
        obj.gfx.throw_matrix = None;
    }
}

/// One frame: the controller read, the area update, and the render pass.
/// `input.camera_yaw` is the camera yaw Mario reads this frame (the camera
/// computed it last frame).
pub fn tick(m: &mut MarioState, w: &mut StepWorld<'_>, input: TickInput) {
    w.controller.sample(input);
    w.camera.yaw = input.camera_yaw;
    // area_update_objects.
    w.area_update_counter = w.area_update_counter.wrapping_add(1);
    update_objects(m, w);
    // render_game, then display_and_vsync.
    render_mario_object(&mut m.obj, w);
    w.global_timer = w.global_timer.wrapping_add(1);
}
