//! Development-only object comparison support: what the native tick harness
//! needs to spawn the same objects as the Rust port (the verbatim scripts'
//! addresses in the Rust side's behavior segment, the loaded models' render
//! traversals, and the macro entries and spawn infos whose scripts the port
//! runs), and the Rust side of the object words c/tick.c's snapshot names.
use crate::tick_trace::Words;
use rustario64::simulation::{
    mario::{MarioState, StepWorld, tick::LevelObjects},
    object::{
        AnimRef, Object, ObjectId, ObjectList, RespawnInfo, ThrowMatrix, render::RenderNodeKind,
        script::Behavior,
    },
};

/// The verbatim scripts c/behavior_data_unit.c compiles, in its order.
pub const VERBATIM_SCRIPTS: [Behavior; 17] = [
    Behavior::CoinFormationSpawn,
    Behavior::CoinFormation,
    Behavior::YellowCoin,
    Behavior::CoinSparkles,
    Behavior::GoldenCoinSparkles,
    Behavior::Mario,
    Behavior::SpinAirborneWarp,
    Behavior::SoundSpawner,
    Behavior::MovingYellowCoin,
    Behavior::Bobomb,
    Behavior::BobombFuseSmoke,
    Behavior::CarrySomething3,
    Behavior::CarrySomething4,
    Behavior::CarrySomething5,
    Behavior::Explosion,
    Behavior::BobombBullyDeathSmoke,
    Behavior::Respawner,
];

/// One animation of a host table (c/object_anims_unit.c's OracleAnimation).
#[derive(Debug, Clone, Default)]
pub struct NativeAnimation {
    pub segmented: u32,
    pub flags: i16,
    pub y_trans_divisor: i16,
    pub start_frame: i16,
    pub loop_start: i16,
    pub loop_end: i16,
    pub bone_count: i16,
    pub index: Vec<u16>,
    pub values: Vec<i16>,
}

/// One model's flattened render traversal.
#[derive(Debug, Clone, Default)]
pub struct NativeModel {
    pub model: i32,
    pub root: i32,
    pub kinds: Vec<i32>,
    pub params: Vec<i32>,
    pub child_start: Vec<i32>,
    pub child_count: Vec<i32>,
    pub children: Vec<i32>,
}

/// The native harness's object setup for one level entry.
#[derive(Debug, Clone, Default)]
pub struct NativeObjects {
    pub scripts: Vec<u32>,
    pub models: Vec<NativeModel>,
    /// The macro entries the port spawns, in the area list's format.
    pub macro_list: Vec<i16>,
    pub macro_original: Vec<i32>,
    pub preset_behaviors: Vec<u32>,
    pub preset_models: Vec<i16>,
    pub preset_params: Vec<i16>,
    pub spawn_infos: Vec<crate::OracleSpawnInfo>,
    /// bhvBobomb's LOAD_ANIMATIONS table (its segmented address in the
    /// Rust side's script) and its animations, for the host copy of
    /// bobomb_seg8_anims_0802396C.
    pub bobomb_table: u32,
    pub bobomb_animations: Vec<NativeAnimation>,
}

