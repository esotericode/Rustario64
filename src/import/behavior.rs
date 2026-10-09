//! The first command of an imported behavior script. This only selects its
//! original object list; it does not interpret or execute behavior commands.
use super::{ImportError, Result, reader::Reader};
use crate::simulation::object::ObjectList;

pub fn object_list(bytes: &[u8]) -> Result<ObjectList> {
    let word = Reader::new(bytes, "behavior header").u32(0)?;
    ObjectList::from_behavior_word(word).map_err(|index| {
        ImportError::new("behavior header", 0, format!("invalid object list {index}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_big_endian_and_ignores_later_commands() {
        assert_eq!(object_list(&[0, 6, 0, 0, 0xFF]), Ok(ObjectList::Level));
        assert_eq!(object_list(&[1, 255, 255, 255]), Ok(ObjectList::Default));
        let error = object_list(&[0, 13, 0, 0]).unwrap_err();
        assert_eq!((error.context, error.offset), ("behavior header", 0));
        assert_eq!(error.detail, "invalid object list 13");
    }

    #[test]
    fn truncated_headers_return_import_errors() {
        for length in 0..4 {
            assert!(object_list(&[0, 6, 0, 0][..length]).is_err());
        }
    }
}
