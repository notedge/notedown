use davinci_types::{Color, DIAGRAM_THEME, contrast_ratio};

const LIGHT_PAGE: Color = Color::rgb(0xFF, 0xFF, 0xFF);
const DARK_PAGE: Color = Color::rgb(0x0D, 0x11, 0x17);

#[test]
fn luminance_matches_the_wcag_reference_values() {
    assert!((davinci_types::relative_luminance(Color::rgb(0xFF, 0xFF, 0xFF)) - 1.0).abs() < 1e-9);
    assert!(davinci_types::relative_luminance(Color::rgb(0x00, 0x00, 0x00)).abs() < 1e-9);
    assert_eq!(contrast_ratio(Color::rgb(0, 0, 0), Color::rgb(0xFF, 0xFF, 0xFF)), 21.0);
}

#[test]
fn light_palette_keeps_text_and_edges_at_wcag_aa() {
    let tokens = DIAGRAM_THEME.light;
    assert!(contrast_ratio(tokens.edge_stroke, LIGHT_PAGE) >= 4.5, "edge vs page: {}", contrast_ratio(tokens.edge_stroke, LIGHT_PAGE));
    assert!(
        contrast_ratio(tokens.edge_label_text, tokens.edge_label_bg) >= 4.5,
        "label text vs backing: {}",
        contrast_ratio(tokens.edge_label_text, tokens.edge_label_bg)
    );
    assert!(
        contrast_ratio(tokens.node_text, tokens.node_fill) >= 4.5,
        "node text vs fill: {}",
        contrast_ratio(tokens.node_text, tokens.node_fill)
    );
}

#[test]
fn dark_palette_keeps_text_and_edges_at_wcag_aa() {
    let tokens = DIAGRAM_THEME.dark;
    assert!(contrast_ratio(tokens.edge_stroke, DARK_PAGE) >= 4.5, "edge vs page: {}", contrast_ratio(tokens.edge_stroke, DARK_PAGE));
    assert!(
        contrast_ratio(tokens.edge_label_text, tokens.edge_label_bg) >= 4.5,
        "label text vs backing: {}",
        contrast_ratio(tokens.edge_label_text, tokens.edge_label_bg)
    );
    assert!(
        contrast_ratio(tokens.node_text, tokens.node_fill) >= 4.5,
        "node text vs fill: {}",
        contrast_ratio(tokens.node_text, tokens.node_fill)
    );
}
