//! 图遍历 / 变换框架 —— 对 [`DocumentGraph`] 的所有节点做有序访问，
//! 并允许 visitor 通过 [`RewriteResult`] 改变图的结构。
//!
//! # 设计
//!
//! - [`DocumentGraph::walk_mut`] **先**从当前 children 结构快照式收集所有
//!   可到达的 [`NodeId`]，**再**按 [`Traversal`] 顺序逐个调用 visitor。
//! - 这意味着单次迭代中的集合是固定的；visitor 在闭包内做的 mutation
//!   不会导致节点被重复访问。
//! - **默认 BottomUp**：子节点先于父节点被访问，这样 visitor 返回
//!   `RewriteResult::Remove` 时不会让父节点的迭代状态失效。

use std::collections::HashSet;

use crate::{
    block::Block,
    graph::DocumentGraph,
    id::NodeId,
    transform_errors::{Diagnostic, RewriteResult, Traversal, TransformError},
};

// ---------------------------------------------------------------------------
// DocumentGraph 上的 walk / walk_mut
// ---------------------------------------------------------------------------

impl DocumentGraph {
    /// 对图中所有 reachable block 做一次不可变遍历。
    ///
    /// 返回实际访问过的 [`NodeId`] 列表（按 `order` 排序）。
    /// visitor 返回的 [`RewriteResult`] 仅用于控制遍历流程 ——
    /// 图本身不会被修改。
    pub fn walk<F>(&self, order: Traversal, mut visit: F) -> Vec<NodeId>
    where
        F: FnMut(&Block, &[NodeId]) -> RewriteResult,
    {
        let (top_down, skipable) = match order {
            Traversal::TopDown => (true, true),
            Traversal::BottomUp => (false, false),
        };

        let order_ids = if top_down {
            Self::collect_pre_order(self)
        }
        else {
            Self::collect_post_order(self)
        };

        let mut skip_children = HashSet::<NodeId>::new();
        let mut visited = Vec::with_capacity(order_ids.len());

        for id in &order_ids {
            if skipable && skip_children.contains(id) {
                continue;
            }
            let Some(node) = self.block(*id) else { continue };
            let kids = Self::immediate_children(&node.block);
            let result = visit(&node.block, kids);
            visited.push(*id);

            match result {
                RewriteResult::SkipChildren => {
                    if top_down {
                        // 标记所有 descendants 跳过
                        Self::mark_descendants_skipped(self, *id, &mut skip_children);
                    }
                }
                RewriteResult::Keep
                | RewriteResult::Replace(_)
                | RewriteResult::Remove
                | RewriteResult::Unwrap
                | RewriteResult::Error(_) => {}
            }
        }

        visited
    }

    /// 可变版本的 walk：收集所有 reachable NodeId，按 `order` 顺序调用 visitor，
    /// 然后根据 [`RewriteResult`] 对图做相应的 mutation。
    ///
    /// # 约束
    ///
    /// - visitor **必须自己先**把 [`RewriteResult::Replace`] 里的新节点
    ///   push 进 graph；本函数只会做替换/删除/解包，不会替你创建节点。
    /// - 返回值是本次变换收集到的 [`Diagnostic`] 列表。
    pub fn walk_mut<F>(&mut self, order: Traversal, mut visit: F) -> Vec<Diagnostic>
    where
        F: FnMut(NodeId, &Block, &[NodeId]) -> RewriteResult,
    {
        let order_ids = match order {
            Traversal::TopDown => Self::collect_pre_order(self),
            Traversal::BottomUp => Self::collect_post_order(self),
        };

        let mut diagnostics = Vec::new();
        // 在 TopDown 模式下记录哪些节点（及其 descendants）应该被跳过
        let mut skip_set = HashSet::<NodeId>::new();

        for id in order_ids {
            if matches!(order, Traversal::TopDown) && skip_set.contains(&id) {
                continue;
            }
            // 如果节点已被前一步 Remove/Unwrap 删除，直接跳过
            let Some(node_pos) = self.blocks.iter().position(|n| n.id == id) else {
                continue;
            };
            let block = self.blocks[node_pos].block.clone();
            let kids = Self::immediate_children(&block);

            match visit(id, &block, kids) {
                RewriteResult::Keep => {}
                RewriteResult::SkipChildren => {
                    if matches!(order, Traversal::TopDown) {
                        Self::mark_descendants_skipped(&*self, id, &mut skip_set);
                    }
                }
                RewriteResult::Error(d) => diagnostics.push(d),
                RewriteResult::Remove => {
                    // delete_block 会同时清除它自身 + 从所有 children 列表里移除引用
                    // 嵌套的子节点虽然仍在 blocks Vec 中，但已不再可从根访问；
                    // 同时它们在别人的 children 列表中的引用也已被 delete_block 递归清除。
                    //
                    // (注：这里的子节点 block 本体不会被 delete_block 物理删除，
                    //  但它们失去了 parent 引用，walk 也不会再访问到它们。
                    //  如果想彻底清理孤立节点，可以额外调用 sweep_orphans。)
                    let _ = self.delete_block(id);
                }
                RewriteResult::Unwrap => {
                    self.apply_unwrap(id);
                }
                RewriteResult::Replace(new_ids) => {
                    if let Err(d) = self.apply_replace(id, new_ids) {
                        diagnostics.push(Diagnostic::for_node(id, d.to_string()));
                    }
                }
            }
        }

        diagnostics
    }
}

