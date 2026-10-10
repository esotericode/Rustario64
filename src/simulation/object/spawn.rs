//! Object creation, translated from pinned CC0 src/game/spawn_object.c
//! (create_object, snap_object_to_floor), object_helpers.c
//! (spawn_object_at_origin, spawn_object, spawn_object_abs_with_rot,
//! spawn_object_relative), graph_node.c (geo_obj_init,
//! geo_obj_init_spawninfo), object_list_processor.c (spawn_objects_from_info)
//! and macro_special_objects.c (spawn_macro_objects, convert_rotation).
//!
//! A level's load spawns only objects whose scripts this port runs
//! ([`super::script::BehaviorScripts::check`]); every other placement is kept in
//! [`AreaObjects::skipped`] with its reason, never silently treated as
//! spawned. The original would also have allocated those objects, so slot
//! assignment and anything the missing objects do (including their random
//! draws) are outside the comparison until they are ported.
use super::{ObjectId, ObjectList, RespawnInfo, helpers, object, script::Unported};
use crate::simulation::mario::{MarioState, StepWorld, constants::*};

/// One macro-object list entry, with its preset resolved from
/// sMacroObjectPresets. `params` is the list's live parameter short:
/// set_object_respawn_info_bits writes the respawn bits into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroEntry {
    /// The entry's segmented address in the area's list.
    pub source: u32,
    /// The list's first short: the biased preset and the yaw bits.
    pub packed: u16,
    pub preset: u16,
    pub behavior: u32,
    pub model: i16,
    pub preset_param: i16,
    pub pos: [i16; 3],
    /// Original signed angle units (convert_rotation of the packed yaw).
    pub yaw: i16,
    pub params: u16,
}

/// An area OBJECT command's spawn info (struct SpawnInfo), in the area's
/// list order (the level script prepends, so script order reversed), with
/// the act filter already applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnInfo {
    pub source: u32,
    pub start_pos: [i16; 3],
    pub start_angle: [i16; 3],
    pub area_index: i8,
    pub active_area_index: i8,
    pub behavior_arg: u32,
    pub behavior_script: u32,
    pub model: u8,
}

/// A placement the port did not spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub source: u32,
    pub behavior: u32,
    pub reason: Unported,
}

/// An area's object placements and the respawn state their objects write.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AreaObjects {
    pub area_index: i8,
    pub macros: Vec<MacroEntry>,
    pub spawn_infos: Vec<SpawnInfo>,
    /// Placements whose scripts are not ported, from the last area load.
    pub skipped: Vec<Skipped>,
}

/// create_object: a slot in the list the script's BEGIN names, pointed at
/// the script. GENACTOR, PUSHABLE and POLELIKE objects snap to the floor
/// under the origin, where a new object still is.
pub fn create_object(w: &mut StepWorld<'_>, script: u32) -> ObjectId {
    let list = ObjectList::from_behavior_word(w.behaviors.word(script))
        .unwrap_or_else(|index| panic!("BEGIN names object list {index}"));
    let id = w.objects.allocate_object(list, w.level_num);
    let obj = w.objects.slot_mut(id);
    obj.cur_bhv_command = script;
    obj.behavior = script;
    if list == ObjectList::Unimportant {
        obj.active_flags |= ACTIVE_FLAG_UNIMPORTANT;
    }
    if matches!(
        list,
        ObjectList::GeneralActor | ObjectList::Pushable | ObjectList::Polelike
    ) {
        // snap_object_to_floor.
        let pos = w.objects.slot(id).pos();
        let height = w.find_floor_height(pos[0], pos[1], pos[2]);
        let obj = w.objects.slot_mut(id);
        obj.raw.set_f32(O_FLOOR_HEIGHT, height);
        let y = obj.raw.f32(O_POS_Y);
        if height + 2.0 > y && y > height - 10.0 {
            obj.raw.set_f32(O_POS_Y, height);
            obj.raw
                .set_u32(O_MOVE_FLAGS, obj.raw.u32(O_MOVE_FLAGS) | OBJ_MOVE_ON_GROUND);
        }
    }
    id
}