impl NativeObjects {
    pub fn new(objects: &LevelObjects<'_>) -> Self {
        let scripts = objects.scripts;
        let mut out = NativeObjects {
            scripts: VERBATIM_SCRIPTS
                .iter()
                .map(|b| scripts.address(*b))
                .collect(),
            preset_behaviors: vec![0; 366],
            preset_models: vec![0; 366],
            preset_params: vec![0; 366],
            ..Default::default()
        };
        for model in objects.models.ids() {
            let mut native = NativeModel {
                model: i32::from(model),
                ..Default::default()
            };
            if let Some(traversal) = objects.models.traversal(model) {
                native.root = traversal.root as i32;
                for node in &traversal.nodes {
                    let (kind, param) = match node.kind {
                        RenderNodeKind::Plain => (0, 0),
                        RenderNodeKind::AnimStateSwitch { num_cases } => (1, i32::from(num_cases)),
                        RenderNodeKind::CullingRadius { radius } => (2, i32::from(radius)),
                        RenderNodeKind::Unaudited { .. } => (3, 0),
                    };
                    native.kinds.push(kind);
                    native.params.push(param);
                    native.child_start.push(native.children.len() as i32);
                    native.child_count.push(node.children.len() as i32);
                    native
                        .children
                        .extend(node.children.iter().map(|c| *c as i32));
                }
            }
            out.models.push(native);
        }
        let bobomb = scripts.address(Behavior::Bobomb);
        if let Ok(tables) = scripts.animation_tables(bobomb) {
            let table = *tables
                .iter()
                .next()
                .expect("bhvBobomb loads one animation table");
            out.bobomb_table = table;
            if let Some(entries) = objects.animations.tables.get(&table) {
                for address in entries {
                    let a = &objects.animations.animations[address];
                    out.bobomb_animations.push(NativeAnimation {
                        segmented: *address,
                        flags: a.flags,
                        y_trans_divisor: a.y_trans_divisor,
                        start_frame: a.start_frame,
                        loop_start: a.loop_start,
                        loop_end: a.loop_end,
                        bone_count: a.bone_count,
                        index: a.index.clone(),
                        values: a.values.clone(),
                    });
                }
            }
        }
        for (index, entry) in objects.area.macros.iter().enumerate() {
            if scripts.check(entry.behavior).is_err() {
                continue;
            }
            out.macro_list.extend([
                entry.packed as i16,
                entry.pos[0],
                entry.pos[1],
                entry.pos[2],
                entry.params as i16,
            ]);
            out.macro_original.push(index as i32);
            let preset = usize::from(entry.preset);
            out.preset_behaviors[preset] = entry.behavior;
            out.preset_models[preset] = entry.model;
            out.preset_params[preset] = entry.preset_param;
        }
        for (index, info) in objects.area.spawn_infos.iter().enumerate() {
            if scripts.check(info.behavior_script).is_err() {
                continue;
            }
            out.spawn_infos.push(crate::OracleSpawnInfo {
                start_pos: info.start_pos.map(i32::from),
                start_angle: info.start_angle.map(i32::from),
                area_index: i32::from(info.area_index),
                active_area_index: i32::from(info.active_area_index),
                behavior_arg: info.behavior_arg,
                behavior_script: info.behavior_script,
                model: i32::from(info.model),
                original: index as i32,
            });
        }
        out
    }
}

/// An object reference as c/tick.c's object_id names it.
pub fn object_id(object: Option<ObjectId>) -> i32 {
    match object {
        None => -1,
        Some(ObjectId::MACRO_DEFAULT_PARENT) => -2,
        Some(id) => id.0 as i32,
    }
}

fn respawn_id(info: Option<RespawnInfo>) -> i32 {
    match info {
        None => -1,
        Some(RespawnInfo::Macro(index) | RespawnInfo::SpawnInfo(index)) => index as i32,
        Some(RespawnInfo::MarioSpawn) => -2,
    }
}

