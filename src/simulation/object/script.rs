//! Behavior scripts and their interpreter, translated from pinned CC0
//! src/engine/behavior_script.c (`cur_obj_update` and the command table).
//!
//! Scripts are the ROM's segment 0x13 words, read in place: addresses stay
//! segmented (0x13xxxxxx) wherever the original holds a virtual address
//! (curBhvCommand, the behavior stack, `behavior`), which is a one-to-one
//! renaming. CALL_NATIVE addresses resolve through the version adapter's
//! table to [`Native`] translations; a level only spawns an object whose
//! script, the scripts it can reach and the natives they call are all ported
//! ([`BehaviorScripts::check`]), so the interpreter never meets an unknown
//! function. Paths the original takes into unported systems panic.
use super::{ObjectId, helpers, object, object_mut};
use crate::simulation::mario::{MarioState, StepWorld, constants::*, f32_to_s32};
use std::collections::{BTreeMap, BTreeSet};

/// The behavior segment.
pub const SCRIPT_SEGMENT: u8 = 0x13;

/// Behavior scripts the port names: the ones ported behaviors spawn or
/// compare against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Behavior {
    Mario,
    YellowCoin,
    CoinFormation,
    CoinFormationSpawn,
    CoinSparkles,
    GoldenCoinSparkles,
    /// A BREAK: the warp node object level entries spin Mario out of.
    SpinAirborneWarp,
    /// Compared by bhv_cmd_begin.
    MessagePanel,
    HauntedChair,
    MadPiano,
}

impl Behavior {
    pub const ALL: [Behavior; 10] = [
        Behavior::Mario,
        Behavior::YellowCoin,
        Behavior::CoinFormation,
        Behavior::CoinFormationSpawn,
        Behavior::CoinSparkles,
        Behavior::GoldenCoinSparkles,
        Behavior::SpinAirborneWarp,
        Behavior::MessagePanel,
        Behavior::HauntedChair,
        Behavior::MadPiano,
    ];

    /// The pinned data/behavior_data.c name.
    pub fn name(self) -> &'static str {
        match self {
            Behavior::Mario => "bhvMario",
            Behavior::YellowCoin => "bhvYellowCoin",
            Behavior::CoinFormation => "bhvCoinFormation",
            Behavior::CoinFormationSpawn => "bhvCoinFormationSpawn",
            Behavior::CoinSparkles => "bhvCoinSparkles",
            Behavior::GoldenCoinSparkles => "bhvGoldenCoinSparkles",
            Behavior::SpinAirborneWarp => "bhvSpinAirborneWarp",
            Behavior::MessagePanel => "bhvMessagePanel",
            Behavior::HauntedChair => "bhvHauntedChair",
            Behavior::MadPiano => "bhvMadPiano",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.name() == name)
    }
}

/// Ported native behavior functions (the targets of CALL_NATIVE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Native {
    /// debug.c: with the boot-time debug page (DEBUG_PAGE_OBJECTINFO) it
    /// prints nothing.
    TryPrintDebugMarioLevelInfo,
    BhvMarioUpdate,
    /// debug.c: spawns only on the stage-info debug page with debug spawning
    /// enabled, which nothing in the game enables.
    TryDoMarioDebugObjectSpawn,
    BhvYellowCoinInit,
    BhvYellowCoinLoop,
    BhvCoinFormationInit,
    BhvCoinFormationLoop,
    BhvCoinFormationSpawnLoop,
    BhvCoinSparklesLoop,
    BhvGoldenCoinSparklesLoop,
}

impl Native {
    pub const ALL: [Native; 10] = [
        Native::TryPrintDebugMarioLevelInfo,
        Native::BhvMarioUpdate,
        Native::TryDoMarioDebugObjectSpawn,
        Native::BhvYellowCoinInit,
        Native::BhvYellowCoinLoop,
        Native::BhvCoinFormationInit,
        Native::BhvCoinFormationLoop,
        Native::BhvCoinFormationSpawnLoop,
        Native::BhvCoinSparklesLoop,
        Native::BhvGoldenCoinSparklesLoop,
    ];

    /// The pinned decomp function name.
    pub fn name(self) -> &'static str {
        match self {
            Native::TryPrintDebugMarioLevelInfo => "try_print_debug_mario_level_info",
            Native::BhvMarioUpdate => "bhv_mario_update",
            Native::TryDoMarioDebugObjectSpawn => "try_do_mario_debug_object_spawn",
            Native::BhvYellowCoinInit => "bhv_yellow_coin_init",
            Native::BhvYellowCoinLoop => "bhv_yellow_coin_loop",
            Native::BhvCoinFormationInit => "bhv_coin_formation_init",
            Native::BhvCoinFormationLoop => "bhv_coin_formation_loop",
            Native::BhvCoinFormationSpawnLoop => "bhv_coin_formation_spawn_loop",
            Native::BhvCoinSparklesLoop => "bhv_coin_sparkles_loop",
            Native::BhvGoldenCoinSparklesLoop => "bhv_golden_coin_sparkles_loop",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|n| n.name() == name)
    }

    /// The behaviors the function can spawn, for [`BehaviorScripts::check`].
    pub fn spawns(self) -> &'static [Behavior] {
        match self {
            Native::BhvYellowCoinLoop | Native::BhvCoinFormationSpawnLoop => {
                &[Behavior::GoldenCoinSparkles]
            }
            Native::BhvCoinFormationLoop => &[Behavior::CoinFormationSpawn],
            Native::BhvGoldenCoinSparklesLoop => &[Behavior::CoinSparkles],
            _ => &[],
        }
    }
}

/// Why a script cannot run on this port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unported {
    /// A CALL_NATIVE target with no Rust translation.
    Native { command: u32, function: u32 },
    /// A command whose system is not ported (object animations, object
    /// collision models, water droplets) or an invalid opcode.
    Command { command: u32, opcode: u8 },
    /// An OR_INT that sets object flags whose cur_obj_update handling needs
    /// object transforms.
    Flags { command: u32, flags: u32 },
    /// An address outside the loaded behavior segment.
    Address(u32),
}

