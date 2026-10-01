use std::slice::Iter;

use super::*;

impl<'i, T> IntoIterator for &'i List<T> {
    type Item = &'i NodeLocation<T>;
    type IntoIter = Iter<'i, NodeLocation<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}
