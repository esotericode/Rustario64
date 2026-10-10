//! The original object system, translated from pinned CC0 n64decomp/sm64:
//! the object pool and its lists (spawn_object.c, object_list_processor.c),
//! the behavior-script interpreter (behavior_script.c), object collision
//! detection (object_collision.c), the object helpers the ported behaviors
//! use (object_helpers.c), and those behaviors (`coin`). Behavior scripts are
//! the ROM's own (segment 0x13), interpreted from their bytes; their native
//! functions resolve through the version adapter to Rust translations.
//!
//! Mario's object is an ordinary [`Object`] held in `MarioState` (the code
//! that updates him reads `m->marioObj` constantly); the pool's slot for it
//! holds only its list links. [`object`] and [`object_mut`] resolve any
//! handle, including Mario's and gMacroObjectDefaultParent.
pub mod coin;
pub mod collision;
pub mod helpers;
pub mod processor;
pub mod render;
pub mod script;
pub mod spawn;

use crate::simulation::mario::{Mat4, constants::*};

/// Original list indices, including the unused slots. Preserve these IDs
/// rather than renumbering the lists that currently have gameplay users.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectList {
    Player = 0,
    Unused1 = 1,
    Destructive = 2,
    Unused3 = 3,
    GeneralActor = 4,
    Pushable = 5,
    Level = 6,
    Unused7 = 7,
    Default = 8,
    Surface = 9,
    Polelike = 10,
    Spawner = 11,
    Unimportant = 12,
}

pub const OBJECT_LIST_COUNT: usize = 13;

impl TryFrom<u16> for ObjectList {
    type Error = u16;

    fn try_from(index: u16) -> Result<Self, Self::Error> {
        match index {
            0 => Ok(Self::Player),
            1 => Ok(Self::Unused1),
            2 => Ok(Self::Destructive),
            3 => Ok(Self::Unused3),
            4 => Ok(Self::GeneralActor),
            5 => Ok(Self::Pushable),
            6 => Ok(Self::Level),
            7 => Ok(Self::Unused7),
            8 => Ok(Self::Default),
            9 => Ok(Self::Surface),
            10 => Ok(Self::Polelike),
            11 => Ok(Self::Spawner),
            12 => Ok(Self::Unimportant),
            _ => Err(index),
        }
    }
}

impl ObjectList {
    /// create_object selects a list from the first behavior word only.
    /// A non-BEGIN opcode selects DEFAULT regardless of the remaining bits.
    /// A malformed BEGIN is rejected before it can index outside the lists.
    pub fn from_behavior_word(word: u32) -> Result<Self, u16> {
        if word >> 24 == 0 {
            Self::try_from(((word >> 16) & 0xFFFF) as u16)
        } else {
            Ok(Self::Default)
        }
    }
}

/// Original frame traversal (sObjectListUpdateOrder), without its -1
/// sentinel. Unused lists are never visited.
pub const UPDATE_ORDER: [ObjectList; 10] = [
    ObjectList::Spawner,
    ObjectList::Surface,
    ObjectList::Polelike,
    ObjectList::Player,
    ObjectList::Pushable,
    ObjectList::GeneralActor,
    ObjectList::Destructive,
    ObjectList::Level,
    ObjectList::Default,
    ObjectList::Unimportant,
];

/// An object handle: a slot of the 240-object pool (gObjectPool), or
/// gMacroObjectDefaultParent. Collision surfaces name their object with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u32);

impl ObjectId {
    /// gMacroObjectDefaultParent, the static object macro and special
    /// objects are spawned from (it is not in the pool).
    pub const MACRO_DEFAULT_PARENT: ObjectId = ObjectId(u32::MAX);

    pub fn slot(self) -> usize {
        self.0 as usize
    }
}

/// What `animInfo.curAnim` points to. Mario's animations live in one DMA
/// buffer whose content is `StepWorld::anim_dma_loaded`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimRef {
    MarioDmaBuffer,
}

/// struct AnimInfo. The frame advance is part of the authoritative tick
/// because actions read the frame (see `mario::animation::update_animation_frame`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AnimInfo {
    pub anim_id: i16,
    pub anim_y_trans: i16,
    pub cur_anim: Option<AnimRef>,
    pub anim_frame: i16,
    pub anim_timer: u16,
    pub anim_frame_accel_assist: i32,
    pub anim_accel: i32,
}