/// Object flags cur_obj_update handles with transforms the port lacks.
pub const UNPORTED_OBJ_FLAGS: u32 =
    OBJ_FLAG_TRANSFORM_RELATIVE_TO_PARENT | OBJ_FLAG_SET_THROW_MATRIX_FROM_TRANSFORM;

/// The words of each behavior command, by opcode (data/behavior_data.c's
/// macros); None for opcodes outside BehaviorCmdTable.
pub fn command_words(opcode: u8) -> Option<u32> {
    Some(match opcode {
        0x02
        | 0x04
        | 0x0C
        | 0x13..=0x17
        | 0x23
        | 0x27
        | 0x2A
        | 0x2E
        | 0x2F
        | 0x31
        | 0x33
        | 0x36
        | 0x37 => 2,
        0x1C | 0x29 | 0x2B | 0x2C => 3,
        0x30 => 5,
        0x00..=0x37 => 1,
        _ => return None,
    })
}

/// The loaded behavior segment with the natives and behaviors it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorScripts {
    words: Vec<u32>,
    natives: BTreeMap<u32, Native>,
    behaviors: BTreeMap<Behavior, u32>,
}

/// A world without behavior scripts, for code that never updates objects.
pub static NO_SCRIPTS: BehaviorScripts = BehaviorScripts {
    words: Vec::new(),
    natives: BTreeMap::new(),
    behaviors: BTreeMap::new(),
};

impl BehaviorScripts {
    /// The segment's big-endian bytes, with every native's address and every
    /// named behavior's segmented address.
    pub fn new(
        bytes: &[u8],
        natives: impl IntoIterator<Item = (Native, u32)>,
        behaviors: impl IntoIterator<Item = (Behavior, u32)>,
    ) -> Result<Self, String> {
        if !bytes.len().is_multiple_of(4) || bytes.len() > 0x100_0000 {
            return Err(format!("behavior segment of {} bytes", bytes.len()));
        }
        let words = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_be_bytes(*w))
            .collect();
        let mut scripts = Self {
            words,
            natives: BTreeMap::new(),
            behaviors: BTreeMap::new(),
        };
        for (native, address) in natives {
            if scripts.natives.insert(address, native).is_some() {
                return Err(format!("two natives at 0x{address:08X}"));
            }
        }
        for native in Native::ALL {
            if !scripts.natives.values().any(|n| *n == native) {
                return Err(format!("no address for {}", native.name()));
            }
        }
        for (behavior, address) in behaviors {
            scripts.index(address).ok_or(format!(
                "{} at 0x{address:08X} is outside the segment",
                behavior.name()
            ))?;
            scripts.behaviors.insert(behavior, address);
        }
        for behavior in Behavior::ALL {
            if !scripts.behaviors.contains_key(&behavior) {
                return Err(format!("no address for {}", behavior.name()));
            }
        }
        Ok(scripts)
    }

    fn index(&self, address: u32) -> Option<usize> {
        let offset = address.checked_sub(u32::from(SCRIPT_SEGMENT) << 24)?;
        (offset % 4 == 0 && (offset / 4) < self.words.len() as u32).then_some(offset as usize / 4)
    }

    /// The word at a segmented address.
    pub fn word(&self, address: u32) -> u32 {
        let index = self
            .index(address)
            .unwrap_or_else(|| panic!("behavior address 0x{address:08X} is outside the segment"));
        self.words[index]
    }

    /// A named behavior's segmented address.
    pub fn address(&self, behavior: Behavior) -> u32 {
        *self
            .behaviors
            .get(&behavior)
            .unwrap_or_else(|| panic!("{} is not loaded", behavior.name()))
    }

    /// The named behavior at a script address, if any.
    pub fn behavior_at(&self, address: u32) -> Option<Behavior> {
        self.behaviors
            .iter()
            .find(|(_, a)| **a == address)
            .map(|(b, _)| *b)
    }

    pub fn native(&self, address: u32) -> Option<Native> {
        self.natives.get(&address).copied()
    }

    /// Whether `script` and everything it can reach run on this port: every
    /// command path (following CALL and GOTO), the scripts its commands and
    /// natives spawn, and every native.
    pub fn check(&self, script: u32) -> Result<(), Unported> {
        let mut scripts = vec![script];
        let mut seen_scripts = BTreeSet::new();
        let mut seen = BTreeSet::new();
        while let Some(start) = scripts.pop() {
            if !seen_scripts.insert(start) {
                continue;
            }
            let mut paths = vec![start];
            while let Some(mut at) = paths.pop() {
                loop {
                    if !seen.insert(at) {
                        break;
                    }
                    let word = self
                        .index(at)
                        .map(|i| self.words[i])
                        .ok_or(Unported::Address(at))?;
                    let opcode = (word >> 24) as u8;
                    let words = command_words(opcode).ok_or(Unported::Command {
                        command: at,
                        opcode,
                    })?;
                    for i in 1..words {
                        self.index(at + 4 * i)
                            .ok_or(Unported::Address(at + 4 * i))?;
                    }
                    let arg = |i: u32| self.words[self.index(at + 4 * i).unwrap()];
                    match opcode {
                        // CALL continues after the RETURN.
                        0x02 => paths.push(arg(1)),
                        0x04 => {
                            paths.push(arg(1));
                            break;
                        }
                        0x03 | 0x09 | 0x0A | 0x0B | 0x1D => break,
                        0x0C => {
                            let function = arg(1);
                            let native = self.native(function).ok_or(Unported::Native {
                                command: at,
                                function,
                            })?;
                            scripts.extend(native.spawns().iter().map(|b| self.address(*b)));
                        }
                        0x1C | 0x29 | 0x2C => scripts.push(arg(2)),
                        0x11 if ((word >> 16) & 0xFF) as usize == O_FLAGS => {
                            let flags = word & 0xFFFF;
                            if flags & UNPORTED_OBJ_FLAGS != 0 {
                                return Err(Unported::Flags { command: at, flags });
                            }
                        }
                        // Object animations, collision models and water droplets.
                        0x27 | 0x28 | 0x2A | 0x37 => {
                            return Err(Unported::Command {
                                command: at,
                                opcode,
                            });
                        }
                        _ => {}
                    }
                    at += 4 * words;
                }
            }
        }
        Ok(())
    }
}

