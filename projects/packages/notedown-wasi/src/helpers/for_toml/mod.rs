use crate::exports::notedown::core::types::{NotedownError, Object};

pub fn toml_to_object(input: &str) -> Result<Object, NotedownError> {
    Ok(Object { map: vec![] })
}