// ---------------------------------------------------------------------------
// 内部辅助函数 —— 也 pub(crate) 以便 mutation / validate 共享
// ---------------------------------------------------------------------------

impl DocumentGraph {
    /// 从一个 block 中取出其直接子节点 NodeId 列表。
    ///
    /// - Section → children
    /// - List → 所有 items 的 children 合并
    /// - 其它块类型 → 空切片
    pub(crate) fn immediate_children(block: &Block) -> &[NodeId] {
        match block {
            Block::Section { children, .. } => children.as_slice(),
            _ => &[],
        }
    }

    /// DFS 生成 post-order（children 先于 parent）的 NodeId 列表。
    fn collect_post_order(&self) -> Vec<NodeId> {
        let roots = Self::collect_roots(&self.blocks);
        let mut result = Vec::new();
        let mut stack: Vec<(NodeId, usize)> = roots.into_iter().map(|r| (r, 0)).collect();

        while let Some((current, child_idx)) = stack.pop() {
            let Some(node) = self.block(current) else { continue };
            let kids = Self::all_children(&node.block);
            if child_idx < kids.len() {
                // 还没处理完所有 children —— 把自己压回栈上 + 索引 + 1，再压下一个 child
                stack.push((current, child_idx + 1));
                stack.push((kids[child_idx], 0));
            }
            else {
                // children 全处理完了，现在 post-order push
                result.push(current);
            }
        }
        result
    }

    /// DFS 生成 pre-order（parent 先于 children）的 NodeId 列表。
    fn collect_pre_order(&self) -> Vec<NodeId> {
        let roots = Self::collect_roots(&self.blocks);
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = roots;

        while let Some(current) = stack.pop() {
            if !seen.insert(current) {
                continue;
            }
            result.push(current);
            // 倒序 push，这样 pop 时是正序
            if let Some(node) = self.block(current) {
                let kids = Self::all_children(&node.block);
                for child in kids.iter().rev() {
                    if !seen.contains(child) {
                        stack.push(*child);
                    }
                }
            }
        }
        result
    }

    /// 找出图中的顶层 NodeId（不被任何 Section.children / ListItem.children 包含）。
    fn collect_roots(blocks: &[crate::block::BlockNode]) -> Vec<NodeId> {
        let mut all = HashSet::new();
        let mut contained = HashSet::new();
        for node in blocks {
            all.insert(node.id);
            match &node.block {
                Block::Section { children, .. } => {
                    for c in children {
                        contained.insert(*c);
                    }
                }
                Block::List { items, .. } => {
                    for item in items {
                        for c in &item.children {
                            contained.insert(*c);
                        }
                    }
                }
                _ => {}
            }
        }
        all.into_iter().filter(|id| !contained.contains(id)).collect()
    }

