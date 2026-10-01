use notedown_formats::{capabilities, FormatDirection, FormatStatus};

#[test]
fn capabilities_list_planned_modules() {
    let caps = capabilities();
    assert!(caps.iter().any(|cap| cap.id == "docx" && cap.direction == FormatDirection::Import));
    assert!(caps.iter().all(|cap| cap.status == FormatStatus::Planned));
}

#[test]
fn markdown_import_reports_not_implemented() {
    let err =
        notedown_formats::import::markdown::import_markdown_bytes("x.md", "# hi").unwrap_err();
    assert!(err.to_string().contains("markdown"));
}