/// Every compared field of an object, as c/tick.c's put_object names it.
pub fn put_object(o: &mut Words, prefix: &str, obj: &Object) {
    let gfx = &obj.gfx;
    let name = |field: &str| format!("{prefix}.{field}");
    o.i(name("gfx.flags"), i32::from(gfx.node_flags));
    o.i(name("gfx.areaIndex"), i32::from(gfx.area_index));
    o.i(
        name("gfx.activeAreaIndex"),
        i32::from(gfx.active_area_index),
    );
    o.i(
        name("gfx.sharedChild"),
        gfx.shared_child.map_or(-1, i32::from),
    );
    o.s16v(&name("gfx.angle"), &gfx.angle);
    o.f32v(&name("gfx.pos"), &gfx.pos);
    o.f32v(&name("gfx.scale"), &gfx.scale);
    o.i(name("gfx.anim.animID"), i32::from(gfx.anim.anim_id));
    o.i(
        name("gfx.anim.animYTrans"),
        i32::from(gfx.anim.anim_y_trans),
    );
    o.put(
        name("gfx.anim.curAnim"),
        match gfx.anim.cur_anim {
            None => 0,
            Some(AnimRef::MarioDmaBuffer) => 1,
            Some(AnimRef::Object(address)) => address,
        },
    );
    o.i(name("gfx.anim.animFrame"), i32::from(gfx.anim.anim_frame));
    o.put(name("gfx.anim.animTimer"), u32::from(gfx.anim.anim_timer));
    o.i(
        name("gfx.anim.animFrameAccelAssist"),
        gfx.anim.anim_frame_accel_assist,
    );
    o.i(name("gfx.anim.animAccel"), gfx.anim.anim_accel);
    match gfx.throw_matrix {
        None => o.i(name("gfx.throwMatrix"), -1),
        Some(ThrowMatrix::FloorAlign(index)) => o.i(name("gfx.throwMatrix"), index as i32),
        Some(ThrowMatrix::Terrain(matrix)) => {
            o.i(name("gfx.throwMatrix"), 2);
            for (i, value) in matrix.as_flattened().iter().enumerate() {
                o.f(name(&format!("gfx.throwMatrixWords[{i}]")), *value);
            }
        }
    }
    o.put(
        name("collidedObjInteractTypes"),
        obj.collided_obj_interact_types,
    );
    o.i(name("activeFlags"), i32::from(obj.active_flags));
    o.i(name("numCollidedObjs"), i32::from(obj.num_collided_objs));
    for i in 0..4 {
        let id = if (i as i16) < obj.num_collided_objs {
            object_id(obj.collided_objs[i])
        } else {
            -1
        };
        o.i(name(&format!("collidedObjs[{i}]")), id);
    }
    for (i, word) in obj.raw.0.iter().enumerate() {
        o.put(name(&format!("raw[0x{i:02X}]")), *word);
    }
    o.put(name("unused1"), obj.unused1);
    o.put(name("bhvStackIndex"), obj.bhv_stack_index);
    for i in 0..8 {
        let value = if (i as u32) < obj.bhv_stack_index {
            obj.bhv_stack[i]
        } else {
            0
        };
        o.put(name(&format!("bhvStack[{i}]")), value);
    }
    o.i(name("bhvDelayTimer"), i32::from(obj.bhv_delay_timer));
    o.i(name("respawnInfoType"), i32::from(obj.respawn_info_type));
    o.i(name("respawnInfo"), respawn_id(obj.respawn_info));
    o.f(name("hitboxRadius"), obj.hitbox_radius);
    o.f(name("hitboxHeight"), obj.hitbox_height);
    o.f(name("hurtboxRadius"), obj.hurtbox_radius);
    o.f(name("hurtboxHeight"), obj.hurtbox_height);
    o.f(name("hitboxDownOffset"), obj.hitbox_down_offset);
    o.put(name("behavior"), obj.behavior);
    o.put(name("curBhvCommand"), obj.cur_bhv_command);
    o.i(name("platform"), object_id(obj.platform));
    o.i(
        name("collisionData"),
        if obj.collision_data.is_none() { -1 } else { 1 },
    );
    o.i(name("parentObj"), object_id(obj.parent));
    o.i(name("prevObj"), object_id(obj.prev_obj));
}

/// The pool, as c/tick.c's snapshot_objects names it.
pub fn capture_objects(o: &mut Words, m: &MarioState, w: &StepWorld<'_>) {
    let mario = w.objects.mario;
    for index in 0..rustario64::simulation::object::OBJECT_LIST_COUNT {
        let list = ObjectList::try_from(index as u16).unwrap();
        let ids = w.objects.list(list);
        for (i, id) in ids.iter().enumerate() {
            o.i(format!("lists[{index}][{i}]"), id.0 as i32);
            if Some(*id) != mario {
                put_object(o, &format!("objects[{}]", id.0), w.objects.slot(*id));
            }
        }
        o.i(format!("lists[{index}].count"), ids.len() as i32);
    }
    let free = w.objects.free_list();
    for (i, id) in free.iter().enumerate() {
        o.i(format!("free[{i}]"), id.0 as i32);
    }
    o.i("free.count", free.len() as i32);
    o.i("world.marioObject", object_id(mario));
    o.i("world.currentObject", object_id(w.objects.current));
    o.put("world.timeStopState", w.time_stop_state);
    o.i("world.rngSeed", i32::from(w.rng.seed));
    for (index, entry) in w.area.macros.iter().enumerate() {
        if w.behaviors.check(entry.behavior).is_ok() {
            o.i(
                format!("area.macro[{index}].params"),
                i32::from(entry.params),
            );
        }
    }
    for (index, info) in w.area.spawn_infos.iter().enumerate() {
        if w.behaviors.check(info.behavior_script).is_ok() {
            o.put(
                format!("area.spawnInfo[{index}].behaviorArg"),
                info.behavior_arg,
            );
        }
    }
    let _ = m;
}