/// An authored encoder for behavior commands, for ROM-free fixtures: each
/// method appends one command as data/behavior_data.c's macro encodes it.
#[derive(Debug, Default, Clone)]
pub struct ScriptBuilder {
    pub words: Vec<u32>,
}

impl ScriptBuilder {
    /// The segmented address the next word will have.
    pub fn here(&self) -> u32 {
        (u32::from(SCRIPT_SEGMENT) << 24) + 4 * self.words.len() as u32
    }
    fn bb(&mut self, opcode: u8, byte: u8) -> &mut Self {
        self.words
            .push((u32::from(opcode) << 24) | (u32::from(byte) << 16));
        self
    }
    fn bbh(&mut self, opcode: u8, byte: u8, half: i16) -> &mut Self {
        self.words
            .push((u32::from(opcode) << 24) | (u32::from(byte) << 16) | u32::from(half as u16));
        self
    }
    pub fn begin(&mut self, list: u8) -> &mut Self {
        self.bb(0x00, list)
    }
    pub fn delay(&mut self, frames: i16) -> &mut Self {
        self.bbh(0x01, 0, frames)
    }
    pub fn call(&mut self, address: u32) -> &mut Self {
        self.words.extend([0x0200_0000, address]);
        self
    }
    pub fn ret(&mut self) -> &mut Self {
        self.bb(0x03, 0)
    }
    pub fn goto(&mut self, address: u32) -> &mut Self {
        self.words.extend([0x0400_0000, address]);
        self
    }
    pub fn begin_repeat(&mut self, count: i16) -> &mut Self {
        self.bbh(0x05, 0, count)
    }
    pub fn end_repeat(&mut self) -> &mut Self {
        self.bb(0x06, 0)
    }
    pub fn end_repeat_continue(&mut self) -> &mut Self {
        self.bb(0x07, 0)
    }
    pub fn begin_loop(&mut self) -> &mut Self {
        self.bb(0x08, 0)
    }
    pub fn end_loop(&mut self) -> &mut Self {
        self.bb(0x09, 0)
    }
    pub fn brk(&mut self) -> &mut Self {
        self.bb(0x0A, 0)
    }
    pub fn call_native(&mut self, address: u32) -> &mut Self {
        self.words.extend([0x0C00_0000, address]);
        self
    }
    pub fn add_float(&mut self, field: usize, value: i16) -> &mut Self {
        self.bbh(0x0D, field as u8, value)
    }
    pub fn set_float(&mut self, field: usize, value: i16) -> &mut Self {
        self.bbh(0x0E, field as u8, value)
    }
    pub fn add_int(&mut self, field: usize, value: i16) -> &mut Self {
        self.bbh(0x0F, field as u8, value)
    }
    pub fn set_int(&mut self, field: usize, value: i16) -> &mut Self {
        self.bbh(0x10, field as u8, value)
    }
    pub fn or_int(&mut self, field: usize, value: u32) -> &mut Self {
        self.bbh(0x11, field as u8, value as u16 as i16)
    }
    pub fn bit_clear(&mut self, field: usize, value: u32) -> &mut Self {
        self.bbh(0x12, field as u8, value as u16 as i16)
    }
    /// SET_INT_RAND_RSHIFT (0x13), SET_RANDOM_FLOAT (0x14), SET_RANDOM_INT
    /// (0x15), ADD_RANDOM_FLOAT (0x16) and ADD_INT_RAND_RSHIFT (0x17).
    pub fn random(&mut self, opcode: u8, field: usize, min: i16, range: i16) -> &mut Self {
        assert!((0x13..=0x17).contains(&opcode));
        self.bbh(opcode, field as u8, min);
        self.words.push(u32::from(range as u16) << 16);
        self
    }
    pub fn set_model(&mut self, model: i16) -> &mut Self {
        self.bbh(0x1B, 0, model)
    }
    pub fn spawn_child(&mut self, model: u32, behavior: u32) -> &mut Self {
        self.words.extend([0x1C00_0000, model, behavior]);
        self
    }
    pub fn deactivate(&mut self) -> &mut Self {
        self.bb(0x1D, 0)
    }
    pub fn drop_to_floor(&mut self) -> &mut Self {
        self.bb(0x1E, 0)
    }
    pub fn sum_float(&mut self, dst: usize, a: usize, b: usize) -> &mut Self {
        self.words
            .push(0x1F00_0000 | ((dst as u32) << 16) | ((a as u32) << 8) | b as u32);
        self
    }
    pub fn sum_int(&mut self, dst: usize, a: usize, b: usize) -> &mut Self {
        self.words
            .push(0x2000_0000 | ((dst as u32) << 16) | ((a as u32) << 8) | b as u32);
        self
    }
    pub fn billboard(&mut self) -> &mut Self {
        self.bb(0x21, 0)
    }
    pub fn hide(&mut self) -> &mut Self {
        self.bb(0x22, 0)
    }
    pub fn set_hitbox(&mut self, radius: i16, height: i16) -> &mut Self {
        self.words.extend([
            0x2300_0000,
            (u32::from(radius as u16) << 16) | u32::from(height as u16),
        ]);
        self
    }
    pub fn delay_var(&mut self, field: usize) -> &mut Self {
        self.bb(0x25, field as u8)
    }
    pub fn begin_repeat_unused(&mut self, count: u8) -> &mut Self {
        self.bb(0x26, count)
    }
    pub fn spawn_child_with_param(&mut self, param: i16, model: u32, behavior: u32) -> &mut Self {
        self.bbh(0x29, 0, param);
        self.words.extend([model, behavior]);
        self
    }
    pub fn set_hitbox_with_offset(&mut self, radius: i16, height: i16, down: i16) -> &mut Self {
        self.words.extend([
            0x2B00_0000,
            (u32::from(radius as u16) << 16) | u32::from(height as u16),
            u32::from(down as u16) << 16,
        ]);
        self
    }
    pub fn spawn_obj(&mut self, model: u32, behavior: u32) -> &mut Self {
        self.words.extend([0x2C00_0000, model, behavior]);
        self
    }
    pub fn set_home(&mut self) -> &mut Self {
        self.bb(0x2D, 0)
    }
    pub fn set_hurtbox(&mut self, radius: i16, height: i16) -> &mut Self {
        self.words.extend([
            0x2E00_0000,
            (u32::from(radius as u16) << 16) | u32::from(height as u16),
        ]);
        self
    }
    pub fn set_interact_type(&mut self, interact: u32) -> &mut Self {
        self.words.extend([0x2F00_0000, interact]);
        self
    }
    pub fn set_obj_physics(&mut self, values: [i16; 8]) -> &mut Self {
        self.words.push(0x3000_0000);
        for pair in values.as_chunks::<2>().0 {
            self.words
                .push((u32::from(pair[0] as u16) << 16) | u32::from(pair[1] as u16));
        }
        self
    }
    pub fn set_interact_subtype(&mut self, subtype: u32) -> &mut Self {
        self.words.extend([0x3100_0000, subtype]);
        self
    }
    pub fn scale(&mut self, percent: i16) -> &mut Self {
        self.bbh(0x32, 0, percent)
    }
    pub fn parent_bit_clear(&mut self, field: usize, flags: u32) -> &mut Self {
        self.bb(0x33, field as u8);
        self.words.push(flags);
        self
    }
    pub fn animate_texture(&mut self, field: usize, rate: i16) -> &mut Self {
        self.bbh(0x34, field as u8, rate)
    }
    pub fn disable_rendering(&mut self) -> &mut Self {
        self.bb(0x35, 0)
    }
    pub fn set_int_unused(&mut self, field: usize, value: i16) -> &mut Self {
        self.bb(0x36, field as u8);
        self.words.push(u32::from(value as u16));
        self
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }
}

