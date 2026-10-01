mod parser;
pub use self::parser::{Parser, PluginParser};
use dashmap::DashMap;
use notedown_ast::NotedownNode;
use notedown_error::{QError, Result};
use std::{
    collections::BTreeSet,
    fmt::{Debug, Display, Formatter},
    hash::{Hash, Hasher},
};

/// Plugin system manager
#[derive(Debug, Default)]
pub struct PluginSystem {
    parsers: DashMap<String, PluginParser>,
}

impl PluginSystem {
    /// Register a new parser
    #[inline]
    pub fn register_parser(&self, new: PluginParser) -> Option<PluginParser> {
        self.parsers.insert(new.name.to_owned(), new)
    }
    pub fn get_parser(&self, name: &str) -> Option<Parser> {
        match self.parsers.get(name) {
            None => None,
            Some(s) => Some(s.parser),
        }
    }
    pub fn get_parser_by_extension(&self, e: &str) -> Option<Parser> {
        for parser in &self.parsers {
            if parser.name.contains(e) {
                return Some(parser.parser);
            }
        }
        return None;
    }
}
#[derive(Debug, Clone)]
pub struct ExtendedPackage {}
