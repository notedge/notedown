pub struct ListNode {
    pub first_item_number: Option<usize>,
}

impl ListNode {
    pub fn ordered(&self) -> bool {
        self.first_item_number.is_some()
    }
}