/// An authored segment for ROM-free tests: the pinned data/behavior_data.c
/// scripts of every [`Behavior`] the port runs, encoded with
/// [`ScriptBuilder`] at authored addresses, and authored native addresses.
/// The three behaviors bhv_cmd_begin compares against are BEGIN/BREAK
/// stand-ins (they are only compared by address). Not ROM content.
pub fn authored_scripts() -> BehaviorScripts {
    let native = |n: Native| {
        AUTHORED_NATIVE_BASE + 4 * Native::ALL.iter().position(|x| *x == n).unwrap() as u32
    };
    let mut b = ScriptBuilder::default();
    let mut behaviors = vec![];
    behaviors.push((Behavior::CoinFormationSpawn, b.here()));
    b.begin(OBJ_LIST_LEVEL as u8)
        .or_int(O_FLAGS, OBJ_FLAG_UPDATE_GFX_POS_AND_ANGLE)
        .billboard()
        .begin_loop()
        .call_native(native(Native::BhvCoinFormationSpawnLoop))
        .end_loop();
    behaviors.push((Behavior::CoinFormation, b.here()));
    b.begin(OBJ_LIST_SPAWNER as u8)
        .or_int(
            O_FLAGS,
            OBJ_FLAG_COMPUTE_DIST_TO_MARIO | OBJ_FLAG_UPDATE_GFX_POS_AND_ANGLE,
        )
        .call_native(native(Native::BhvCoinFormationInit))
        .begin_loop()
        .call_native(native(Native::BhvCoinFormationLoop))
        .end_loop();
    behaviors.push((Behavior::YellowCoin, b.here()));
    b.begin(OBJ_LIST_LEVEL as u8)
        .billboard()
        .or_int(
            O_FLAGS,
            OBJ_FLAG_COMPUTE_DIST_TO_MARIO | OBJ_FLAG_UPDATE_GFX_POS_AND_ANGLE,
        )
        .call_native(native(Native::BhvYellowCoinInit))
        .begin_loop()
        .call_native(native(Native::BhvYellowCoinLoop))
        .end_loop();
    behaviors.push((Behavior::CoinSparkles, b.here()));
    b.begin(OBJ_LIST_DEFAULT as u8)
        .or_int(O_FLAGS, OBJ_FLAG_UPDATE_GFX_POS_AND_ANGLE)
        .billboard()
        .set_float(O_GRAPH_Y_OFFSET, 25)
        .set_int(O_ANIM_STATE, -1)
        .begin_repeat(8)
        .add_int(O_ANIM_STATE, 1)
        .end_repeat()
        .begin_repeat(2)
        .call_native(native(Native::BhvCoinSparklesLoop))
        .end_repeat()
        .deactivate();
    behaviors.push((Behavior::GoldenCoinSparkles, b.here()));
    b.begin(OBJ_LIST_DEFAULT as u8)
        .or_int(O_FLAGS, OBJ_FLAG_UPDATE_GFX_POS_AND_ANGLE)
        .disable_rendering()
        .begin_repeat(3)
        .call_native(native(Native::BhvGoldenCoinSparklesLoop))
        .end_repeat()
        .deactivate();
    behaviors.push((Behavior::Mario, b.here()));
    b.begin(OBJ_LIST_PLAYER as u8)
        .set_int(O_INTANGIBLE_TIMER, 0)
        .or_int(O_FLAGS, OBJ_FLAG_0100)
        .or_int(O_UNK94, 0x0001)
        .set_hitbox(37, 160)
        .begin_loop()
        .call_native(native(Native::TryPrintDebugMarioLevelInfo))
        .call_native(native(Native::BhvMarioUpdate))
        .call_native(native(Native::TryDoMarioDebugObjectSpawn))
        .end_loop();
    behaviors.push((Behavior::SpinAirborneWarp, b.here()));
    b.brk();
    for stand_in in [
        Behavior::MessagePanel,
        Behavior::HauntedChair,
        Behavior::MadPiano,
    ] {
        behaviors.push((stand_in, b.here()));
        b.begin(OBJ_LIST_DEFAULT as u8).brk();
    }
    let natives = Native::ALL.into_iter().map(|n| (n, native(n)));
    BehaviorScripts::new(&b.bytes(), natives, behaviors).expect("the authored segment is complete")
}

