use notedown_formats::{capabilities, FormatDirection, FormatStatus};

#[test]
fn capabilities_mark_ready_modules() {
    let caps = capabilities();
    assert!(caps.iter().any(|cap| {
        cap.id == "markdown"
            && cap.direction == FormatDirection::Import
            && cap.status == FormatStatus::Ready
    }));
    assert!(caps.iter().any(|cap| {
        cap.id == "markdown"
            && cap.direction == FormatDirection::Export
            && cap.status == FormatStatus::Ready
    }));
    assert!(caps.iter().any(|cap| {
        cap.id == "docx"
            && cap.direction == FormatDirection::Import
            && cap.status == FormatStatus::Ready
    }));
    assert!(caps.iter().any(|cap| cap.id == "epub" && cap.status == FormatStatus::Planned));
}
