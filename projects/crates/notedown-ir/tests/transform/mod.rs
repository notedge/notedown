//! DocumentTransform infrastructure 集成测试。
//!
//! 覆盖：
//! - [`TransformError`] 枚举
//! - [`DocumentGraph`] mutation API（delete_block / replace_block / set_children / ...）
//! - [`DocumentGraph::walk`] / [`DocumentGraph::walk_mut`] 遍历
//! - [`RewriteResult`] 各种变体的实际效果

use notedown_ir::{
    Block, Diagnostic, DocumentGraph, IdAllocator, Inline, ListItem, NodeId,
    RewriteResult, Traversal, TransformError,
};

// ---------------------------------------------------------------------------
// Helper — 构造一棵典型的 graph
// ---------------------------------------------------------------------------

/// 构造一个 graph：
///
/// ```text
/// Section(level=1, "Root")
/// ├── Paragraph "Hello"
/// ├── Section(level=2, "Nested")
/// │   └── Paragraph "Nested body"
/// └── List(unordered)
///     └── ListItem
///         └── Paragraph "Item body"
/// ```
fn build_sample_graph() -> (DocumentGraph, NodeId, NodeId, NodeId, NodeId, NodeId, NodeId) {
    let mut ids = IdAllocator::default();
    let mut g = DocumentGraph::new(ids.document_id());

    let p_hello = g.push_block(Block::Paragraph {
        content: vec![Inline::Text { text: "Hello".into() }],
    });
    let p_nested_body = g.push_block(Block::Paragraph {
        content: vec![Inline::Text { text: "Nested body".into() }],
    });
    let sec_nested = g.push_block(Block::Section {
        level: 2,
        title: vec![Inline::Text { text: "Nested".into() }],
        children: vec![p_nested_body],
    });
    let p_item_body = g.push_block(Block::Paragraph {
        content: vec![Inline::Text { text: "Item body".into() }],
    });
    let list_root = g.push_block(Block::List {
        ordered: false,
        items: vec![ListItem {
            content: vec![],
            children: vec![p_item_body],
            number: None,
        }],
    });
    let sec_root = g.push_block(Block::Section {
        level: 1,
        title: vec![Inline::Text { text: "Root".into() }],
        children: vec![p_hello, sec_nested, list_root],
    });

    // (graph, sec_root, p_hello, sec_nested, p_nested_body, list_root, p_item_body)
    (g, sec_root, p_hello, sec_nested, p_nested_body, list_root, p_item_body)
}

// ===========================================================================
// 1. delete_block
// ===========================================================================

