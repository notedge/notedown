use crate::exports::notedown::core::syntax_tree::NotedownRoot;

pub trait ToNotedown
    where
        Self: Sized,
{
    fn into_notedown(self) -> NotedownRoot;
    fn into_notedown_list(self) -> Vec<NotedownRoot> {
        vec![self.into_notedown()]
    }
}