/// The header.gfx fields (struct GraphNodeObject) that gameplay code writes.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GfxState {
    pub node_flags: i16,
    pub area_index: i8,
    pub active_area_index: i8,
    /// sharedChild: the model ID whose gLoadedGraphNodes entry it holds, or
    /// NULL (MODEL_NONE and models the level did not load).
    pub shared_child: Option<u16>,
    pub angle: [i16; 3],
    pub pos: [f32; 3],
    pub scale: [f32; 3],
    pub anim: AnimInfo,
    /// throwMatrix: an index into `StepWorld::floor_align_matrix` or NULL.
    /// Other objects' render-pass matrices are not authoritative and stay NULL.
    pub throw_matrix: Option<usize>,
}

/// The object field union (rawData): 0x50 words viewed as s32, u32 or f32.
/// Original names alias the same word (for Mario, slot 0x22 holds the walking
/// pitch, long-jump flag, burn timer and steep-jump yaw), so storage is shared.
/// S16 sub-fields follow the N64's big-endian layout (`[i][0]` is the high
/// half) when they are needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectFields(pub [u32; 0x50]);

impl Default for ObjectFields {
    fn default() -> Self {
        Self([0; 0x50])
    }
}

impl ObjectFields {
    pub fn s32(&self, index: usize) -> i32 {
        self.0[index] as i32
    }
    pub fn set_s32(&mut self, index: usize, value: i32) {
        self.0[index] = value as u32;
    }
    pub fn u32(&self, index: usize) -> u32 {
        self.0[index]
    }
    pub fn set_u32(&mut self, index: usize, value: u32) {
        self.0[index] = value;
    }
    pub fn f32(&self, index: usize) -> f32 {
        f32::from_bits(self.0[index])
    }
    pub fn set_f32(&mut self, index: usize, value: f32) {
        self.0[index] = value.to_bits();
    }
}

/// Where an object records that it was collected or killed (respawnInfo):
/// the behaviorArg of an area spawn info (RESPAWN_INFO_TYPE_32), or the
/// parameter short of a macro-object entry (RESPAWN_INFO_TYPE_16), by index
/// into the area's lists (`spawn::AreaObjects`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RespawnInfo {
    SpawnInfo(usize),
    Macro(usize),
    /// gMarioSpawnInfo's behaviorArg.
    MarioSpawn,
}

/// struct Object: the members gameplay code reads or writes.
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub gfx: GfxState,
    pub collided_obj_interact_types: u32,
    pub active_flags: i16,
    pub num_collided_objs: i16,
    pub collided_objs: [Option<ObjectId>; 4],
    pub raw: ObjectFields,
    pub unused1: u32,
    pub bhv_stack_index: u32,
    /// Return addresses (segmented) and repeat counts, as the original's
    /// uintptr_t stack holds them.
    pub bhv_stack: [u32; 8],
    pub bhv_delay_timer: i16,
    pub respawn_info_type: i16,
    pub hitbox_radius: f32,
    pub hitbox_height: f32,
    pub hurtbox_radius: f32,
    pub hurtbox_height: f32,
    pub hitbox_down_offset: f32,
    /// behavior: the segmented address of the script it was created with (or
    /// a later cur_obj_set_behavior); 0 is NULL.
    pub behavior: u32,
    /// curBhvCommand: the segmented address of the next command.
    pub cur_bhv_command: u32,
    pub platform: Option<ObjectId>,
    /// collisionData, as a segmented address. No ported behavior loads one.
    pub collision_data: Option<u32>,
    pub transform: Mat4,
    pub respawn_info: Option<RespawnInfo>,
    /// parentObj (NULL only before allocation).
    pub parent: Option<ObjectId>,
    pub prev_obj: Option<ObjectId>,
}

impl Default for Object {
    fn default() -> Self {
        Self {
            gfx: GfxState::default(),
            collided_obj_interact_types: 0,
            active_flags: 0,
            num_collided_objs: 0,
            collided_objs: [None; 4],
            raw: ObjectFields::default(),
            unused1: 0,
            bhv_stack_index: 0,
            bhv_stack: [0; 8],
            bhv_delay_timer: 0,
            respawn_info_type: 0,
            hitbox_radius: 0.0,
            hitbox_height: 0.0,
            hurtbox_radius: 0.0,
            hurtbox_height: 0.0,
            hitbox_down_offset: 0.0,
            behavior: 0,
            cur_bhv_command: 0,
            platform: None,
            collision_data: None,
            transform: [[0.0; 4]; 4],
            respawn_info: None,
            parent: None,
            prev_obj: None,
        }
    }
}

