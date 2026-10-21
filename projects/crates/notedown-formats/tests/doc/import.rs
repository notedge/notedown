use notedown_formats::import::doc::split_doc_paragraphs;

#[test]
fn preserves_leading_and_trailing_spaces_inside_doc_paragraphs() {
    let paragraphs = split_doc_paragraphs("  leading\ttext  \nnext  ").collect::<Vec<_>>();
    assert_eq!(paragraphs, ["  leading\ttext  ", "next  "]);
}
