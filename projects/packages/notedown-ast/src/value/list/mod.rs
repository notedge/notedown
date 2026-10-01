use super::*;
mod iter;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct List<T> {
    inner: Box<Vec<NodeLocation<T>>>,
}

impl<T> List<T> {
    #[inline]
    pub fn push(&mut self, value: NodeLocation<T>) {
        self.inner.push(value);
    }
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}
