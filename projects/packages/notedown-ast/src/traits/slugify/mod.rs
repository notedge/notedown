use crate::{
    nodes::{Header, NotedownKind, NotedownNode, NotedownNodes},
    traits::Slugify,
};
pub use text_utils::slugify;

impl Slugify for NotedownNodes {
    fn slugify(&self) -> String {
        let mut out = String::new();
        for span in self {
            if !out.is_empty() {
                out.push('-');
            }
            out.push_str(&span.slugify());
        }
        return out;
    }
}

impl Slugify for NotedownNode {
    fn slugify(&self) -> String {
        self.value.slugify()
    }
}

impl Slugify for NotedownKind {
    fn slugify(&self) -> String {
        match self {
            _ => format!("Slugify: {:#?}", self),
        }
    }
}

impl Slugify for Header {
    fn slugify(&self) -> String {
        todo!()
    }
}

impl Slugify for String {
    fn slugify(&self) -> String {
        slugify!(self)
    }
}

impl Slugify for &String {
    fn slugify(&self) -> String {
        slugify!(self)
    }
}

impl Slugify for &str {
    fn slugify(&self) -> String {
        slugify!(self)
    }
}
