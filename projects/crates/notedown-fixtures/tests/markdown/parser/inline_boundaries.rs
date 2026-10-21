use oak_core::{Parser, RedNode, RedTree, SourceText, parser::session::ParseSession};
use oak_markdown::{MarkdownLanguage, MarkdownParser, parser::element_type::MarkdownElementType};

fn collect_elements(node: RedNode<MarkdownLanguage>, source: &SourceText, result: &mut Vec<(MarkdownElementType, String)>) {
    for child in node.children() {
        if let RedTree::Node(child) = child {
            result.push((child.element_type(), child.text(source).into_owned()));
            collect_elements(child, source, result);
        }
    }
}

#[test]
fn math_inline_is_a_single_node_and_does_not_consume_following_text() {
    let source = SourceText::new("before $x + y$ after");
    let language = MarkdownLanguage { allow_math: true, ..MarkdownLanguage::common_mark() };
    let parser = MarkdownParser::new(&language);
    let mut cache = ParseSession::<MarkdownLanguage>::default();
    let tree = parser.parse(&source, &[], &mut cache).result.expect("parse");
    let root = RedNode::new(tree, 0);
    let mut elements = Vec::new();
    collect_elements(root, &source, &mut elements);

    assert!(elements.iter().any(|(kind, text)| *kind == MarkdownElementType::MathInline && text == "$x + y$"));
    assert!(elements.iter().any(|(kind, text)| text.contains("after") && *kind != MarkdownElementType::MathInline), "{elements:#?}");
}

#[test]
fn footnote_reference_is_a_single_node_between_surrounding_text() {
    let source = SourceText::new("before[^1] after");
    let language = MarkdownLanguage { allow_footnotes: true, ..MarkdownLanguage::common_mark() };
    let parser = MarkdownParser::new(&language);
    let mut cache = ParseSession::<MarkdownLanguage>::default();
    let tree = parser.parse(&source, &[], &mut cache).result.expect("parse");
    let root = RedNode::new(tree, 0);
    let mut elements = Vec::new();
    collect_elements(root, &source, &mut elements);

    assert!(elements.iter().any(|(kind, text)| *kind == MarkdownElementType::FootnoteReference && text == "[^1]"));
    assert!(elements.iter().any(|(kind, text)| text.contains("after") && *kind != MarkdownElementType::FootnoteReference), "{elements:#?}");
}

#[test]
fn reference_link_is_a_single_node_between_surrounding_text() {
    let source = SourceText::new("before [label][target] after");
    let language = MarkdownLanguage::common_mark();
    let parser = MarkdownParser::new(&language);
    let mut cache = ParseSession::<MarkdownLanguage>::default();
    let tree = parser.parse(&source, &[], &mut cache).result.expect("parse");
    let root = RedNode::new(tree, 0);
    let mut elements = Vec::new();
    collect_elements(root, &source, &mut elements);

    assert!(elements.iter().any(|(kind, text)| *kind == MarkdownElementType::Link && text == "[label][target]"));
    assert!(elements.iter().any(|(kind, text)| text.contains("after") && *kind != MarkdownElementType::Link), "{elements:#?}");
}

#[test]
fn unresolved_reference_link_is_still_a_link_node() {
    let source = SourceText::new("before [label][missing] after");
    let language = MarkdownLanguage::common_mark();
    let parser = MarkdownParser::new(&language);
    let mut cache = ParseSession::<MarkdownLanguage>::default();
    let tree = parser.parse(&source, &[], &mut cache).result.expect("parse");
    let root = RedNode::new(tree, 0);
    let mut elements = Vec::new();
    collect_elements(root, &source, &mut elements);
    assert!(elements.iter().any(|(kind, text)| *kind == MarkdownElementType::Link && text == "[label][missing]"), "{elements:#?}");
}

#[test]
fn shortcut_reference_is_a_single_node_between_surrounding_text() {
    let source = SourceText::new("before [guide] after");
    let language = MarkdownLanguage::common_mark();
    let parser = MarkdownParser::new(&language);
    let mut cache = ParseSession::<MarkdownLanguage>::default();
    let tree = parser.parse(&source, &[], &mut cache).result.expect("parse");
    let root = RedNode::new(tree, 0);
    let mut elements = Vec::new();
    collect_elements(root, &source, &mut elements);
    assert!(elements.iter().any(|(kind, text)| *kind == MarkdownElementType::Link && text == "[guide]"), "{elements:#?}");
    assert!(elements.iter().any(|(kind, text)| text.contains("after") && *kind != MarkdownElementType::Link), "{elements:#?}");
}
