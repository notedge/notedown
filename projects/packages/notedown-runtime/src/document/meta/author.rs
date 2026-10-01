use super::*;
use std::collections::btree_map::Values;

#[derive(Debug, Clone)]
pub struct DocumentAuthor {
    pub name: String,
    pub email: Option<String>,
    pub org: Option<String>,
}

impl DocumentAuthor {
    /// Constructor of [`DocumentAuthor`]
    #[inline]
    pub fn new(name: impl Into<String>) -> DocumentAuthor {
        Self { name: name.into(), email: None, org: None }
    }
}

/// Methods about [`DocumentAuthor`]
impl NoteDocument {
    #[inline]
    pub fn authors(&self) -> DocumentAuthorIter {
        DocumentAuthorIter { inner: self.meta.authors.values() }
    }
    #[inline]
    pub fn set_authors(&mut self, authors: BTreeMap<String, DocumentAuthor>) {
        self.meta.authors = authors
    }
    #[inline]
    pub fn get_author(&self, name: &str) -> Option<&DocumentAuthor> {
        self.meta.authors.get(name)
    }
    #[inline]
    pub fn get_author_mut(&mut self, name: &str) -> Option<&mut DocumentAuthor> {
        self.meta.authors.get_mut(name)
    }
    #[inline]
    pub fn add_author(&mut self, author: DocumentAuthor) -> Option<DocumentAuthor> {
        self.meta.authors.insert(author.name.to_owned(), author)
    }
}

#[derive(Debug)]
pub struct DocumentAuthorIter<'a> {
    inner: Values<'a, String, DocumentAuthor>,
}

impl<'a> Iterator for DocumentAuthorIter<'a> {
    type Item = &'a DocumentAuthor;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
