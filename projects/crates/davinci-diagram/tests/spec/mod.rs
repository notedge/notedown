use davinci_diagram::spec::{DiagramSpec, Direction};

#[test]
fn new_spec_is_empty_and_keeps_the_direction() {
    let spec = DiagramSpec::new(Direction::LeftToRight);
    assert_eq!(spec.direction, Direction::LeftToRight);
    assert!(spec.nodes.is_empty());
    assert!(spec.edges.is_empty());
    assert!(spec.clusters.is_empty());
    assert!(spec.constraints.is_empty());
}
