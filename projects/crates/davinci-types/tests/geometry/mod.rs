use davinci_types::{Bounds, Point, Rect, Size};

#[test]
fn rect_normalizes_points_and_reports_center() {
    let rect = Rect::from_points(Point::new(30.0, 40.0), Point::new(10.0, 20.0));
    assert_eq!(rect, Rect::new(10.0, 20.0, 20.0, 20.0));
    assert_eq!(rect.center(), Point::new(20.0, 30.0));
    assert_eq!(rect.right(), 30.0);
    assert_eq!(rect.bottom(), 40.0);
    assert!(rect.contains(Point::new(10.0, 20.0)));
    assert!(!rect.contains(Point::new(9.9, 20.0)));
}

#[test]
fn bounds_normalize_and_union() {
    let a = Bounds::new(Point::new(10.0, 40.0), Point::new(30.0, 20.0));
    assert_eq!(a.min, Point::new(10.0, 20.0));
    assert_eq!(a.max, Point::new(30.0, 40.0));
    assert_eq!(a.width(), 20.0);
    assert_eq!(a.height(), 20.0);
    assert_eq!(a.center(), Point::new(20.0, 30.0));

    let b = Bounds::from_point(Point::new(-5.0, 10.0));
    let union = a.union(b);
    assert_eq!(union.min, Point::new(-5.0, 10.0));
    assert_eq!(union.max, Point::new(30.0, 40.0));
}

#[test]
fn bounds_intersection_reports_disjoint() {
    let a = Bounds::new(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
    let b = Bounds::new(Point::new(5.0, 5.0), Point::new(20.0, 20.0));
    let overlap = a.intersection(b).expect("overlapping bounds");
    assert_eq!(overlap, Bounds::new(Point::new(5.0, 5.0), Point::new(10.0, 10.0)));

    let c = Bounds::new(Point::new(20.0, 20.0), Point::new(30.0, 30.0));
    assert_eq!(a.intersection(c), None);
}

#[test]
fn rect_and_bounds_round_trip() {
    let rect = Rect::new(3.0, 4.0, 5.0, 6.0);
    let bounds = rect.to_bounds();
    assert_eq!(bounds.to_rect(), rect);
    assert_eq!(Bounds::from_rect(rect), bounds);
    assert_eq!(bounds.is_empty(), false);
    assert_eq!(Size::ZERO.is_empty(), true);
}
