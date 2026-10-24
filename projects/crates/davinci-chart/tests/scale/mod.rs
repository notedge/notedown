use davinci_chart::scale::{MARKERS, PALETTE, Scale, linear_ticks, train_band, train_linear, train_nominal, train_point};
use davinci_scene::MarkerShape;

use crate::support::approx;

#[test]
fn explicit_linear_domains_map_their_endpoints() {
    let scale = train_linear(&[3.0], Some((0.0, 10.0)), (0.0, 100.0));

    assert_eq!(scale.domain, (0.0, 10.0));
    assert!(approx(scale.map(0.0), 0.0));
    assert!(approx(scale.map(5.0), 50.0));
    assert!(approx(scale.map(10.0), 100.0));
}

#[test]
fn inferred_domains_use_the_data_extremes() {
    let scale = train_linear(&[2.0, 8.0, -4.0], None, (0.0, 200.0));

    assert_eq!(scale.domain, (-4.0, 8.0));
    assert!(approx(scale.map(-4.0), 0.0));
    assert!(approx(scale.map(8.0), 200.0));
}

#[test]
fn degenerate_linear_domains_expand_without_nan() {
    let scale = train_linear(&[5.0], None, (0.0, 100.0));

    assert!(scale.domain.0 < 5.0 && 5.0 < scale.domain.1, "a single value must expand its domain");
    let position = scale.map(5.0);
    assert!(position.is_finite());
    assert!(approx(position, 50.0));

    for value in [scale.domain.0, scale.domain.1, 3.0, 99.0] {
        assert!(scale.map(value).is_finite(), "mapping {value} must stay finite");
    }
}

#[test]
fn band_scales_reserve_padding_inside_the_range() {
    let categories = vec!["A".to_string(), "B".to_string()];
    let scale = train_band(&categories, (0.0, 100.0), 0.2);

    assert!(approx(scale.step, 100.0 / 2.2));
    assert!(approx(scale.band_width, scale.step * 0.8));
    assert!(approx(scale.position("A").expect("A"), scale.step * 0.2));
    assert!(approx(scale.center("A").expect("A"), 27.272_727_272_727_27));
    assert!(approx(scale.center("B").expect("B"), 72.727_272_727_272_72));
    assert!(scale.position("missing").is_none());
}

#[test]
fn a_single_band_stays_centered() {
    let scale = train_band(&["only".to_string()], (0.0, 100.0), 0.1);

    assert!(approx(scale.center("only").expect("only"), 50.0));
    assert!(scale.band_width > 0.0);
}

#[test]
fn point_scales_place_categories_at_their_slots() {
    let two = train_point(&["A".to_string(), "B".to_string()], (0.0, 100.0), 0.5);
    assert!(approx(two.position("A").expect("A"), 25.0));
    assert!(approx(two.position("B").expect("B"), 75.0));

    let single = train_point(&["only".to_string()], (0.0, 100.0), 0.5);
    assert!(approx(single.position("only").expect("only"), 50.0));
}

#[test]
fn linear_ticks_stay_inside_the_domain_and_are_sorted() {
    let ticks = linear_ticks((0.0, 10.0), 5);
    assert!(ticks.len() >= 2);
    assert!(ticks.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(ticks.iter().all(|tick| (0.0..=10.0).contains(tick)));
    assert!(approx(ticks[0], 0.0));
    assert!(approx(ticks[ticks.len() - 1], 10.0));

    let signed = linear_ticks((-1.0, 1.0), 3);
    assert!(signed.iter().all(|tick| (-1.0..=1.0).contains(tick)));
    assert!(signed.len() >= 2);
}

#[test]
fn nominal_scales_assign_stable_colors_and_markers() {
    let domain = vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()];
    let scale = train_nominal(&domain);

    assert_eq!(scale.color("a"), Some(PALETTE[0]));
    assert_eq!(scale.color("c"), Some(PALETTE[2]));
    assert_eq!(scale.marker("a"), Some(MARKERS[0]));
    assert_eq!(scale.marker("b"), Some(MarkerShape::Triangle));
    assert_eq!(scale.marker("c"), Some(MarkerShape::Diamond));
    assert_eq!(scale.color("missing"), None);
    assert_eq!(scale.marker("missing"), None);
}

#[test]
fn the_scale_enum_reports_its_family() {
    let linear = Scale::Linear(train_linear(&[1.0], None, (0.0, 1.0)));
    let band = Scale::Band(train_band(&["a".to_string()], (0.0, 1.0), 0.1));
    let point = Scale::Point(train_point(&["a".to_string()], (0.0, 1.0), 0.5));
    let nominal = Scale::Nominal(train_nominal(&["a".to_string()]));

    assert!(linear.linear().is_some());
    assert!(band.band().is_some());
    assert!(point.point().is_some());
    assert!(nominal.nominal().is_some());
    assert!(approx(band.position("a").expect("a"), 0.5));
    assert!(approx(point.position("a").expect("a"), 0.5));
    assert!(band.position("missing").is_none());
}