impl Object {
    /// geo_reset_object_node, as clear_objects leaves every pool slot:
    /// init_graph_node_object with no model at the origin, then inactive.
    pub fn reset_node(&mut self) {
        self.gfx.node_flags = GRAPH_RENDER_ACTIVE | GRAPH_RENDER_HAS_ANIMATION;
        self.gfx.shared_child = None;
        self.gfx.pos = [0.0; 3];
        self.gfx.angle = [0; 3];
        self.gfx.scale = [1.0; 3];
        self.gfx.throw_matrix = None;
        self.gfx.anim = AnimInfo {
            anim_accel: 0x10000,
            ..AnimInfo::default()
        };
        self.gfx.node_flags &= !GRAPH_RENDER_ACTIVE;
    }

    pub fn pos(&self) -> [f32; 3] {
        [
            self.raw.f32(O_POS_X),
            self.raw.f32(O_POS_Y),
            self.raw.f32(O_POS_Z),
        ]
    }
}

/// One end of an ObjectNode link: a list head (gObjectListArray) or a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    Head(ObjectList),
    Slot(ObjectId),
}

/// An ObjectNode's links. A free slot's `next` is the free list's next slot
/// (None ends it); `prev` keeps its stale value, as the original's does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Links {
    pub next: Option<Node>,
    pub prev: Option<Node>,
}

/// gObjectPool, gObjectListArray, gFreeObjectList and the globals that name
/// objects (gMarioObject, gCurrentObject, gMacroObjectDefaultParent).
#[derive(Debug, Clone)]
pub struct ObjectPool {
    slots: Vec<Object>,
    /// Presentation lifetime tokens. Never read by gameplay or reference traces.
    generations: Vec<u64>,
    links: Vec<Links>,
    heads: [Links; OBJECT_LIST_COUNT],
    /// gFreeObjectList.next.
    free: Option<ObjectId>,
    /// gMarioObject's slot; its data is MarioState's object.
    pub mario: Option<ObjectId>,
    /// gCurrentObject: the object whose update or unload check ran last.
    pub current: Option<ObjectId>,
    /// gMacroObjectDefaultParent.
    pub macro_default_parent: Object,
}

impl Default for ObjectPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectPool {
    /// The pool as clear_objects leaves it: every slot free in pool order
    /// (init_free_object_list), the lists empty (clear_object_lists), every
    /// slot deactivated with a reset graph node.
    pub fn new() -> Self {
        let mut slot = Object {
            active_flags: ACTIVE_FLAG_DEACTIVATED,
            ..Default::default()
        };
        slot.reset_node();
        let links = (0..OBJECT_POOL_CAPACITY)
            .map(|i| Links {
                next: (i + 1 < OBJECT_POOL_CAPACITY).then(|| Node::Slot(ObjectId((i + 1) as u32))),
                prev: None,
            })
            .collect();
        let mut heads = [Links {
            next: None,
            prev: None,
        }; OBJECT_LIST_COUNT];
        for (i, head) in heads.iter_mut().enumerate() {
            let me = Node::Head(ObjectList::try_from(i as u16).unwrap());
            *head = Links {
                next: Some(me),
                prev: Some(me),
            };
        }
        Self {
            slots: vec![slot; OBJECT_POOL_CAPACITY],
            generations: vec![0; OBJECT_POOL_CAPACITY],
            links,
            heads,
            free: Some(ObjectId(0)),
            mario: None,
            current: None,
            macro_default_parent: Object::default(),
        }
    }

    /// A slot's data. Mario's slot holds only links; read his object from
    /// MarioState (see [`object`]).
    pub fn slot(&self, id: ObjectId) -> &Object {
        assert!(
            Some(id) != self.mario,
            "Mario's object lives in MarioState, not in the pool"
        );
        if id == ObjectId::MACRO_DEFAULT_PARENT {
            return &self.macro_default_parent;
        }
        &self.slots[id.slot()]
    }

    pub fn slot_mut(&mut self, id: ObjectId) -> &mut Object {
        assert!(
            Some(id) != self.mario,
            "Mario's object lives in MarioState, not in the pool"
        );
        if id == ObjectId::MACRO_DEFAULT_PARENT {
            return &mut self.macro_default_parent;
        }
        &mut self.slots[id.slot()]
    }

    pub fn links(&self, node: Node) -> &Links {
        match node {
            Node::Head(list) => &self.heads[list as usize],
            Node::Slot(id) => &self.links[id.slot()],
        }
    }

