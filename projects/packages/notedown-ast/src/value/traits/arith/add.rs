use super::*;
use num::FromPrimitive;
use rust_decimal::Decimal;

impl Add for NotedownValue {
    type Output = QResult<Self>;

    /// a + b
    fn add(self, other: Self) -> Self::Output {
        let msg = format!("Can not apply `+` on lhs: {}, rhs: {}", self.get_type_name(), other.get_type_name());
        let type_mismatch = Err(QError::runtime_error(msg));

        let out = match (self, other) {
            (Self::String(lhs), Self::String(rhs)) => Self::String(lhs + &rhs),
            (Self::Integer(lhs), Self::Integer(rhs)) => Self::Integer(lhs + &rhs),
            (Self::Integer(int), Self::Decimal(dec)) | (Self::Decimal(dec), Self::Integer(int)) => {
                match int.to_i128().and_then(|s| Decimal::from_i128(s)) {
                    Some(s) => Self::Decimal(s + dec),
                    None => return fail_int2dec(int),
                }
            }
            (Self::Decimal(lhs), Self::Decimal(rhs)) => Self::Decimal(lhs + rhs),
            _ => return type_mismatch,
        };
        return Ok(out);
    }
}

impl Shr for NotedownValue {
    type Output = QResult<Self>;

    /// a ++ b
    fn shr(self, other: Self) -> Self::Output {
        let msg = format!("Can not apply `++` on lhs: {}, rhs: {}", self.get_type_name(), other.get_type_name());
        let type_mismatch = Err(QError::runtime_error(msg));
        let out = match (self, other) {
            (Self::String(lhs), rhs) => Self::string_join(lhs, rhs)?,
            _ => return type_mismatch,
        };
        return Ok(out);
    }
}

impl NotedownValue {
    /// join a value to the string
    pub fn string_join(lhs: String, other: NotedownValue) -> QResult<Self> {
        let msg = format!("Can not apply `++` on lhs: `String`, rhs: {}", other.get_type_name());
        let type_mismatch = Err(QError::runtime_error(msg));
        let out = match other {
            NotedownValue::Null => lhs,
            NotedownValue::Boolean(v) => {
                format!("{}{}", lhs, v)
            }
            NotedownValue::Integer(v) => {
                format!("{}{}", lhs, v)
            }
            NotedownValue::Decimal(v) => {
                format!("{}{}", lhs, v)
            }
            NotedownValue::String(v) => {
                format!("{}{}", lhs, v)
            }
            NotedownValue::Array(_) => {
                return type_mismatch;
            }
            NotedownValue::Object(_) => {
                return type_mismatch;
            }
        };
        Ok(NotedownValue::String(out))
    }
}

impl Sub for NotedownValue {
    type Output = QResult<Self>;
    /// a- b
    fn sub(self, other: Self) -> Self::Output {
        let msg = format!("Can not apply `-` on lhs: {}, rhs: {}", self.get_type_name(), other.get_type_name());
        let type_mismatch = Err(QError::runtime_error(msg));

        let out = match (self, other) {
            (Self::Integer(lhs), Self::Integer(rhs)) => Self::Integer(lhs - rhs),
            (Self::Integer(int), Self::Decimal(dec)) | (Self::Decimal(dec), Self::Integer(int)) => {
                match int.to_i128().and_then(|s| Decimal::from_i128(s)) {
                    Some(s) => Self::Decimal(s - dec),
                    None => return fail_int2dec(int),
                }
            }
            (Self::Decimal(lhs), Self::Decimal(rhs)) => Self::Decimal(lhs - rhs),
            _ => return type_mismatch,
        };
        return Ok(out);
    }
}
