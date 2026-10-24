use davinci_types::Color;

#[test]
fn opaque_colors_use_hex() {
    assert_eq!(Color::rgb(0x12, 0x34, 0xab).to_css(), "#1234ab");
    assert_eq!(Color::BLACK.to_css(), "#000000");
    assert_eq!(Color::WHITE.to_css(), "#ffffff");
}

#[test]
fn translucent_colors_use_rgba() {
    let color = Color::rgba(255, 0, 0, 128);
    assert_eq!(color.to_css(), "rgba(255, 0, 0, 0.502)");
    assert_eq!(color.with_alpha(255).to_css(), "#ff0000");
    assert_eq!(Color::TRANSPARENT.alpha(), 0);
}

#[test]
fn channels_are_readable() {
    let color = Color::rgba(1, 2, 3, 4);
    assert_eq!((color.red(), color.green(), color.blue(), color.alpha()), (1, 2, 3, 4));
}
