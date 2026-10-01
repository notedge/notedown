use super::*;

impl Display for NotedownType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NotedownType::Null => f.write_str("Null"),
            NotedownType::Boolean => f.write_str("Boolean"),
            NotedownType::Integer => f.write_str("Integer"),
            NotedownType::Decimal => f.write_str("Decimal"),
            NotedownType::String => f.write_str("String"),
            NotedownType::Set(v) => {
                if v.is_empty() {
                    return f.write_str("Set<?>");
                }
                let t = v.iter().map(|e| e.to_string()).collect_vec().join(" | ");
                write!(f, "Set<{}, {}>", t, v.len())
            }
            NotedownType::List(v) => {
                if v.is_empty() {
                    return f.write_str("List<?>");
                }
                let t = v.iter().map(|e| e.to_string()).join(" | ");
                write!(f, "List<{}, {}>", t, v.len())
            }
            NotedownType::Object(_) => {
                unimplemented!()
            }
        }
    }
}
