use super::*;

/// Type of [`NotedownValue`]
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum NotedownType {
    /// It doesn't look like anything to me
    Null,
    /// `true` or `false`
    Boolean,
    /// Arbitrarily large integer
    Integer,
    /// 128-bit fixed point decimal
    Decimal,
    /// A UTF-8–encoded string
    String,
    /// Ordered set of values
    Set(BTreeSet<NotedownType>),
    /// Array of values
    List(BTreeSet<NotedownType>),
    /// Ordered map of key value pairs
    Object(BTreeMap<String, NotedownType>),
}

impl NotedownValue {
    /// get type of the value
    pub fn get_type(&self) -> NotedownType {
        match self {
            Self::Null => NotedownType::Null,
            Self::Boolean(_) => NotedownType::Boolean,
            Self::Integer(_) => NotedownType::Integer,
            Self::Decimal(_) => NotedownType::Decimal,
            Self::String(_) => NotedownType::String,
            Self::Array(v) => self.check_list_type(v),
            Self::Object(v) => self.check_dict_type(v),
        }
    }
    /// get type name of the value
    pub fn get_type_name(&self) -> String {
        self.get_type().to_string()
    }
    fn check_set_type(&self, input: &Vec<NotedownValue>) -> NotedownType {
        let mut count = BTreeSet::new();
        for v in input {
            count.insert(v.get_type());
        }
        NotedownType::Set(count)
    }
    fn check_list_type(&self, input: &List<NotedownValue>) -> NotedownType {
        let mut count = BTreeSet::new();
        for v in input {
            count.insert(v.get_type());
        }
        NotedownType::List(count)
    }
    fn check_dict_type(&self, input: &Dict<NotedownValue>) -> NotedownType {
        // let input: BTreeMap<String, Literal<Value>>;
        let mut count = BTreeMap::new();
        for (k, v) in input {
            count.insert(k.value.to_owned(), v.value.get_type());
        }
        NotedownType::Object(count)
    }
}
