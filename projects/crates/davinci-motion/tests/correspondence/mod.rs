//! Correspondence tests: stable ids only, never paint order.

use davinci_motion::{Correspondence, CorrespondenceKind, match_scenes};

use crate::common::{group, rect, scene, unlabeled_rect};

fn update(id: &str) -> Correspondence {
    Correspondence { id: id.to_owned(), kind: CorrespondenceKind::Update }
}

fn enter(id: &str) -> Correspondence {
    Correspondence { id: id.to_owned(), kind: CorrespondenceKind::Enter }
}

fn exit(id: &str) -> Correspondence {
    Correspondence { id: id.to_owned(), kind: CorrespondenceKind::Exit }
}

#[test]
fn matches_updates_then_enters_then_exits() {
    let previous = scene(vec![rect("a", 0.0, 1.0), rect("b", 20.0, 1.0), rect("c", 40.0, 1.0)]);
    let next = scene(vec![rect("c", 0.0, 1.0), rect("a", 60.0, 1.0), rect("d", 80.0, 1.0)]);

    let correspondences = match_scenes(&previous, &next);
    assert_eq!(correspondences, vec![update("c"), update("a"), enter("d"), exit("b")]);
}

#[test]
fn ignores_nodes_without_ids() {
    let previous = scene(vec![rect("a", 0.0, 1.0), unlabeled_rect(10.0)]);
    let next = scene(vec![unlabeled_rect(10.0), rect("b", 20.0, 1.0)]);

    let correspondences = match_scenes(&previous, &next);
    assert_eq!(correspondences, vec![enter("b"), exit("a")]);
    assert!(correspondences.iter().all(|item| !item.id.is_empty()));
}

#[test]
fn keeps_the_first_duplicate_id() {
    let previous = scene(vec![rect("dup", 0.0, 1.0)]);
    let next = scene(vec![rect("dup", 50.0, 1.0), rect("dup", 10.0, 1.0)]);

    assert_eq!(match_scenes(&previous, &next), vec![update("dup")]);
}

#[test]
fn walks_group_children_in_document_order() {
    let previous = scene(vec![group("g", 1.0, vec![rect("a", 0.0, 1.0), rect("b", 0.0, 1.0)])]);
    let next = scene(vec![group("g", 1.0, vec![rect("b", 0.0, 1.0), rect("a", 0.0, 1.0), unlabeled_rect(0.0)])]);

    let correspondences = match_scenes(&previous, &next);
    let ids: Vec<&str> = correspondences.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, vec!["g", "b", "a"]);
    assert!(correspondences.iter().all(|item| item.kind == CorrespondenceKind::Update));
}
