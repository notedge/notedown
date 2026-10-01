mod datetime;
mod document_class;

pub trait NoteCommand {
    fn apply();
}
