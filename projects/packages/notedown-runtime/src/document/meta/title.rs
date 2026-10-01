use super::*;

#[derive(Debug, Clone)]
pub struct DocumentTitle {
    title: Option<String>,
}

impl Default for DocumentTitle {
    fn default() -> Self {
        Self { title: None }
    }
}

impl NoteDocument {
    #[inline]
    pub fn get_title(&self) -> &Option<String> {
        &self.meta.title.title
    }

    #[inline]
    pub fn set_title(&mut self, title: String) {
        self.meta.title.title = Some(title)
    }
}