/// geo_obj_init at the origin with the model's gLoadedGraphNodes entry.
fn geo_obj_init(gfx: &mut super::GfxState, shared_child: Option<u16>) {
    gfx.scale = [1.0; 3];
    gfx.pos = [0.0; 3];
    gfx.angle = [0; 3];
    gfx.shared_child = shared_child;
    gfx.throw_matrix = None;
    gfx.anim.cur_anim = None;
    gfx.node_flags |= GRAPH_RENDER_ACTIVE;
    gfx.node_flags &= !GRAPH_RENDER_INVISIBLE;
    gfx.node_flags |= GRAPH_RENDER_HAS_ANIMATION;
    gfx.node_flags &= !GRAPH_RENDER_BILLBOARD;
}

/// spawn_object_at_origin: `behavior` must have passed the spawn check.
pub fn spawn_object_at_origin(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    parent: ObjectId,
    model: i32,
    behavior: u32,
) -> ObjectId {
    if let Err(reason) = w.behaviors.check(behavior) {
        panic!("spawning unported behavior 0x{behavior:08X}: {reason:?}");
    }
    let area = object(&w.objects, &m.obj, parent).gfx.area_index;
    let id = create_object(w, behavior);
    let shared_child = w.loaded_model(model);
    let obj = w.objects.slot_mut(id);
    obj.parent = Some(parent);
    obj.gfx.area_index = area;
    obj.gfx.active_area_index = area;
    geo_obj_init(&mut obj.gfx, shared_child);
    id
}

/// spawn_object: at the parent's position and angles.
pub fn spawn_object(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    parent: ObjectId,
    model: i32,
    behavior: u32,
) -> ObjectId {
    let id = spawn_object_at_origin(m, w, parent, model, behavior);
    let source = object(&w.objects, &m.obj, parent).clone();
    helpers::obj_copy_pos_and_angle(w.objects.slot_mut(id), &source);
    id
}

/// spawn_object_abs_with_rot.
#[allow(clippy::too_many_arguments)]
pub fn spawn_object_abs_with_rot(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    parent: ObjectId,
    model: i32,
    behavior: u32,
    pos: [i16; 3],
    angle: [i16; 3],
) -> ObjectId {
    let id = spawn_object_at_origin(m, w, parent, model, behavior);
    let obj = w.objects.slot_mut(id);
    helpers::obj_set_pos(obj, pos[0], pos[1], pos[2]);
    helpers::obj_set_angle(obj, angle[0], angle[1], angle[2]);
    id
}

/// spawn_object_relative: at an offset rotated by the parent's angles,
/// through the new object's transform (obj_build_relative_transform).
pub fn spawn_object_relative(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    behavior_param: i16,
    relative: [i16; 3],
    parent: ObjectId,
    model: i32,
    behavior: u32,
) -> ObjectId {
    let id = spawn_object_at_origin(m, w, parent, model, behavior);
    let source = object(&w.objects, &m.obj, parent).clone();
    let trig = w.trig;
    let obj = w.objects.slot_mut(id);
    helpers::obj_copy_pos_and_angle(obj, &source);
    helpers::obj_set_parent_relative_pos(obj, relative[0], relative[1], relative[2]);
    helpers::obj_build_transform_from_pos_and_angle(
        obj,
        trig,
        O_PARENT_RELATIVE_POS_X,
        O_FACE_ANGLE_PITCH,
    );
    helpers::obj_translate_local(obj, O_POS_X, O_PARENT_RELATIVE_POS_X);
    obj.raw
        .set_s32(O_BHV_PARAMS2ND_BYTE, i32::from(behavior_param));
    obj.raw
        .set_s32(O_BHV_PARAMS, (i32::from(behavior_param) & 0xFF) << 16);
    id
}