#[test]
fn delete_block_removes_from_graph() {
    let (mut g, _sec_root, p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    assert!(g.block(p_hello).is_some());
    assert_eq!(g.blocks.len(), 6);

    g.delete_block(p_hello).unwrap();

    assert!(g.block(p_hello).is_none(), "node should be gone");
    assert_eq!(g.blocks.len(), 5);
    // revision 自增
    assert!(g.revision > 0);
}

#[test]
fn delete_block_returns_not_found_for_missing_node() {
    let (mut g, ..) = build_sample_graph();
    let fake = NodeId(9999);
    let err = g.delete_block(fake).unwrap_err();
    assert_eq!(err, TransformError::NotFound(fake));
}

#[test]
fn delete_block_clears_parent_children_refs() {
    let (mut g, _sec_root, p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    g.delete_block(p_hello).unwrap();

    // sec_root 的 children 里不应该再有 p_hello
    let sec_root = g.block(g.blocks.last().unwrap().id).unwrap();
    if let Block::Section { children, .. } = &sec_root.block {
        assert!(!children.contains(&p_hello));
        assert_eq!(children.len(), 2);
    }
    else {
        panic!("expected root to be a Section");
    }
}

#[test]
fn delete_block_with_dangling_relation_fails_validation() {
    use notedown_ir::{LinkId, Relation, RelationEndpoint, RelationKind};

    let (mut g, _sec_root, p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    // 建一个指向 p_hello 的 relation
    g.push_relation(Relation {
        id: LinkId(1),
        kind: RelationKind::References,
        source: RelationEndpoint::Node(_sec_root),
        target: RelationEndpoint::Node(p_hello),
    });

    // 初始 validate 应该是干净的
    let report = g.validate();
    assert!(report.is_valid(), "before delete");

    // 删除 p_hello
    g.delete_block(p_hello).unwrap();

    // 现在 relation.target 变成 dangling
    let report = g.validate();
    assert!(
        !report.is_valid(),
        "delete_block 后引用它的 relation 应该变成 dangling（由 validate 捕获）"
    );
}

// ===========================================================================
// 2. replace_block
// ===========================================================================

#[test]
fn replace_block_swaps_payload_preserves_id() {
    let (mut g, _sec_root, p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    let new_block = Block::Code {
        language: Some("rust".into()),
        content: "fn main() {}".into(),
    };

    g.replace_block(p_hello, new_block.clone()).unwrap();

    let node = g.block(p_hello).expect("id still there");
    assert_eq!(node.block, new_block, "payload replaced");
}

#[test]
fn replace_block_missing_node_returns_not_found() {
    let (mut g, ..) = build_sample_graph();
    let fake = NodeId(42);
    let err = g
        .replace_block(fake, Block::Paragraph { content: vec![] })
        .unwrap_err();
    assert_eq!(err, TransformError::NotFound(fake));
}

// ===========================================================================
// 3. set_children / append_child / remove_child
// ===========================================================================

#[test]
fn set_children_replaces_section_children() {
    let (mut g, sec_root, p_hello, sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    g.set_children(sec_root, vec![p_hello, sec_nested]).unwrap();

    let node = g.block(sec_root).unwrap();
    if let Block::Section { children, .. } = &node.block {
        assert_eq!(children, &[p_hello, sec_nested]);
    }
    else {
        panic!("expected Section");
    }
}

#[test]
fn set_children_on_non_section_fails() {
    let (mut g, _sec_root, p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    let err = g.set_children(p_hello, vec![]).unwrap_err();
    assert!(matches!(err, TransformError::InvalidParent(_)));
}

#[test]
fn append_child_adds_to_section() {
    let (mut g, sec_root, _p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    let new_p = g.push_block(Block::Paragraph {
        content: vec![Inline::Text { text: "Appended".into() }],
    });

    g.append_child(sec_root, new_p).unwrap();

    let node = g.block(sec_root).unwrap();
    if let Block::Section { children, .. } = &node.block {
        assert_eq!(children.last(), Some(&new_p));
    }
    else {
        panic!("expected Section");
    }
}

#[test]
fn remove_child_takes_child_out_of_section() {
    let (mut g, sec_root, p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    g.remove_child(sec_root, p_hello).unwrap();

    let node = g.block(sec_root).unwrap();
    if let Block::Section { children, .. } = &node.block {
        assert!(!children.contains(&p_hello));
        assert_eq!(children.len(), 2);
    }
    else {
        panic!("expected Section");
    }
}

#[test]
fn remove_child_when_child_not_present_errors() {
    let (mut g, sec_root, _p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    let fake_child = NodeId(9999);
    let err = g.remove_child(sec_root, fake_child).unwrap_err();
    assert!(matches!(err, TransformError::ChildNotFound { .. }));
}

// ===========================================================================
// 4. walk_mut — 遍历顺序
// ===========================================================================

#[test]
fn walk_mut_bottomup_processes_children_before_parent() {
    let (mut g, sec_root, p_hello, sec_nested, p_nested_body, list_root, p_item_body) =
        build_sample_graph();

    let mut order: Vec<NodeId> = Vec::new();

    g.walk_mut(Traversal::BottomUp, |id, _block, _kids| {
        order.push(id);
        RewriteResult::Keep
    });

    // bottom-up: children 先于 parents
    // 期望 p_hello, p_nested_body, sec_nested, p_item_body, list_root, sec_root
    let idx_of = |ids: &[NodeId], id: NodeId| ids.iter().position(|x| *x == id).unwrap();
    assert!(idx_of(&order, p_nested_body) < idx_of(&order, sec_nested));
    assert!(idx_of(&order, p_item_body) < idx_of(&order, list_root));
    assert!(idx_of(&order, sec_nested) < idx_of(&order, sec_root));
    assert!(idx_of(&order, p_hello) < idx_of(&order, sec_root));
    assert_eq!(order.last(), Some(&sec_root), "root 应该是最后一个");
}

#[test]
fn walk_mut_topdown_processes_parent_before_children() {
    let (mut g, sec_root, _p_hello, sec_nested, _p_nested_body, list_root, p_item_body) =
        build_sample_graph();

    let mut order: Vec<NodeId> = Vec::new();

    g.walk_mut(Traversal::TopDown, |id, _block, _kids| {
        order.push(id);
        RewriteResult::Keep
    });

    let idx_of = |ids: &[NodeId], id: NodeId| ids.iter().position(|x| *x == id).unwrap();
    assert_eq!(order.first(), Some(&sec_root), "root 应该是第一个");
    assert!(idx_of(&order, sec_root) < idx_of(&order, sec_nested));
    assert!(idx_of(&order, sec_root) < idx_of(&order, list_root));
    assert!(idx_of(&order, sec_nested) < idx_of(&order, p_item_body));
}

// ===========================================================================
// 5. walk_mut + Remove
// ===========================================================================

#[test]
fn walk_mut_remove_deletes_node() {
    let (mut g, _sec_root, p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    assert!(g.block(p_hello).is_some());

    g.walk_mut(Traversal::BottomUp, |id, _block, _kids| {
        if id == p_hello {
            RewriteResult::Remove
        }
        else {
            RewriteResult::Keep
        }
    });

    assert!(g.block(p_hello).is_none(), "Remove 应该从 blocks Vec 中删除节点");
}

// ===========================================================================
// 6. walk_mut + Replace
// ===========================================================================

#[test]
fn walk_mut_replace_swaps_old_node_for_new_on_parent() {
    let (mut g, sec_root, p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    // 新节点必须在 walk_mut 之前 push 进 graph
    let new_code = g.push_block(Block::Code {
        language: Some("rust".into()),
        content: "println!(\"hi\")".into(),
    });

    g.walk_mut(Traversal::BottomUp, |id, _block, _kids| {
        if id == p_hello {
            RewriteResult::Replace(vec![new_code])
        }
        else {
            RewriteResult::Keep
        }
    });

    // p_hello 不见了
    assert!(g.block(p_hello).is_none());
    // new_code 还在
    assert!(g.block(new_code).is_some());
    // parent children 更新
    let root = g.block(sec_root).unwrap();
    if let Block::Section { children, .. } = &root.block {
        assert!(!children.contains(&p_hello));
        assert!(children.contains(&new_code));
    }
    else {
        panic!("expected Section");
    }
}

#[test]
fn walk_mut_replace_with_empty_vec_removes_from_parent() {
    let (mut g, sec_root, p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    g.walk_mut(Traversal::BottomUp, |id, _block, _kids| {
        if id == p_hello {
            RewriteResult::Replace(vec![])
        }
        else {
            RewriteResult::Keep
        }
    });

    assert!(g.block(p_hello).is_none());
    let root = g.block(sec_root).unwrap();
    if let Block::Section { children, .. } = &root.block {
        assert!(!children.contains(&p_hello));
        assert_eq!(children.len(), 2);
    }
    else {
        panic!("expected Section");
    }
}

// ===========================================================================
// 7. walk_mut + Error
// ===========================================================================

#[test]
fn walk_mut_error_collects_diagnostic_leaves_node_untouched() {
    let (mut g, _sec_root, p_hello, _sec_nested, _p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    let diagnostics = g.walk_mut(Traversal::BottomUp, |id, _block, _kids| {
        if id == p_hello {
            RewriteResult::Error(Diagnostic::for_node(id, "can't touch this"))
        }
        else {
            RewriteResult::Keep
        }
    });

    // 诊断被收集
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].node, Some(p_hello));
    assert!(diagnostics[0].message.contains("can't touch this"));

    // 节点保持原样
    assert!(g.block(p_hello).is_some());
}

// ===========================================================================
// 8. walk_mut + Unwrap
// ===========================================================================

#[test]
fn walk_mut_unwrap_promotes_children_to_parent() {
    let (mut g, sec_root, _p_hello, sec_nested, p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    // sec_root.children = [p_hello, sec_nested, list_root]
    // 我们 unwrap sec_nested —— sec_nested 消失，p_nested_body 提升到 sec_root.children
    g.walk_mut(Traversal::BottomUp, |id, _block, _kids| {
        if id == sec_nested {
            RewriteResult::Unwrap
        }
        else {
            RewriteResult::Keep
        }
    });

    assert!(g.block(sec_nested).is_none(), "被 Unwrap 的节点本体应被删除");
    assert!(g.block(p_nested_body).is_some(), "子节点 block 本体仍应存在");

    let root = g.block(sec_root).unwrap();
    if let Block::Section { children, .. } = &root.block {
        // 期望 children: [p_hello, p_nested_body, list_root]（原来 sec_nested 的位置被其 children 替换）
        assert!(children.contains(&p_nested_body));
        assert!(!children.contains(&sec_nested));
    }
    else {
        panic!("expected Section");
    }
}

// ===========================================================================
// 9. walk (不可变版本)
// ===========================================================================

#[test]
fn walk_collects_all_reachable_nodes() {
    let (g, sec_root, p_hello, sec_nested, p_nested_body, list_root, p_item_body) =
        build_sample_graph();

    let visited = g.walk(Traversal::BottomUp, |_b, _kids| RewriteResult::Keep);

    // 6 个节点全部被访问
    assert_eq!(visited.len(), 6);
    for expected in [sec_root, p_hello, sec_nested, p_nested_body, list_root, p_item_body] {
        assert!(visited.contains(&expected), "节点 {expected:?} 应该被访问");
    }
}

#[test]
fn walk_topdown_skipchildren_stops_descendants() {
    let (g, _sec_root, _p_hello, _sec_nested, p_nested_body, _list_root, _p_item_body) =
        build_sample_graph();

    let visited = g.walk(Traversal::TopDown, |_b, kids| {
        // 只要遇到带 children 的 Section 就 SkipChildren
        if !kids.is_empty() {
            RewriteResult::SkipChildren
        }
        else {
            RewriteResult::Keep
        }
    });

    // 在这个 sample 里有 children 的节点是 sec_root 和 sec_nested 和 list_root
    // 当它们被 SkipChildren 时，其 descendants 不被访问
    // 结果：sec_root, sec_nested, list_root（以及 p_hello, p_item_body 因为它们的 parent 被 SkipChildren 了）
    // 嗯等下，这个逻辑有点复杂。让我们换个更清晰的验证。

    // 简单验证：SkipChildren 后至少 p_nested_body（sec_nested 的 child）不会被访问到
    assert!(
        !visited.contains(&p_nested_body),
        "sec_nested SkipChildren 后 p_nested_body 不应被访问"
    );
}