    /// 一个节点所有结构上的子节点（Section.children 直接展开，List 递归 items.children）。
    fn all_children(block: &Block) -> Vec<NodeId> {
        let mut out = Vec::new();
        Self::collect_children_rec(block, &mut out);
        out
    }

    fn collect_children_rec(block: &Block, out: &mut Vec<NodeId>) {
        match block {
            Block::Section { children, .. } => out.extend(children.iter().copied()),
            Block::List { items, .. } => {
                for item in items {
                    out.extend(item.children.iter().copied());
                }
            }
            _ => {}
        }
    }

    /// 标记从 `id` 出发所有后代节点都应该被跳过（TopDown + SkipChildren 场景）。
    fn mark_descendants_skipped(&self, id: NodeId, skip_set: &mut HashSet<NodeId>) {
        let Some(node) = self.block(id) else { return };
        let kids = Self::all_children(&node.block);
        let mut stack: Vec<NodeId> = kids.into_iter().collect();
        while let Some(c) = stack.pop() {
            if !skip_set.insert(c) {
                continue;
            }
            if let Some(n) = self.block(c) {
                stack.extend(Self::all_children(&n.block));
            }
        }
    }

    /// 找到包含 `child_id` 的 parent（Section 或包含它的 ListItem 所在的 List）。
    ///
    /// 返回值：`Option<(parent_node_id, contained_via)>` —— 这里 `contained_via`
    /// 只做标记用。如果 child 是顶层节点（没 parent），返回 `None`。
    fn find_parent(&self, child_id: NodeId) -> Option<NodeId> {
        for node in &self.blocks {
            match &node.block {
                Block::Section { children, .. } => {
                    if children.iter().any(|c| *c == child_id) {
                        return Some(node.id);
                    }
                }
                Block::List { items, .. } => {
                    if items.iter().any(|it| it.children.iter().any(|c| *c == child_id)) {
                        return Some(node.id);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// 把 `id` 从它的 parent children 列表中移除，替换为 `replacements`。
    /// 然后删除 `id` 对应的 block 节点。
    fn apply_replace(&mut self, id: NodeId, replacements: Vec<NodeId>) -> Result<(), TransformError> {
        // 1. 先从 parent 的 children 中替换
        if let Some(parent_id) = self.find_parent(id) {
            self.replace_in_parent_children(parent_id, id, &replacements)?;
        }
        // 2. 删除旧 block
        self.delete_block(id)?;
        Ok(())
    }

    /// 删除 `id`，把它的 children 接到 parent 的 children 里（Unwrap 语义）。
    fn apply_unwrap(&mut self, id: NodeId) {
        // 取出要被 unwrap 的节点当前 children（在它还没被删除之前）
        let Some(node_pos) = self.blocks.iter().position(|n| n.id == id) else {
            return;
        };
        let children: Vec<NodeId> = match &self.blocks[node_pos].block {
            Block::Section { children, .. } => children.clone(),
            _ => Vec::new(),
        };

        // 先把 parent 里的 id 替换为 id.children
        if let Some(parent_id) = self.find_parent(id) {
            let _ = self.replace_in_parent_children(parent_id, id, &children);
        }
        // 再 delete_block —— 它会清除所有对 id 的引用
        let _ = self.delete_block(id);
    }

    fn replace_in_parent_children(
        &mut self,
        parent_id: NodeId,
        old_child: NodeId,
        new_children: &[NodeId],
    ) -> Result<(), TransformError> {
        let node = self
            .blocks
            .iter_mut()
            .find(|n| n.id == parent_id)
            .ok_or(TransformError::NotFound(parent_id))?;
        match &mut node.block {
            Block::Section { children, .. } => {
                if let Some(pos) = children.iter().position(|c| *c == old_child) {
                    children.splice(pos..=pos, new_children.iter().copied());
                }
            }
            Block::List { items, .. } => {
                for item in items {
                    if let Some(pos) = item.children.iter().position(|c| *c == old_child) {
                        item.children.splice(pos..=pos, new_children.iter().copied());
                    }
                }
            }
            _ => return Err(TransformError::InvalidParent(parent_id)),
        }
        Ok(())
    }
}
