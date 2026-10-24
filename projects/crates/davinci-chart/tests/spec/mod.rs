use davinci_chart::{Aggregate, Channel, ChannelEncoding, ChartSpec, Coordinate, FieldKind, FieldRef, Guide, Mark, ScaleSpec, codes};

use crate::support::{bar_spec, has_code};

#[test]
fn a_new_spec_keeps_the_mark_and_both_default_axes() {
    let spec = ChartSpec::new(Mark::Line);

    assert_eq!(spec.mark, Mark::Line);
    assert_eq!(spec.coordinate, Coordinate::Cartesian);
    assert_eq!(spec.guides, vec![Guide::axis(Channel::X), Guide::axis(Channel::Y)]);
    assert!(has_code(&spec.validate(), codes::INVALID_SPEC), "a spec without bindings must be rejected");
}

#[test]
fn a_valid_bar_spec_has_no_diagnostics() {
    assert!(bar_spec().validate().is_empty());
}

#[test]
fn undeclared_encoding_fields_are_reported() {
    let spec = ChartSpec::new(Mark::Point)
        .with_encoding(Channel::X, ChannelEncoding::field("missing", FieldKind::Quantitative))
        .with_encoding(Channel::Y, ChannelEncoding::field("also-missing", FieldKind::Quantitative));

    let diagnostics = spec.validate();
    assert_eq!(diagnostics.iter().filter(|record| record.code().as_str() == codes::UNKNOWN_FIELD).count(), 2);
}

#[test]
fn binding_a_field_with_a_different_kind_is_reported() {
    let spec = ChartSpec::new(Mark::Point)
        .with_field(FieldRef::new("x", FieldKind::Quantitative))
        .with_field(FieldRef::new("y", FieldKind::Quantitative))
        .with_encoding(Channel::X, ChannelEncoding::field("x", FieldKind::Nominal))
        .with_encoding(Channel::Y, ChannelEncoding::field("y", FieldKind::Quantitative));

    assert!(has_code(&spec.validate(), codes::TYPE_MISMATCH));
}

#[test]
fn aggregates_need_a_quantitative_measure() {
    let spec = ChartSpec::new(Mark::Bar)
        .with_field(FieldRef::new("category", FieldKind::Nominal))
        .with_encoding(Channel::X, ChannelEncoding::field("category", FieldKind::Nominal))
        .with_encoding(Channel::Y, ChannelEncoding::field("category", FieldKind::Nominal).with_aggregate(Aggregate::Mean));

    assert!(has_code(&spec.validate(), codes::INVALID_SPEC));

    let counted = ChartSpec::new(Mark::Bar)
        .with_field(FieldRef::new("category", FieldKind::Nominal))
        .with_encoding(Channel::X, ChannelEncoding::field("category", FieldKind::Nominal))
        .with_encoding(Channel::Y, ChannelEncoding::field("category", FieldKind::Nominal).with_aggregate(Aggregate::Count));
    assert!(counted.validate().is_empty(), "count works on any field kind");
}

#[test]
fn band_scales_reject_quantitative_positions() {
    let spec = ChartSpec::new(Mark::Point)
        .with_field(FieldRef::new("x", FieldKind::Quantitative))
        .with_field(FieldRef::new("y", FieldKind::Quantitative))
        .with_encoding(Channel::X, ChannelEncoding::field("x", FieldKind::Quantitative))
        .with_encoding(Channel::Y, ChannelEncoding::field("y", FieldKind::Quantitative))
        .with_scale(Channel::X, ScaleSpec::band(0.1));

    assert!(has_code(&spec.validate(), codes::INVALID_SCALE));
}

#[test]
fn a_degenerate_linear_domain_is_rejected() {
    let spec = bar_spec().with_scale(Channel::Y, ScaleSpec::linear_domain(5.0, 5.0));
    assert!(has_code(&spec.validate(), codes::INVALID_SCALE));
}

#[test]
fn the_bar_mark_rejects_a_size_channel() {
    let spec = bar_spec()
        .with_field(FieldRef::new("value2", FieldKind::Quantitative))
        .with_encoding(Channel::Size, ChannelEncoding::field("value2", FieldKind::Quantitative));

    assert!(has_code(&spec.validate(), codes::INVALID_SPEC));
}

#[test]
fn point_marks_accept_shape_and_size_channels() {
    let spec = ChartSpec::new(Mark::Point)
        .with_field(FieldRef::new("x", FieldKind::Quantitative))
        .with_field(FieldRef::new("y", FieldKind::Quantitative))
        .with_field(FieldRef::new("size", FieldKind::Quantitative))
        .with_field(FieldRef::new("shape", FieldKind::Nominal))
        .with_encoding(Channel::X, ChannelEncoding::field("x", FieldKind::Quantitative))
        .with_encoding(Channel::Y, ChannelEncoding::field("y", FieldKind::Quantitative))
        .with_encoding(Channel::Size, ChannelEncoding::field("size", FieldKind::Quantitative))
        .with_encoding(Channel::Shape, ChannelEncoding::field("shape", FieldKind::Nominal));

    assert!(spec.validate().is_empty());
}

#[test]
fn an_empty_guide_list_is_legal() {
    let spec = bar_spec().with_guides(Vec::new());
    assert!(spec.validate().is_empty());
}