/// convert_rotation.
pub fn convert_rotation(in_rotation: i16) -> i16 {
    let mut rotation = (in_rotation & 0xFF) as u16;
    rotation <<= 8;
    match rotation {
        0x3F00 => rotation = 0x4000,
        0x7F00 => rotation = 0x8000,
        0xBF00 => rotation = 0xC000,
        0xFF00 => rotation = 0x0000,
        _ => {}
    }
    rotation as i16
}

/// spawn_macro_objects for the area's list, spawning the entries whose
/// scripts are ported. gMacroObjectDefaultParent takes the area's index.
pub fn spawn_macro_objects(m: &mut MarioState, w: &mut StepWorld<'_>) {
    let area = w.area.area_index;
    w.objects.macro_default_parent.gfx.area_index = area;
    w.objects.macro_default_parent.gfx.active_area_index = area;
    for index in 0..w.area.macros.len() {
        let entry = w.area.macros[index];
        let mut params = entry.params as i16;
        if entry.preset_param != 0 {
            params =
                ((i32::from(params) & 0xFF00) + (i32::from(entry.preset_param) & 0x00FF)) as i16;
        }
        if (i32::from(params) >> 8) & i32::from(RESPAWN_INFO_DONT_RESPAWN)
            == i32::from(RESPAWN_INFO_DONT_RESPAWN)
        {
            continue;
        }
        if let Err(reason) = w.behaviors.check(entry.behavior) {
            w.area.skipped.push(Skipped {
                source: entry.source,
                behavior: entry.behavior,
                reason,
            });
            continue;
        }
        let id = spawn_object_abs_with_rot(
            m,
            w,
            ObjectId::MACRO_DEFAULT_PARENT,
            i32::from(entry.model),
            entry.behavior,
            entry.pos,
            [0, entry.yaw, 0],
        );
        let obj = w.objects.slot_mut(id);
        let params = i32::from(params);
        obj.raw.set_s32(O_UNUSED_BHV_PARAMS, params);
        obj.raw
            .set_s32(O_BHV_PARAMS, ((params & 0x00FF) << 16) + (params & 0xFF00));
        obj.raw.set_s32(O_BHV_PARAMS2ND_BYTE, params & 0x00FF);
        obj.respawn_info_type = RESPAWN_INFO_TYPE_16;
        obj.respawn_info = Some(RespawnInfo::Macro(index));
        obj.parent = Some(id);
    }
}

/// spawn_objects_from_info's per-object part for one spawn info, after its
/// respawn check. Mario's info (behaviorArg bit 0) makes gMarioObject, whose
/// data moves into MarioState.
fn spawn_from_info(
    m: &mut MarioState,
    w: &mut StepWorld<'_>,
    info: &SpawnInfo,
    respawn: RespawnInfo,
) -> ObjectId {
    let script = info.behavior_script;
    let id = create_object(w, script);
    let shared_child = w.loaded_model(i32::from(info.model));
    let obj = w.objects.slot_mut(id);
    obj.raw.set_u32(O_BHV_PARAMS, info.behavior_arg);
    obj.raw.set_s32(
        O_BHV_PARAMS2ND_BYTE,
        ((info.behavior_arg >> 16) & 0xFF) as i32,
    );
    obj.behavior = script;
    obj.unused1 = 0;
    obj.respawn_info_type = RESPAWN_INFO_TYPE_32;
    obj.respawn_info = Some(respawn);
    // geo_obj_init_spawninfo.
    obj.gfx.scale = [1.0; 3];
    obj.gfx.angle = info.start_angle;
    obj.gfx.pos = info.start_pos.map(f32::from);
    obj.gfx.area_index = info.area_index;
    obj.gfx.active_area_index = info.active_area_index;
    obj.gfx.shared_child = shared_child;
    obj.gfx.throw_matrix = None;
    obj.gfx.anim.cur_anim = None;
    obj.gfx.node_flags |= GRAPH_RENDER_ACTIVE;
    obj.gfx.node_flags &= !GRAPH_RENDER_INVISIBLE;
    obj.gfx.node_flags |= GRAPH_RENDER_HAS_ANIMATION;
    obj.gfx.node_flags &= !GRAPH_RENDER_BILLBOARD;
    for (axis, field) in [O_POS_X, O_POS_Y, O_POS_Z].into_iter().enumerate() {
        obj.raw.set_f32(field, f32::from(info.start_pos[axis]));
    }
    for (axis, (face, moving)) in [
        (O_FACE_ANGLE_PITCH, O_MOVE_ANGLE_PITCH),
        (O_FACE_ANGLE_YAW, O_MOVE_ANGLE_YAW),
        (O_FACE_ANGLE_ROLL, O_MOVE_ANGLE_ROLL),
    ]
    .into_iter()
    .enumerate()
    {
        obj.raw.set_s32(face, i32::from(info.start_angle[axis]));
        obj.raw.set_s32(moving, i32::from(info.start_angle[axis]));
    }
    if info.behavior_arg & 0x01 != 0 {
        // gMarioObject = object; geo_make_first_child only orders drawing.
        m.obj = std::mem::take(w.objects.slot_mut(id));
        w.objects.mario = Some(id);
    }
    id
}

