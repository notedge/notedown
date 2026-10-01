use indexmap::map::Values;

use super::*;

impl<'i, T> IntoIterator for &'i Dict<T> {
    type Item = &'i (NodeLocation<String>, NodeLocation<T>);
    type IntoIter = Values<'i, String, (NodeLocation<String>, NodeLocation<T>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.dict.values()
    }
}