/// Where [`authored_scripts`] places its natives (outside the ROM's code).
pub const AUTHORED_NATIVE_BASE: u32 = 0x80F0_0000;

const CONTINUE: bool = true;
const BREAK: bool = false;

/// cur_obj_update for `id` (gCurrentObject): the distance and angle to
/// Mario, the action-timer resets, the script until a command breaks, the
/// timer, the flag-driven movement and the visibility rules.
pub fn cur_obj_update(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    w.objects.current = Some(id);
    let obj_flags = flags16(object(&w.objects, &m.obj, id).raw.u32(O_FLAGS));
    let mario = w
        .objects
        .mario
        .expect("objects update with gMarioObject NULL");
    let mut distance_from_mario = 0.0;
    if obj_flags & OBJ_FLAG_COMPUTE_DIST_TO_MARIO as i32 != 0 {
        let d = helpers::dist_between_objects(
            object(&w.objects, &m.obj, id),
            object(&w.objects, &m.obj, mario),
        );
        object_mut(&mut w.objects, &mut m.obj, id)
            .raw
            .set_f32(O_DISTANCE_TO_MARIO, d);
        distance_from_mario = d;
    }
    if obj_flags & OBJ_FLAG_COMPUTE_ANGLE_TO_MARIO as i32 != 0 {
        let angle = helpers::obj_angle_to_object(
            w.trig,
            object(&w.objects, &m.obj, id),
            object(&w.objects, &m.obj, mario),
        );
        object_mut(&mut w.objects, &mut m.obj, id)
            .raw
            .set_s32(O_ANGLE_TO_MARIO, i32::from(angle));
    }
    reset_timer_on_action_change(object_mut(&mut w.objects, &mut m.obj, id));

    let mut command = object(&w.objects, &m.obj, id).cur_bhv_command;
    while run_command(m, w, id, &mut command) == CONTINUE {}

    let o = object_mut(&mut w.objects, &mut m.obj, id);
    o.cur_bhv_command = command;
    let timer = o.raw.s32(O_TIMER);
    if timer < 0x3FFF_FFFF {
        o.raw.set_s32(O_TIMER, timer + 1);
    }
    reset_timer_on_action_change(o);

    let obj_flags = flags16(o.raw.u32(O_FLAGS));
    assert!(
        obj_flags & UNPORTED_OBJ_FLAGS as i32 == 0,
        "object flags {obj_flags:#X} need object transforms, which are not ported"
    );
    if obj_flags & OBJ_FLAG_SET_FACE_ANGLE_TO_MOVE_ANGLE as i32 != 0 {
        for (face, moving) in [
            (O_FACE_ANGLE_PITCH, O_MOVE_ANGLE_PITCH),
            (O_FACE_ANGLE_YAW, O_MOVE_ANGLE_YAW),
            (O_FACE_ANGLE_ROLL, O_MOVE_ANGLE_ROLL),
        ] {
            o.raw.set_s32(face, o.raw.s32(moving));
        }
    }
    if obj_flags & OBJ_FLAG_SET_FACE_YAW_TO_MOVE_YAW as i32 != 0 {
        o.raw.set_s32(O_FACE_ANGLE_YAW, o.raw.s32(O_MOVE_ANGLE_YAW));
    }
    if obj_flags & OBJ_FLAG_MOVE_XZ_USING_FVEL as i32 != 0 {
        helpers::cur_obj_move_xz_using_fvel_and_yaw(o, w.trig);
    }
    if obj_flags & OBJ_FLAG_MOVE_Y_WITH_TERMINAL_VEL as i32 != 0 {
        helpers::cur_obj_move_y_with_terminal_vel(o);
    }
    if obj_flags & OBJ_FLAG_UPDATE_GFX_POS_AND_ANGLE as i32 != 0 {
        helpers::obj_update_gfx_pos_and_angle(o);
    }
    if o.raw.s32(O_ROOM) != -1 {
        helpers::cur_obj_enable_rendering_if_mario_in_room(o);
    } else if obj_flags & OBJ_FLAG_COMPUTE_DIST_TO_MARIO as i32 != 0
        && o.collision_data.is_none()
        && obj_flags & OBJ_FLAG_ACTIVE_FROM_AFAR as i32 == 0
    {
        if distance_from_mario > o.raw.f32(O_DRAWING_DISTANCE) {
            o.gfx.node_flags &= !GRAPH_RENDER_ACTIVE;
            o.active_flags |= ACTIVE_FLAG_FAR_AWAY;
        } else if o.raw.s32(O_HELD_STATE) == HELD_FREE {
            o.gfx.node_flags |= GRAPH_RENDER_ACTIVE;
            o.active_flags &= !ACTIVE_FLAG_FAR_AWAY;
        }
    }
}

/// `s16 objFlags = o->oFlags`, promoted for the `&` with an int flag.
fn flags16(flags: u32) -> i32 {
    i32::from(flags as i16)
}

fn reset_timer_on_action_change(o: &mut super::Object) {
    let action = o.raw.s32(O_ACTION);
    if action != o.raw.s32(O_PREV_ACTION) {
        o.raw.set_s32(O_TIMER, 0);
        o.raw.set_s32(O_SUB_ACTION, 0);
        o.raw.set_s32(O_PREV_ACTION, action);
    }
}

fn push(o: &mut super::Object, value: u32) {
    let index = o.bhv_stack_index as usize;
    assert!(index < 8, "behavior stack overflow corrupts the object");
    o.bhv_stack[index] = value;
    o.bhv_stack_index += 1;
}

