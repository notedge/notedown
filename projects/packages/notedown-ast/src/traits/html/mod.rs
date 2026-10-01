use crate::{
    ast::ParagraphTerm,
    hir::{
        BadNode, CodeNode, CommandNode, HeadingLevel, HeadingNode, NotedownHIR, NotedownKind, ParagraphKind, ParagraphNode, TextPlainNode,
        TextStyleNode, UriNode,
    },
    NoteGenerator,
};
use html_ast::HtmlElement;
use notedown_error::{NoteError, Validation};
use std::{
    fmt::{Display, Formatter, Write},
    mem::take,
};

/// generate html compatible with most browsers
pub struct HtmlBuilder {
    /// The root node
    pub root: &'static str,
    /// The style
    pub style: String,
    /// Allow rendering of bare uri
    pub allow_uri: bool,
}

impl Default for HtmlBuilder {
    fn default() -> Self {
        Self { style: "".to_string(), root: "article", allow_uri: true }
    }
}

impl HtmlBuilder {
    /// Use the given name as the root node
    pub fn new(root: &'static str) -> Self {
        Self { root, ..Default::default() }
    }
    /// Use the builtin style
    pub const DEFAULT_STYLE: &'static str = include_str!("style.css");
    /// Use the given style
    pub fn with_style<S: ToString>(self, style: S) -> Self {
        Self { style: style.to_string(), ..self }
    }
}

impl NoteGenerator for HtmlBuilder {
    type Output = HtmlElement;

    fn generate(&mut self, info: &NotedownHIR) -> Validation<Self::Output> {
        let mut errors = vec![];
        match info.as_html(self, &mut errors) {
            Ok(out) => Validation::Success { value: out, diagnostics: errors },
            Err(e) => Validation::Failure { fatal: e, diagnostics: errors },
        }
    }
}

pub(crate) trait AsHtml {
    fn as_html(&self, config: &HtmlBuilder, errors: &mut Vec<NoteError>) -> Result<HtmlElement, NoteError>;
}

impl AsHtml for NotedownHIR {
    fn as_html(&self, config: &HtmlBuilder, errors: &mut Vec<NoteError>) -> Result<HtmlElement, NoteError> {
        let mut out = HtmlElement::new(&config.root);
        for node in &self.node {
            out.add_child(node.as_html(config, errors)?)
        }
        Ok(out)
    }
}

impl AsHtml for NotedownKind {
    fn as_html(&self, config: &HtmlBuilder, errors: &mut Vec<NoteError>) -> Result<HtmlElement, NoteError> {
        match self {
            NotedownKind::Heading(v) => v.as_html(config, errors),
            NotedownKind::Paragraph(v) => v.as_html(config, errors),
            NotedownKind::SyntaxError(v) => v.as_html(config, errors),
        }
    }
}

impl AsHtml for HeadingNode {
    fn as_html(&self, config: &HtmlBuilder, errors: &mut Vec<NoteError>) -> Result<HtmlElement, NoteError> {
        let mut out = HtmlElement::default();
        match self.level {
            HeadingLevel::BookPart => {
                out.set_tag("h1");
                out.add_class("note-book-part")
            }
            HeadingLevel::Chapter => {
                out.set_tag("h1");
                out.add_class("note-chapter")
            }
            HeadingLevel::Section => {
                out.set_tag("h1");
                out.add_class("note-section")
            }
            HeadingLevel::Article => {
                out.set_tag("h1");
                out.add_class("note-article")
            }
            HeadingLevel::Header1 => out.set_tag("h2"),
            HeadingLevel::Header2 => out.set_tag("h3"),
            HeadingLevel::Header3 => out.set_tag("h4"),
            HeadingLevel::Header4 => out.set_tag("h5"),
            HeadingLevel::Header5 => out.set_tag("h6"),
            // HeadingLevel::Header6 => out.set_tag("h6"),
        }
        out.set_id(&self.id);
        config.push_terms(&mut out, &self.terms.terms, errors)?;
        Ok(out)
    }
}

impl HtmlBuilder {
    fn push_terms(&self, out: &mut HtmlElement, terms: &[ParagraphKind], errors: &mut Vec<NoteError>) -> Result<(), NoteError> {
        for term in terms {
            match term {
                ParagraphKind::Plain(v) => out.add_text(&v.text),
                ParagraphKind::Style(v) => out.add_child(v.as_html(self, errors)?),
                ParagraphKind::Space(_) => out.add_child(" "),
                ParagraphKind::Code(v) => out.add_child(v.as_html(self, errors)?),
                ParagraphKind::Command(v) => out.add_child(v.as_html(self, errors)?),
                ParagraphKind::Uri(v) => out.add_child(v.as_html(self, errors)?),
            }
        }
        Ok(())
    }
}

impl AsHtml for ParagraphNode {
    fn as_html(&self, config: &HtmlBuilder, errors: &mut Vec<NoteError>) -> Result<HtmlElement, NoteError> {
        let mut out = HtmlElement::new("p");
        config.push_terms(&mut out, &self.terms, errors)?;
        Ok(out)
    }
}

impl AsHtml for TextStyleNode {
    fn as_html(&self, config: &HtmlBuilder, errors: &mut Vec<NoteError>) -> Result<HtmlElement, NoteError> {
        let mut out = if self.italic {
            HtmlElement::new("i")
        }
        else if self.bold {
            HtmlElement::new("b")
        }
        else if self.underline {
            HtmlElement::new("u")
        }
        else if self.delete {
            HtmlElement::new("del")
        }
        else {
            HtmlElement::new("span")
        };
        config.push_terms(&mut out, &self.text.terms, errors)?;
        Ok(out)
    }
}

impl AsHtml for CodeNode {
    fn as_html(&self, config: &HtmlBuilder, errors: &mut Vec<NoteError>) -> Result<HtmlElement, NoteError> {
        let mut out = HtmlElement::new("code");
        out.add_child(self.code.clone());
        Ok(out)
    }
}

impl AsHtml for CommandNode {
    fn as_html(&self, config: &HtmlBuilder, errors: &mut Vec<NoteError>) -> Result<HtmlElement, NoteError> {
        let mut out = HtmlElement::new(&self.command);
        for arg in &self.arguments {
            todo!()
        }
        out.add_child(self.text.clone());
        Ok(out)
    }
}

impl AsHtml for UriNode {
    fn as_html(&self, config: &HtmlBuilder, errors: &mut Vec<NoteError>) -> Result<HtmlElement, NoteError> {
        let html = if config.allow_uri {
            // not all browsers support this
            // <https://developer.mozilla.org/en-US/docs/Web/API/Navigator/registerProtocolHandler#browser_compatibility>
            let mut out = HtmlElement::new("a");
            out.add_attribute("scheme", &self.scheme.name);
            out.add_attribute("href", &format!("{}:{}", self.scheme.name, self.body.text));
            out.add_text(self.body.text.trim_matches('/'));
            out
        }
        else {
            HtmlElement::new("span").with_text(format!("{}:{}", self.scheme.name, self.body.text))
        };
        Ok(html)
    }
}

impl AsHtml for BadNode {
    fn as_html(&self, config: &HtmlBuilder, errors: &mut Vec<NoteError>) -> Result<HtmlElement, NoteError> {
        let mut out = HtmlElement::new("span");
        out.add_class("note-error");
        out.add_child(self.text.clone());
        Ok(out)
    }
}
