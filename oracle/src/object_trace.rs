//! Development-only object comparison support: what the native tick harness
//! needs to spawn the same objects as the Rust port (the verbatim scripts'
//! addresses in the Rust side's behavior segment, the loaded models' render
//! traversals, and the macro entries and spawn infos whose scripts the port
//! runs), and the Rust side of the object words c/tick.c's snapshot names.
use crate::{OracleGeoNode, tick_trace::Words};
use rustario64::import::geo::GeoNodeKind;
use rustario64::simulation::{
    mario::{
        MarioState, StepWorld,
        render::{MarioCallback, MarioModel},
        tick::LevelObjects,
    },
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
    /// MODEL_MARIO's graph for c/mario_render_unit.c (empty without one),
    /// and its root's index.
    pub mario_model: Vec<OracleGeoNode>,
    pub mario_root: i32,
}

/// MODEL_MARIO's nodes in registration order with their parents, as
/// c/mario_render_unit.c builds them.
pub fn mario_geo_nodes(model: &MarioModel) -> (Vec<OracleGeoNode>, i32) {
    let layout = &model.layout;
    let mut parent = vec![-1i32; layout.nodes.len()];
    for (i, node) in layout.nodes.iter().enumerate() {
        for &child in &node.children {
            parent[child] = i as i32;
        }
    }
    let role = |i: usize| match model.role(i) {
        None => -1,
        Some(role) => match role {
            MarioCallback::MirrorBackfaceCulling => 0,
            MarioCallback::MirrorSetAlpha => 1,
            MarioCallback::SwitchStandRun => 2,
            MarioCallback::SwitchCapEffect => 3,
            MarioCallback::SwitchCapOnOff => 4,
            MarioCallback::SwitchEyes => 5,
            MarioCallback::SwitchHand => 6,
            MarioCallback::HeadRotation => 7,
            MarioCallback::TiltTorso => 8,
            MarioCallback::RotateWingCapWings => 9,
            MarioCallback::HandFootScaler => 10,
            MarioCallback::MovePartFromParent => 11,
            MarioCallback::HandGrabPos => 12,
        },
    };
    let nodes = layout
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let mut out = OracleGeoNode {
                parent: parent[i],
                layer: i32::from(node.kind.layer()),
                callback: role(i),
                flags: i32::from(node.flags as i16),
                ..Default::default()
            };
            let dl = |d: Option<u32>| i32::from(d.is_some());
            match node.kind {
                GeoNodeKind::Start => out.kind = 0,
                GeoNodeKind::LevelOfDetail {
                    min_distance,
                    max_distance,
                } => {
                    out.kind = 1;
                    out.param = i32::from(min_distance);
                    out.param2 = i32::from(max_distance);
                }
                GeoNodeKind::SwitchCase { num_cases, .. } => {
                    out.kind = 2;
                    out.param = i32::from(num_cases);
                }
                GeoNodeKind::TranslationRotation {
                    translation,
                    rotation,
                    display_list,
                    ..
                } => {
                    out.kind = 3;
                    out.a = translation;
                    out.b = rotation;
                    out.has_display_list = dl(display_list);
                }
                GeoNodeKind::Translation {
                    translation,
                    display_list,
                    ..
                } => {
                    out.kind = 4;
                    out.a = translation;
                    out.has_display_list = dl(display_list);
                }
                GeoNodeKind::Rotation {
                    rotation,
                    display_list,
                    ..
                } => {
                    out.kind = 5;
                    out.b = rotation;
                    out.has_display_list = dl(display_list);
                }
                GeoNodeKind::Scale {
                    scale,
                    display_list,
                    ..
                } => {
                    out.kind = 6;
                    out.scale = scale;
                    out.has_display_list = dl(display_list);
                }
                GeoNodeKind::AnimatedPart {
                    translation,
                    display_list,
                    ..
                } => {
                    out.kind = 7;
                    out.a = translation;
                    out.has_display_list = dl(display_list);
                }
                GeoNodeKind::Billboard {
                    translation,
                    display_list,
                    ..
                } => {
                    out.kind = 8;
                    out.a = translation;
                    out.has_display_list = dl(display_list);
                }
                GeoNodeKind::DisplayList { .. } => {
                    out.kind = 9;
                    out.has_display_list = 1;
                }
                GeoNodeKind::Shadow {
                    shadow_type,
                    solidity,
                    scale,
                } => {
                    out.kind = 10;
                    out.param = i32::from(shadow_type);
                    out.param2 = i32::from(solidity);
                    out.param3 = i32::from(scale);
                }
                GeoNodeKind::Generated { param, .. } => {
                    out.kind = 11;
                    out.param = i32::from(param);
                }
                GeoNodeKind::HeldObject { param, offset, .. } => {
                    out.kind = 12;
                    out.param = i32::from(param);
                    out.a = offset;
                }
                GeoNodeKind::CullingRadius { radius } => {
                    out.kind = 13;
                    out.param = i32::from(radius);
                }
                ref other => panic!("Mario's model holds a {other:?} node"),
            }
            out
        })
        .collect();
    let root = layout.root.expect("Mario's model has no root") as i32;
    (nodes, root)
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
            mario_root: -1,
            ..Default::default()
        };
        if let Some(model) = objects.models.mario_model() {
            (out.mario_model, out.mario_root) = mario_geo_nodes(model);
        }
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