/// spawn_objects_from_info's per-call resets: gTimeStopState and
/// clear_mario_platform. The WDW water, merry-go-round and CCM slide flags
/// it also writes are read only by those levels' unported code.
fn spawn_objects_reset(w: &mut StepWorld<'_>) {
    w.time_stop_state = 0;
    w.mario_platform = None;
}

/// spawn_objects_from_info for the area's spawn infos, spawning the ported ones.
pub fn spawn_area_objects(m: &mut MarioState, w: &mut StepWorld<'_>) {
    spawn_objects_reset(w);
    for index in 0..w.area.spawn_infos.len() {
        let info = w.area.spawn_infos[index];
        if info.behavior_arg & (u32::from(RESPAWN_INFO_DONT_RESPAWN as u8) << 8)
            == u32::from(RESPAWN_INFO_DONT_RESPAWN as u8) << 8
        {
            continue;
        }
        if let Err(reason) = w.behaviors.check(info.behavior_script) {
            w.area.skipped.push(Skipped {
                source: info.source,
                behavior: info.behavior_script,
                reason,
            });
            continue;
        }
        spawn_from_info(m, w, &info, RespawnInfo::SpawnInfo(index));
    }
}

/// spawn_objects_from_info for gMarioSpawnInfo (load_mario_area).
pub fn spawn_mario(m: &mut MarioState, w: &mut StepWorld<'_>, info: &SpawnInfo) -> ObjectId {
    spawn_objects_reset(w);
    if let Err(reason) = w.behaviors.check(info.behavior_script) {
        panic!("Mario's behavior is not ported: {reason:?}");
    }
    assert!(info.behavior_arg & 1 != 0, "the spawn info is not Mario's");
    spawn_from_info(m, w, info, RespawnInfo::MarioSpawn)
}

/// set_object_respawn_info_bits: OR `bits << 8` into the object's spawn
/// info argument or macro parameter short.
pub fn set_object_respawn_info_bits(
    w: &mut StepWorld<'_>,
    info_type: i16,
    info: Option<RespawnInfo>,
    bits: u8,
) {
    match (info_type, info) {
        (RESPAWN_INFO_TYPE_32, Some(RespawnInfo::SpawnInfo(index))) => {
            w.area.spawn_infos[index].behavior_arg |= u32::from(bits) << 8;
        }
        (RESPAWN_INFO_TYPE_32, Some(RespawnInfo::MarioSpawn)) => {
            panic!("Mario's object is never unloaded")
        }
        (RESPAWN_INFO_TYPE_16, Some(RespawnInfo::Macro(index))) => {
            let params = &mut w.area.macros[index].params;
            *params = (i32::from(*params) | (i32::from(bits) << 8)) as u16;
        }
        (RESPAWN_INFO_TYPE_NULL, _) => {}
        other => panic!("inconsistent respawn info {other:?}"),
    }
}