    fn links_mut(&mut self, node: Node) -> &mut Links {
        match node {
            Node::Head(list) => &mut self.heads[list as usize],
            Node::Slot(id) => &mut self.links[id.slot()],
        }
    }

    /// The node after `node` in its list.
    pub fn next(&self, node: Node) -> Node {
        self.links(node)
            .next
            .expect("a listed object node always has a next node")
    }

    /// The objects of `list`, first to last.
    pub fn list(&self, list: ObjectList) -> Vec<ObjectId> {
        let mut out = vec![];
        let mut node = self.next(Node::Head(list));
        while node != Node::Head(list) {
            let Node::Slot(id) = node else {
                panic!("a list contains another list's head")
            };
            out.push(id);
            node = self.next(node);
        }
        out
    }

    /// gFreeObjectList.next.
    pub fn first_free(&self) -> Option<ObjectId> {
        self.free
    }

    /// The free list's slots in order (the next allocations).
    pub fn free_list(&self) -> Vec<ObjectId> {
        let mut out = vec![];
        let mut next = self.free;
        while let Some(id) = next {
            out.push(id);
            next = match self.links[id.slot()].next {
                Some(Node::Slot(id)) => Some(id),
                None => None,
                Some(Node::Head(_)) => panic!("the free list reached a list head"),
            };
        }
        out
    }

    /// try_allocate_object: take the first free slot and append it to the
    /// end of `list`. (Its graph node's move to the end of gObjParentGraphNode's
    /// children only orders drawing.)
    fn try_allocate(&mut self, list: ObjectList) -> Option<ObjectId> {
        let id = self.free?;
        self.generations[id.slot()] = self.generations[id.slot()].wrapping_add(1);
        self.free = match self.links[id.slot()].next {
            Some(Node::Slot(next)) => Some(next),
            None => None,
            Some(Node::Head(_)) => panic!("the free list reached a list head"),
        };
        let head = Node::Head(list);
        let last = self.links(head).prev.unwrap();
        self.links[id.slot()] = Links {
            next: Some(head),
            prev: Some(last),
        };
        self.links_mut(last).next = Some(Node::Slot(id));
        self.links_mut(head).prev = Some(Node::Slot(id));
        Some(id)
    }

    /// Distinguish successive occupants of the same slot when interpolating.
    pub fn generation(&self, id: ObjectId) -> u64 {
        self.generations[id.slot()]
    }

    /// deallocate_object: unlink the slot and push it on the free list.
    fn deallocate(&mut self, id: ObjectId) {
        let links = self.links[id.slot()];
        let (next, prev) = (links.next.unwrap(), links.prev.unwrap());
        self.links_mut(next).prev = Some(prev);
        self.links_mut(prev).next = Some(next);
        self.links[id.slot()].next = self.free.map(Node::Slot);
        self.free = Some(id);
    }

    /// find_unimportant_object: the first object of OBJ_LIST_UNIMPORTANT.
    pub fn find_unimportant_object(&self) -> Option<ObjectId> {
        match self.next(Node::Head(ObjectList::Unimportant)) {
            Node::Slot(id) => Some(id),
            Node::Head(_) => None,
        }
    }

    /// allocate_object: a slot at the end of `list`, freeing the first
    /// unimportant object if the pool is full, with allocate_object's field
    /// values. The original hangs when no unimportant object exists; the port
    /// panics.
    pub fn allocate_object(&mut self, list: ObjectList, level_num: i16) -> ObjectId {
        let id = match self.try_allocate(list) {
            Some(id) => id,
            None => {
                let victim = self
                    .find_unimportant_object()
                    .expect("the object pool is exhausted (the original hangs)");
                self.unload_object(victim);
                self.try_allocate(list).unwrap()
            }
        };
        init_allocated(self.slot_mut(id), id, level_num);
        id
    }

    /// unload_object. stop_sounds_from_source has no simulated state, and the
    /// graph node's return to gObjParentGraphNode only orders drawing.
    pub fn unload_object(&mut self, id: ObjectId) {
        let obj = self.slot_mut(id);
        obj.active_flags = ACTIVE_FLAG_DEACTIVATED;
        obj.prev_obj = None;
        obj.gfx.throw_matrix = None;
        obj.gfx.node_flags &= !GRAPH_RENDER_BILLBOARD;
        obj.gfx.node_flags &= !GRAPH_RENDER_ACTIVE;
        self.deallocate(id);
    }

