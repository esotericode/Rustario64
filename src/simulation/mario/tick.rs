//! One frame of the original game loop as it affects the objects, translated
//! from pinned CC0 code: read_controller_inputs (game_init.c),
//! area_update_objects and update_objects (level_update.c,
//! object_list_processor.c, `object::processor`), bhvMario's native
//! bhv_mario_update and copy_mario_state_to_object, update_mario_platform
//! (platform_displacement.c), the level entry (level_script.c's object
//! placements, area.c's load_mario_area, level_update.c's init_level), and
//! the authoritative part of the render pass (geo_process_object and
//! geo_set_animation_globals in rendering_graph_node.c; other objects'
//! writes are in `object::render`).
//!
//! update_hud_values (`simulation::hud`) runs after the objects. Outside the
//! tick: the camera update (`simulation::game` links it), the HUD's drawing,
//! warps, particle objects (bhv_mario_update's spawn_particle) and the
//! render pass's presentation-only writes (matrices, torso and head angles,
//! the hand-scale counter).
use super::{
    MarioObject, MarioState, ObjectId, SaveInputs, StepWorld, SurfaceRef, ThrowMatrix,
    animation::update_animation_frame,
    constants::*,
    core::{
        SpawnPoint, execute_mario_action, init_mario, init_mario_from_save_file, set_mario_action,
    },
};
use crate::{
    content::{
        ImportedLevel,
        animation::{MarioAnimations, ObjectAnimations},
    },
    simulation::{
        TickInput,
        collision::CollisionWorld,
        math::TrigTables,
        object::{
            processor,
            render::ObjectModels,
            script::{Behavior, BehaviorScripts},
            spawn::{self, AreaObjects, SpawnInfo},
        },
        rng::Rng,
    },
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

/// What a level's objects come from: the behavior segment, the loaded
/// models and object animations, and the entered area's placements.
#[derive(Debug, Clone)]
pub struct LevelObjects<'a> {
    pub scripts: &'a BehaviorScripts,
    pub models: &'a ObjectModels,
    pub animations: &'a ObjectAnimations,
    pub area: AreaObjects,
}

impl<'a> LevelObjects<'a> {
    /// A level whose area places no objects: Mario alone.
    pub fn mario_only(
        scripts: &'a BehaviorScripts,
        models: &'a ObjectModels,
        animations: &'a ObjectAnimations,
    ) -> Self {
        Self {
            scripts,
            models,
            animations,
            area: AreaObjects::default(),
        }
    }
}

/// Enter a level: init_mario_from_save_file (file select), the area's load
/// (its macro objects and spawn infos, then Mario's object from
/// gMarioSpawnInfo), init_mario, then the idle action.
pub fn enter_level<'a>(
    collision: &'a CollisionWorld,
    trig: &'a TrigTables,
    anims: &'a MarioAnimations,
    objects: &LevelObjects<'a>,
    entry: &LevelEntry,
) -> (MarioState, StepWorld<'a>) {
    enter_level_with(
        collision,
        trig,
        anims,
        objects,
        Rng::default(),
        entry,
        |_, _| {},
    )
}

/// `enter_level` with gRandomSeed16 at `rng`, running `after_init_mario`
/// where init_level runs reset_camera: after init_mario and before the idle
/// action.
pub fn enter_level_with<'a>(
    collision: &'a CollisionWorld,
    trig: &'a TrigTables,
    anims: &'a MarioAnimations,
    objects: &LevelObjects<'a>,
    rng: Rng,
    entry: &LevelEntry,
    after_init_mario: impl FnOnce(&mut MarioState, &mut StepWorld<'a>),
) -> (MarioState, StepWorld<'a>) {
    let mut w = StepWorld::new(collision, trig, anims);
    w.rng = rng;
    w.level_num = entry.level_num;
    w.course_num = course_of_level(entry.level_num);
    w.area_terrain_type = entry.terrain_type;
    w.camera.mode = entry.camera_mode;
    w.camera.def_mode = entry.camera_def_mode;
    w.save = entry.save;
    w.behaviors = objects.scripts;
    w.models = objects.models;
    w.object_anims = objects.animations;
    w.area = objects.area.clone();
    // load_mario_area renders the spawn's area.
    w.area_index = entry.spawn.area_index;
    w.area.area_index = entry.spawn.area_index;
    let mut m = MarioState::default();
    init_mario_from_save_file(&mut m, &mut w);
    // init_level, with no credits entry.
    w.hud.flags = HUD_DISPLAY_DEFAULT as i16;
    // INIT_LEVEL's clear_objects left the pool empty; load_area spawns the
    // area's macro objects (load_area_terrain) and spawn infos, then
    // load_mario_area spawns Mario.
    spawn::spawn_macro_objects(&mut m, &mut w);
    spawn::spawn_area_objects(&mut m, &mut w);
    let info = mario_spawn_info(&entry.spawn, w.behaviors);
    spawn::spawn_mario(&mut m, &mut w, &info);
    init_mario(&mut m, &mut w, entry.spawn);
    after_init_mario(&mut m, &mut w);
    set_mario_action(&mut m, &mut w, ACT_IDLE, 0);
    (m, w)
}

