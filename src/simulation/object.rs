//! Object-list setup from pinned CC0 n64decomp/sm64: ObjectList in
//! object_list_processor.h, create_object in spawn_object.c, and
//! sObjectListUpdateOrder in object_list_processor.c. No objects run yet.

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

/// Original frame traversal, without its -1 sentinel. Unused lists are not
/// visited. This is setup data for the future processor, not a tick driver.
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
        assert_eq!(UPDATE_ORDER.map(|list| list as u8), [11, 9, 10, 0, 5, 4, 2, 6, 8, 12]);
        for (index, list) in UPDATE_ORDER.iter().enumerate() {
            assert!(!UPDATE_ORDER[..index].contains(list));
        }
        assert_eq!(ObjectList::try_from(13), Err(13));
        assert_eq!(ObjectList::try_from(u16::MAX), Err(u16::MAX));
    }
}
