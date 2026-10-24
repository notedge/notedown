use davinci_types::{Point, Transform};

fn assert_point(actual: Point, expected: Point) {
    assert!((actual.x - expected.x).abs() < 1e-9, "x: {actual:?} != {expected:?}");
    assert!((actual.y - expected.y).abs() < 1e-9, "y: {actual:?} != {expected:?}");
}

#[test]
fn identity_leaves_points_unchanged() {
    assert!(Transform::IDENTITY.is_identity());
    assert_point(Transform::IDENTITY.apply_point(Point::new(3.0, -7.0)), Point::new(3.0, -7.0));
}

#[test]
fn composition_applies_self_then_next() {
    let transform = Transform::translate(10.0, 5.0).then(Transform::scale(2.0, 3.0));
    assert_point(transform.apply_point(Point::new(1.0, 1.0)), Point::new(22.0, 18.0));

    let nested = Transform::scale(2.0, 2.0).then(Transform::translate(10.0, 0.0));
    assert_point(nested.apply_point(Point::new(1.0, 0.0)), Point::new(12.0, 0.0));
}

#[test]
fn rotation_matches_svg_clockwise_orientation() {
    let rotation = Transform::rotate_degrees(90.0);
    assert_point(rotation.apply_point(Point::new(1.0, 0.0)), Point::new(0.0, 1.0));
    assert_point(rotation.apply_point(Point::new(0.0, 1.0)), Point::new(-1.0, 0.0));
}

#[test]
fn determinant_tracks_area_scale() {
    assert_eq!(Transform::IDENTITY.determinant(), 1.0);
    assert_eq!(Transform::scale(3.0, 2.0).determinant(), 6.0);
    assert!((Transform::rotate_degrees(45.0).determinant() - 1.0).abs() < 1e-9);
}

#[test]
fn matrix_components_are_exposed() {
    let transform = Transform::new(2.0, 0.0, 0.0, 3.0, 4.0, 5.0);
    assert_eq!((transform.a(), transform.b(), transform.c(), transform.d(), transform.e(), transform.f()), (2.0, 0.0, 0.0, 3.0, 4.0, 5.0));
    assert_eq!(Transform::default(), Transform::IDENTITY);
}
