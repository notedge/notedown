use davinci_types::{DiagnosticSeverity, NAMESPACE, codes, diagnostic, origin};

#[test]
fn diagnostic_carries_code_origin_and_fallback() {
    let record = diagnostic("scene", codes::INVALID_GEOMETRY, DiagnosticSeverity::Error, "negative extent");
    assert_eq!(record.code().as_str(), codes::INVALID_GEOMETRY);
    assert_eq!(record.severity(), DiagnosticSeverity::Error);
    assert_eq!(record.origin().namespace(), NAMESPACE);
    assert_eq!(record.origin().component(), "scene");
    assert_eq!(record.message().fallback(), Some("negative extent"));
}

#[test]
fn origin_uses_davinci_namespace() {
    let origin = origin("diagram");
    assert_eq!(origin.namespace(), "davinci");
    assert_eq!(origin.component(), "diagram");
    assert_eq!(origin.to_string(), "davinci/diagram");
}

#[test]
fn codes_are_namespaced_and_stable() {
    for code in [
        codes::INVALID_GEOMETRY,
        codes::INVALID_STYLE,
        codes::INVALID_TRANSFORM,
        codes::SERIALIZATION_FAILED,
        codes::PARSE_FAILED,
        codes::AST_UNAVAILABLE,
        codes::UNSUPPORTED,
        codes::APPROXIMATED,
        codes::EMPTY_DIAGRAM,
    ] {
        assert!(code.starts_with("davinci."));
    }
}