fn pop(o: &mut super::Object) -> u32 {
    assert!(o.bhv_stack_index > 0, "behavior stack underflow");
    o.bhv_stack_index -= 1;
    o.bhv_stack[o.bhv_stack_index as usize]
}

/// One command of BehaviorCmdTable at `*cmd` (gCurBhvCommand).
fn run_command(m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId, cmd: &mut u32) -> bool {
    let scripts = w.behaviors;
    let at = *cmd;
    let word = scripts.word(at);
    let arg = |i: u32| scripts.word(at + 4 * i);
    let opcode = (word >> 24) as u8;
    let byte2 = ((word >> 16) & 0xFF) as usize;
    let s16_2 = word as i16;
    match opcode {
        // BEGIN: three behaviors initialize here.
        0x00 => {
            let behavior = object(&w.objects, &m.obj, id).behavior;
            if behavior == scripts.address(Behavior::HauntedChair)
                || behavior == scripts.address(Behavior::MadPiano)
            {
                let level_num = w.level_num;
                helpers::bhv_init_room(object_mut(&mut w.objects, &mut m.obj, id), level_num);
            }
            if behavior == scripts.address(Behavior::MessagePanel) {
                object_mut(&mut w.objects, &mut m.obj, id)
                    .raw
                    .set_f32(O_COLLISION_DISTANCE, 150.0);
            }
            *cmd += 4;
            CONTINUE
        }
        // DELAY.
        0x01 => {
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            if i32::from(o.bhv_delay_timer) < i32::from(s16_2) - 1 {
                o.bhv_delay_timer += 1;
            } else {
                o.bhv_delay_timer = 0;
                *cmd += 4;
            }
            BREAK
        }
        // CALL.
        0x02 => {
            push(object_mut(&mut w.objects, &mut m.obj, id), at + 8);
            *cmd = arg(1);
            CONTINUE
        }
        // RETURN.
        0x03 => {
            *cmd = pop(object_mut(&mut w.objects, &mut m.obj, id));
            CONTINUE
        }
        // GOTO.
        0x04 => {
            *cmd = arg(1);
            CONTINUE
        }
        // BEGIN_REPEAT, BEGIN_REPEAT_UNUSED.
        0x05 | 0x26 => {
            let count = if opcode == 0x05 {
                i32::from(s16_2)
            } else {
                byte2 as i32
            };
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            push(o, at + 4);
            push(o, count as u32);
            *cmd += 4;
            CONTINUE
        }
        // END_REPEAT, END_REPEAT_CONTINUE.
        0x06 | 0x07 => {
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            let count = pop(o).wrapping_sub(1);
            if count != 0 {
                *cmd = pop(o);
                push(o, *cmd);
                push(o, count);
            } else {
                pop(o);
                *cmd += 4;
            }
            if opcode == 0x06 { BREAK } else { CONTINUE }
        }
        // BEGIN_LOOP.
        0x08 => {
            push(object_mut(&mut w.objects, &mut m.obj, id), at + 4);
            *cmd += 4;
            CONTINUE
        }
        // END_LOOP.
        0x09 => {
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            *cmd = pop(o);
            push(o, *cmd);
            BREAK
        }
        // BREAK, BREAK_UNUSED.
        0x0A | 0x0B => BREAK,
        // CALL_NATIVE.
        0x0C => {
            let function = arg(1);
            let native = scripts.native(function).unwrap_or_else(|| {
                panic!("unported native 0x{function:08X} (scripts are checked at spawn)")
            });
            call_native(native, m, w, id);
            *cmd += 8;
            CONTINUE
        }
        // ADD_FLOAT, SET_FLOAT, ADD_INT, SET_INT, OR_INT, BIT_CLEAR.
        0x0D..=0x12 => {
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            let raw = &mut o.raw;
            match opcode {
                0x0D => raw.set_f32(byte2, raw.f32(byte2) + f32::from(s16_2)),
                0x0E => raw.set_f32(byte2, f32::from(s16_2)),
                0x0F => raw.set_s32(byte2, raw.s32(byte2).wrapping_add(i32::from(s16_2))),
                0x10 => raw.set_s32(byte2, i32::from(s16_2)),
                0x11 => raw.set_s32(byte2, raw.s32(byte2) | (i32::from(s16_2) & 0xFFFF)),
                _ => raw.set_s32(
                    byte2,
                    raw.s32(byte2) & ((i32::from(s16_2) & 0xFFFF) ^ 0xFFFF),
                ),
            }
            *cmd += 4;
            CONTINUE
        }
        // SET_INT_RAND_RSHIFT.
        0x13 => {
            let (min, rshift) = (i32::from(s16_2), i32::from((arg(1) >> 16) as i16));
            let value = (i32::from(w.rng.random_u16()) >> rshift).wrapping_add(min);
            object_mut(&mut w.objects, &mut m.obj, id)
                .raw
                .set_s32(byte2, value);
            *cmd += 8;
            CONTINUE
        }
        // SET_RANDOM_FLOAT.
        0x14 => {
            let (min, range) = (f32::from(s16_2), f32::from((arg(1) >> 16) as i16));
            let value = range * w.rng.random_float() + min;
            object_mut(&mut w.objects, &mut m.obj, id)
                .raw
                .set_f32(byte2, value);
            *cmd += 8;
            CONTINUE
        }
        // SET_RANDOM_INT.
        0x15 => {
            let (min, range) = (i32::from(s16_2), i32::from((arg(1) >> 16) as i16));
            let value = f32_to_s32(range as f32 * w.rng.random_float()).wrapping_add(min);
            object_mut(&mut w.objects, &mut m.obj, id)
                .raw
                .set_s32(byte2, value);
            *cmd += 8;
            CONTINUE
        }
        // ADD_RANDOM_FLOAT.
        0x16 => {
            let (min, range) = (f32::from(s16_2), f32::from((arg(1) >> 16) as i16));
            let random = w.rng.random_float();
            let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
            raw.set_f32(byte2, raw.f32(byte2) + min + range * random);
            *cmd += 8;
            CONTINUE
        }
        // ADD_INT_RAND_RSHIFT.
        0x17 => {
            let (min, rshift) = (i32::from(s16_2), i32::from((arg(1) >> 16) as i16));
            let rnd = i32::from(w.rng.random_u16());
            let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
            raw.set_s32(
                byte2,
                raw.s32(byte2).wrapping_add(min).wrapping_add(rnd >> rshift),
            );
            *cmd += 8;
            CONTINUE
        }
        // CMD_NOP_1..3, CMD_NOP_4.
        0x18..=0x1A | 0x24 => {
            *cmd += 4;
            CONTINUE
        }
        // SET_MODEL.
        0x1B => {
            let model = i32::from(s16_2);
            let child = w.loaded_model(model);
            object_mut(&mut w.objects, &mut m.obj, id).gfx.shared_child = child;
            *cmd += 4;
            CONTINUE
        }
        // SPAWN_CHILD, SPAWN_OBJ, SPAWN_CHILD_WITH_PARAM.
        0x1C | 0x2C | 0x29 => {
            let (model, behavior) = (arg(1), arg(2));
            let child = super::spawn::spawn_object_at_origin(m, w, id, model as i32, behavior);
            let parent = object(&w.objects, &m.obj, id).clone();
            let c = object_mut(&mut w.objects, &mut m.obj, child);
            helpers::obj_copy_pos_and_angle(c, &parent);
            if opcode == 0x29 {
                c.raw.set_s32(O_BHV_PARAMS2ND_BYTE, i32::from(s16_2));
            }
            if opcode == 0x2C {
                object_mut(&mut w.objects, &mut m.obj, id).prev_obj = Some(child);
            }
            *cmd += 12;
            CONTINUE
        }
        // DEACTIVATE.
        0x1D => {
            object_mut(&mut w.objects, &mut m.obj, id).active_flags = ACTIVE_FLAG_DEACTIVATED;
            BREAK
        }
        // DROP_TO_FLOOR.
        0x1E => {
            let pos = object(&w.objects, &m.obj, id).pos();
            let floor = w.find_floor_height(pos[0], pos[1] + 200.0, pos[2]);
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            o.raw.set_f32(O_POS_Y, floor);
            o.raw
                .set_u32(O_MOVE_FLAGS, o.raw.u32(O_MOVE_FLAGS) | OBJ_MOVE_ON_GROUND);
            *cmd += 4;
            CONTINUE
        }
        // SUM_FLOAT, SUM_INT.
        0x1F | 0x20 => {
            let (a, b) = (((word >> 8) & 0xFF) as usize, (word & 0xFF) as usize);
            let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
            if opcode == 0x1F {
                raw.set_f32(byte2, raw.f32(a) + raw.f32(b));
            } else {
                raw.set_s32(byte2, raw.s32(a).wrapping_add(raw.s32(b)));
            }
            *cmd += 4;
            CONTINUE
        }
        // BILLBOARD, HIDE, DISABLE_RENDERING.
        0x21 | 0x22 | 0x35 => {
            let gfx = &mut object_mut(&mut w.objects, &mut m.obj, id).gfx;
            match opcode {
                0x21 => gfx.node_flags |= GRAPH_RENDER_BILLBOARD,
                0x22 => gfx.node_flags |= GRAPH_RENDER_INVISIBLE,
                _ => gfx.node_flags &= !GRAPH_RENDER_ACTIVE,
            }
            *cmd += 4;
            CONTINUE
        }
        // SET_HITBOX, SET_HURTBOX.
        0x23 | 0x2E => {
            let (radius, height) = (f32::from((arg(1) >> 16) as i16), f32::from(arg(1) as i16));
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            if opcode == 0x23 {
                (o.hitbox_radius, o.hitbox_height) = (radius, height);
            } else {
                (o.hurtbox_radius, o.hurtbox_height) = (radius, height);
            }
            *cmd += 8;
            CONTINUE
        }
        // DELAY_VAR.
        0x25 => {
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            let num = o.raw.s32(byte2);
            if i32::from(o.bhv_delay_timer) < num.wrapping_sub(1) {
                o.bhv_delay_timer += 1;
            } else {
                o.bhv_delay_timer = 0;
                *cmd += 4;
            }
            BREAK
        }
        // SET_HITBOX_WITH_OFFSET.
        0x2B => {
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            o.hitbox_radius = f32::from((arg(1) >> 16) as i16);
            o.hitbox_height = f32::from(arg(1) as i16);
            o.hitbox_down_offset = f32::from((arg(2) >> 16) as i16);
            *cmd += 12;
            CONTINUE
        }
        // SET_HOME.
        0x2D => {
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            for (home, pos) in [
                (O_HOME_X, O_POS_X),
                (O_HOME_Y, O_POS_Y),
                (O_HOME_Z, O_POS_Z),
            ] {
                o.raw.set_u32(home, o.raw.u32(pos));
            }
            *cmd += 4;
            CONTINUE
        }
        // SET_INTERACT_TYPE, SET_INTERACT_SUBTYPE.
        0x2F | 0x31 => {
            let field = if opcode == 0x2F {
                O_INTERACT_TYPE
            } else {
                O_INTERACTION_SUBTYPE
            };
            object_mut(&mut w.objects, &mut m.obj, id)
                .raw
                .set_u32(field, arg(1));
            *cmd += 8;
            CONTINUE
        }
        // SET_OBJ_PHYSICS.
        0x30 => {
            let half = |i: u32, high: bool| {
                let w = arg(i);
                if high { (w >> 16) as i16 } else { w as i16 }
            };
            let o = object_mut(&mut w.objects, &mut m.obj, id);
            o.raw
                .set_f32(O_WALL_HITBOX_RADIUS, f32::from(half(1, true)));
            o.raw.set_f32(O_GRAVITY, f32::from(half(1, false)) / 100.0);
            o.raw
                .set_f32(O_BOUNCINESS, f32::from(half(2, true)) / 100.0);
            o.raw
                .set_f32(O_DRAG_STRENGTH, f32::from(half(2, false)) / 100.0);
            o.raw.set_f32(O_FRICTION, f32::from(half(3, true)) / 100.0);
            o.raw.set_f32(O_BUOYANCY, f32::from(half(3, false)) / 100.0);
            *cmd += 20;
            CONTINUE
        }
        // SCALE.
        0x32 => {
            let scale = f32::from(s16_2) / 100.0;
            object_mut(&mut w.objects, &mut m.obj, id).gfx.scale = [scale; 3];
            *cmd += 4;
            CONTINUE
        }
        // PARENT_BIT_CLEAR.
        0x33 => {
            let value = arg(1) ^ 0xFFFF_FFFF;
            let parent = object(&w.objects, &m.obj, id)
                .parent
                .expect("parentObj is NULL");
            let raw = &mut object_mut(&mut w.objects, &mut m.obj, parent).raw;
            raw.set_u32(byte2, raw.u32(byte2) & value);
            *cmd += 8;
            CONTINUE
        }
        // ANIMATE_TEXTURE.
        0x34 => {
            let rate = i32::from(s16_2) as u32;
            assert!(rate != 0, "ANIMATE_TEXTURE with rate 0 divides by zero");
            if w.global_timer.is_multiple_of(rate) {
                let raw = &mut object_mut(&mut w.objects, &mut m.obj, id).raw;
                raw.set_s32(byte2, raw.s32(byte2).wrapping_add(1));
            }
            *cmd += 4;
            CONTINUE
        }
        // SET_INT_UNUSED.
        0x36 => {
            object_mut(&mut w.objects, &mut m.obj, id)
                .raw
                .set_s32(byte2, i32::from(arg(1) as i16));
            *cmd += 8;
            CONTINUE
        }
        _ => panic!(
            "behavior command 0x{opcode:02X} at 0x{at:08X} is not ported (scripts are checked at spawn)"
        ),
    }
}