    /// The list an allocated slot belongs to, found from its links.
    pub fn list_of(&self, id: ObjectId) -> ObjectList {
        let mut node = Node::Slot(id);
        loop {
            node = self.next(node);
            if let Node::Head(list) = node {
                return list;
            }
        }
    }
}

/// Any object by handle: Mario's from MarioState, the rest from the pool.
pub fn object<'x>(pool: &'x ObjectPool, mario: &'x Object, id: ObjectId) -> &'x Object {
    if pool.mario == Some(id) {
        mario
    } else {
        pool.slot(id)
    }
}

pub fn object_mut<'x>(
    pool: &'x mut ObjectPool,
    mario: &'x mut Object,
    id: ObjectId,
) -> &'x mut Object {
    if pool.mario == Some(id) {
        mario
    } else {
        pool.slot_mut(id)
    }
}

/// Two different objects at once (detect_object_hitbox_overlap's a and b).
pub fn pair_mut<'x>(
    pool: &'x mut ObjectPool,
    mario: &'x mut Object,
    a: ObjectId,
    b: ObjectId,
) -> (&'x mut Object, &'x mut Object) {
    assert!(a != b, "an object cannot collide with itself");
    let mario_id = pool.mario;
    if mario_id == Some(a) {
        (mario, pool.slot_mut(b))
    } else if mario_id == Some(b) {
        (pool.slot_mut(a), mario)
    } else {
        assert!(
            a != ObjectId::MACRO_DEFAULT_PARENT && b != ObjectId::MACRO_DEFAULT_PARENT,
            "gMacroObjectDefaultParent is never in an object list"
        );
        let (low, high) = (a.slot().min(b.slot()), a.slot().max(b.slot()));
        let (left, right) = pool.slots.split_at_mut(high);
        let (x, y) = (&mut left[low], &mut right[0]);
        if a.slot() < b.slot() { (x, y) } else { (y, x) }
    }
}

/// allocate_object's field initialization, for a slot taken from the free
/// list into `list` in level `level_num`.
pub fn init_allocated(obj: &mut Object, id: ObjectId, level_num: i16) {
    obj.active_flags = ACTIVE_FLAG_ACTIVE | ACTIVE_FLAG_UNK8;
    obj.parent = Some(id);
    obj.prev_obj = None;
    obj.collided_obj_interact_types = 0;
    obj.num_collided_objs = 0;
    obj.raw = ObjectFields::default();
    obj.unused1 = 0;
    obj.bhv_stack_index = 0;
    obj.bhv_delay_timer = 0;
    obj.hitbox_radius = 50.0;
    obj.hitbox_height = 100.0;
    obj.hurtbox_radius = 0.0;
    obj.hurtbox_height = 0.0;
    obj.hitbox_down_offset = 0.0;
    obj.platform = None;
    obj.collision_data = None;
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
    obj.transform = crate::simulation::math::mtxf_identity();
    obj.respawn_info_type = RESPAWN_INFO_TYPE_NULL;
    obj.respawn_info = None;
    obj.raw.set_f32(O_DISTANCE_TO_MARIO, 19000.0);
    obj.raw.set_s32(O_ROOM, -1);
    obj.gfx.node_flags &= !GRAPH_RENDER_INVISIBLE;
    obj.gfx.pos = [-10000.0; 3];
    obj.gfx.throw_matrix = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_word_selection_covers_every_opcode_and_list_byte() {
        for opcode in 0u32..=255 {
            for list in 0u32..=255 {
                for low in [0, 0x1234, 0xFFFF] {
                    let word = (opcode << 24) | (list << 16) | low;
                    let result = ObjectList::from_behavior_word(word);
                    if opcode != 0 {
                        assert_eq!(result, Ok(ObjectList::Default));
                    } else if list < OBJECT_LIST_COUNT as u32 {
                        assert_eq!(u32::from(result.unwrap() as u8), list);
                    } else {
                        assert_eq!(result, Err(list as u16));
                    }
                }
            }
        }
    }

    #[test]
    fn original_update_order_preserves_ids_and_omits_unused_lists() {
        assert_eq!(
            UPDATE_ORDER.map(|list| list as u8),
            [11, 9, 10, 0, 5, 4, 2, 6, 8, 12]
        );
        for (index, list) in UPDATE_ORDER.iter().enumerate() {
            assert!(!UPDATE_ORDER[..index].contains(list));
        }
        assert_eq!(ObjectList::try_from(13), Err(13));
        assert_eq!(ObjectList::try_from(u16::MAX), Err(u16::MAX));
    }
}
