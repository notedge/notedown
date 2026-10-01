use super::*;

pub trait Index<I> {
    type Output;
    fn get_index(&self, index: &I) -> Self::Output;
}

impl Index<BigInt> for NotedownValue {
    type Output = QResult<Self>;

    fn get_index(&self, index: &BigInt) -> Self::Output {
        match self {
            Self::Null | Self::Boolean(_) | Self::Integer(_) | Self::Decimal(_) => {
                Err(QError::runtime_error(format!("Can not take `Integer` index of type `{}`", self.get_type_name())))
            }
            Self::String(s) => {
                let i = if index.is_negative() {
                    match index.neg().to_usize() {
                        Some(u) => {
                            let max = s.chars().count();
                            // (u <= max).then_some(max);
                            if u <= max { Some(max) } else { None }
                        }
                        None => None,
                    }
                }
                else {
                    index.to_usize()
                };

                match i.and_then(|e| s.chars().nth(e)) {
                    Some(s) => Ok(NotedownValue::string(s)),
                    None => Err(QError::runtime_error(format!("Index `{}` of `String` out of range.", index))),
                }
            }
            Self::Set(v) => {
                // let v : BTreeSet<Literal<Value>>;
                let out = if index.is_negative() {
                    index.neg().to_usize().and_then(|i| v.iter().rev().nth(i))
                }
                else {
                    index.to_usize().and_then(|i| v.iter().nth(i))
                };
                match out {
                    Some(s) => Ok(s.to_owned()),
                    None => Err(QError::runtime_error(format!("Index `{}` of `Set` out of range.", index))),
                }
            }
            Self::Array(v) => {
                let out = if index.is_negative() {
                    match v.last_key_value() {
                        None => None,
                        Some((max, _)) => match index.neg().to_biguint() {
                            None => None,
                            Some(i) => {
                                if &i < max {
                                    v.get(&(max - i))
                                }
                                else {
                                    None
                                }
                            }
                        },
                    }
                }
                else {
                    index.to_biguint().and_then(|i| v.get(&i))
                };
                match out {
                    Some(s) => Ok(s.to_owned()),
                    None => Err(QError::runtime_error(format!("Index `{}` of `Array` out of range.", index))),
                }
            }
            Self::Object(v) => {
                // let v : BTreeMap<String, Literal<Value>>;
                let out = if index.is_negative() {
                    index.neg().to_usize().and_then(|i| v.values().rev().nth(i))
                }
                else {
                    index.to_usize().and_then(|i| v.values().nth(i))
                };
                match out {
                    Some(s) => Ok(s.to_owned()),
                    None => Err(QError::runtime_error(format!("Index `{}` of `Object` out of range.", index))),
                }
            }
        }
    }
}

impl Index<String> for NotedownValue {
    type Output = QResult<Self>;

    fn get_index(&self, index: &String) -> Self::Output {
        match self {
            Self::Object(v) => {
                // let v : BTreeMap<String, Literal<Value>>;
                match v.get(index) {
                    Some(s) => Ok(s.to_owned()),
                    None => Err(QError::runtime_error(format!("Index `{}` of `Object` not found.", index))),
                }
            }
            _ => Err(QError::runtime_error(format!("Can not take `String` index of type `{}`", self.get_type_name()))),
        }
    }
}