/// CALL_NATIVE's dispatch to the Rust translations.
fn call_native(native: Native, m: &mut MarioState, w: &mut StepWorld<'_>, id: ObjectId) {
    use super::coin;
    match native {
        Native::TryPrintDebugMarioLevelInfo | Native::TryDoMarioDebugObjectSpawn => {}
        Native::BhvMarioUpdate => {
            assert!(
                w.objects.mario == Some(id),
                "bhv_mario_update runs only on gMarioObject"
            );
            crate::simulation::mario::tick::bhv_mario_update(m, w);
        }
        Native::BhvYellowCoinInit => coin::bhv_yellow_coin_init(m, w, id),
        Native::BhvYellowCoinLoop => coin::bhv_yellow_coin_loop(m, w, id),
        Native::BhvCoinFormationInit => coin::bhv_coin_formation_init(m, w, id),
        Native::BhvCoinFormationLoop => coin::bhv_coin_formation_loop(m, w, id),
        Native::BhvCoinFormationSpawnLoop => coin::bhv_coin_formation_spawn_loop(m, w, id),
        Native::BhvCoinSparklesLoop => coin::bhv_coin_sparkles_loop(m, w, id),
        Native::BhvGoldenCoinSparklesLoop => coin::bhv_golden_coin_sparkles_loop(m, w, id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scripts(builder: &ScriptBuilder) -> BehaviorScripts {
        let natives = Native::ALL
            .into_iter()
            .enumerate()
            .map(|(i, n)| (n, 0x8000_0000 + 4 * i as u32));
        let behaviors = Behavior::ALL.into_iter().map(|b| (b, 0x1300_0000));
        BehaviorScripts::new(&builder.bytes(), natives, behaviors).unwrap()
    }

    #[test]
    fn command_lengths_follow_the_macros() {
        let mut b = ScriptBuilder::default();
        b.begin(6)
            .call_native(0x8000_0000)
            .random(0x14, 0x20, 1, 2)
            .spawn_child(0x74, 0x1300_0000)
            .set_hitbox_with_offset(1, 2, 3)
            .set_obj_physics([0; 8])
            .end_loop();
        let mut at = 0;
        let mut opcodes = vec![];
        while at < b.words.len() {
            let opcode = (b.words[at] >> 24) as u8;
            opcodes.push(opcode);
            at += command_words(opcode).unwrap() as usize;
        }
        assert_eq!(at, b.words.len());
        assert_eq!(opcodes, [0x00, 0x0C, 0x14, 0x1C, 0x2B, 0x30, 0x09]);
        assert_eq!(command_words(0x38), None);
    }

    #[test]
    fn the_check_follows_calls_gotos_spawns_and_natives() {
        let mut b = ScriptBuilder::default();
        b.begin(8).begin_loop().call_native(0x8000_0000).end_loop();
        let unknown_native = b.here();
        b.begin(8).call_native(0x8123_4560).brk();
        let animated = b.here();
        b.begin(8).words.extend([0x2800_0000]);
        let flags = b.here();
        b.begin(8)
            .or_int(O_FLAGS, OBJ_FLAG_TRANSFORM_RELATIVE_TO_PARENT)
            .brk();
        let spawner = b.here();
        b.begin(8).spawn_obj(0, unknown_native).brk();
        let through_goto = b.here();
        b.begin(8).goto(animated);
        let s = scripts(&b);
        assert_eq!(s.check(0x1300_0000), Ok(()));
        assert_eq!(
            s.check(unknown_native),
            Err(Unported::Native {
                command: unknown_native + 4,
                function: 0x8123_4560
            })
        );
        assert_eq!(
            s.check(animated),
            Err(Unported::Command {
                command: animated + 4,
                opcode: 0x28
            })
        );
        assert!(matches!(s.check(flags), Err(Unported::Flags { .. })));
        assert!(matches!(s.check(spawner), Err(Unported::Native { .. })));
        assert!(matches!(
            s.check(through_goto),
            Err(Unported::Command { .. })
        ));
        assert_eq!(s.check(0x1400_0000), Err(Unported::Address(0x1400_0000)));
    }
}
