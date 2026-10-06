use notedown_formats::{FormatDirection, FormatStatus, capabilities};

#[test]
fn capabilities_do_not_overstate_partial_formats() {
    let caps = capabilities();
    #[cfg(feature = "notedown")]
    assert!(caps.iter().any(|cap| { cap.id == "notedown" && cap.direction == FormatDirection::Import && cap.status == FormatStatus::Partial }));
    #[cfg(feature = "markdown")]
    assert!(caps.iter().any(|cap| { cap.id == "markdown" && cap.direction == FormatDirection::Import && cap.status == FormatStatus::Partial }));
    #[cfg(feature = "markdown")]
    assert!(caps.iter().any(|cap| { cap.id == "markdown" && cap.direction == FormatDirection::Export && cap.status == FormatStatus::Partial }));
    #[cfg(feature = "docx")]
    assert!(caps.iter().any(|cap| { cap.id == "docx" && cap.direction == FormatDirection::Import && cap.status == FormatStatus::Partial }));
    #[cfg(feature = "docx")]
    assert!(caps.iter().any(|cap| { cap.id == "docx" && cap.direction == FormatDirection::Export && cap.status == FormatStatus::Partial }));
    #[cfg(feature = "epub")]
    assert!(caps.iter().any(|cap| { cap.id == "epub" && cap.direction == FormatDirection::Import && cap.status == FormatStatus::Partial }));
    #[cfg(feature = "doc")]
    assert!(caps.iter().any(|cap| { cap.id == "doc" && cap.direction == FormatDirection::Import && cap.status == FormatStatus::Partial }));
    #[cfg(feature = "doc")]
    assert!(caps.iter().any(|cap| { cap.id == "doc" && cap.direction == FormatDirection::Export && cap.status == FormatStatus::Unavailable }));
    assert!(caps.iter().any(|cap| { cap.id == "html" && cap.direction == FormatDirection::Export && cap.status == FormatStatus::Partial }));
    #[cfg(not(feature = "docx"))]
    assert!(!caps.iter().any(|cap| cap.id == "docx"));
    #[cfg(not(feature = "epub"))]
    assert!(!caps.iter().any(|cap| cap.id == "epub"));
}
