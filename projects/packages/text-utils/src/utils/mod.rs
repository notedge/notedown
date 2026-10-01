mod color;
mod emoji;
mod html;
mod table_align;
mod text_align;
mod text_escape;
mod urls;

pub use self::{color::*, emoji::*, html::*, table_align::*, text_align::*, text_escape::*, urls::*};

/// capitalize first letter
pub fn capitalize_first_letter(text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    let mut c = text.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
