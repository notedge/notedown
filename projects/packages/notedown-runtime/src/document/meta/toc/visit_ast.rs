use super::*;
use notedown_ast::{traits::Slugify, NotedownKind, NotedownNode};

impl TocNode {
    fn last_at_level(&mut self, depth: u8) -> &mut TocNode {
        if depth == 0 || self.children.is_empty() { self } else { self.children.last_mut().unwrap().last_at_level(depth - 1) }
    }
}

impl TableOfContent for NotedownNode {
    fn toc_configurable(&self, config: &TocConfig) -> TocNode {
        let mut root = TocNode::default();
        let mut toc_ignore = false;
        if let NotedownKind::Statements(terms) = &self.value {
            for term in terms {
                match &term.value {
                    NotedownKind::Header(header) => {
                        let level = header.level;
                        if toc_ignore {
                            toc_ignore = false;
                            continue;
                        }
                        if level > config.max_depth {
                            continue;
                        }
                        let parent = root.last_at_level(level - 1);
                        let new = TocNode { level, detail: header.slugify(), range: self.range.to_owned().unwrap_or_default(), children: vec![] };
                        parent.children.push(new);
                    }
                    NotedownKind::Command(cmd) => {
                        if cmd.is("toc_ignore") {
                            toc_ignore = true
                        }
                    }
                    _ => (),
                }
            }
        }
        return root;
    }
}