/// gCurrCourseNum for a level (gLevelToCourseNumTable in the pinned
/// levels/level_defines.h); only the levels the port enters are listed.
pub fn course_of_level(level_num: i16) -> i16 {
    match level_num {
        LEVEL_BOB => COURSE_BOB,
        _ => COURSE_NONE,
    }
}

/// gMarioSpawnInfo as the level script's MARIO and MARIO_POS commands set it.
pub fn mario_spawn_info(spawn: &SpawnPoint, scripts: &BehaviorScripts) -> SpawnInfo {
    SpawnInfo {
        source: 0,
        start_pos: spawn.start_pos,
        start_angle: spawn.start_angle,
        area_index: spawn.area_index,
        active_area_index: spawn.active_area_index,
        behavior_arg: spawn.behavior_arg,
        behavior_script: scripts.address(Behavior::Mario),
        model: MODEL_MARIO as u8,
    }
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

/// update_objects (`object::processor`).
pub fn update_objects(m: &mut MarioState, w: &mut StepWorld<'_>) {
    processor::update_objects(m, w);
}

/// How the render pass placed Mario's object this frame. Presentation only:
/// nothing in the simulation reads it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderedFrame {
    /// The object was processed: active and in the rendered area.
    pub processed: bool,
    /// The floor-alignment matrix (`StepWorld::floor_align_matrix` index)
    /// that replaced the object's position and angles, which the pass clears.
    pub throw_matrix: Option<usize>,
    /// The object the pass drew in his hand (`mario::render`).
    pub held: Option<ObjectId>,
}

/// What the render pass changes in Mario's object: geo_process_node_and_siblings
/// and geo_process_object. Advancing the animation is authoritative (actions
/// read the frame); the matrices and camera-relative position are not.
pub fn render_mario_object(obj: &mut MarioObject, w: &StepWorld<'_>) -> RenderedFrame {
    let throw_matrix = match obj.gfx.throw_matrix {
        Some(ThrowMatrix::FloorAlign(index)) => Some(index),
        Some(ThrowMatrix::Terrain(_)) => {
            panic!("Mario's throw matrix is always a floor-align matrix")
        }
        None => None,
    };
    if obj.gfx.node_flags & GRAPH_RENDER_ACTIVE == 0 {
        obj.gfx.throw_matrix = None;
        return RenderedFrame::default();
    }
    if obj.gfx.area_index == w.area_index {
        update_animation_frame(obj, w);
        obj.gfx.throw_matrix = None;
        return RenderedFrame {
            processed: true,
            throw_matrix,
            held: None,
        };
    }
    RenderedFrame::default()
}

/// One frame: the controller read, the area update, and the render pass.
/// `input.camera_yaw` is the camera yaw Mario reads this frame (the camera
/// computed it last frame).
pub fn tick(m: &mut MarioState, w: &mut StepWorld<'_>, input: TickInput) -> RenderedFrame {
    w.controller.sample(input);
    w.camera.yaw = input.camera_yaw;
    // area_update_objects.
    w.area_update_counter = w.area_update_counter.wrapping_add(1);
    update_objects(m, w);
    crate::simulation::hud::update_hud_values(m, w);
    // render_game, then display_and_vsync. Other objects' render-pass writes
    // need the camera's view, which this camera-less frame does not have.
    assert!(
        processor::all_objects(w).len() == 1,
        "objects other than Mario need the linked camera's render pass"
    );
    let rendered = render_mario_object(&mut m.obj, w);
    w.global_timer = w.global_timer.wrapping_add(1);
    rendered
}
