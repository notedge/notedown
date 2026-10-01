use super::*;

mod iter;

#[derive(Clone, Default, Eq, PartialEq)]
pub struct Dict<T> {
    dict: Box<IndexMap<String, (NodeLocation<String>, NodeLocation<T>)>>,
}

impl<T> Debug for Dict<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.dict.iter().map(|(k, v)| (k.as_str(), &v.1))).finish()
    }
}

impl<T> Dict<T> {
    pub fn is_empty(&self) -> bool {
        self.dict.is_empty()
    }
    pub fn insert(&mut self, key: NodeLocation<String>, value: NodeLocation<T>) {
        self.dict.insert(key.value.clone(), (key, value));
    }
    pub fn get_key(&self, key: &str) -> Option<&NodeLocation<String>> {
        self.dict.get(key).map(|v| &v.0)
    }
    pub fn mut_key(&mut self, key: &str) -> Option<&mut NodeLocation<String>> {
        self.dict.get_mut(key).map(|v| &mut v.0)
    }
    pub fn get_value(&self, key: &str) -> Option<&NodeLocation<T>> {
        self.dict.get(key).map(|v| &v.1)
    }
    pub fn mut_value(&mut self, key: &str) -> Option<&mut NodeLocation<T>> {
        self.dict.get_mut(key).map(|v| &mut v.1)
    }
    pub fn keys(&self) -> impl Iterator<Item = &NodeLocation<String>> {
        self.dict.values().map(|v| &v.0)
    }
    pub fn values(&self) -> impl Iterator<Item = &NodeLocation<T>> {
        self.dict.values().map(|v| &v.1)
    }

    pub fn len(&self) -> usize {
        self.dict.len()
    }
    pub fn clear(&mut self) {
        self.dict.clear();
    }
}
