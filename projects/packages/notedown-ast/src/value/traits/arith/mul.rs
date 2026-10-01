use super::*;
use num::FromPrimitive;
use rust_decimal::Decimal;

impl Mul for NotedownValue {
    type Output = QResult<Self>;
    // a * b
    fn mul(self, other: Self) -> Self::Output {
        let msg = format!("Can not apply `*` on lhs: {}, rhs: {}", self.get_type_name(), other.get_type_name());
        let type_mismatch = Err(QError::runtime_error(msg));
        let out = match (self, other) {
            (Self::Integer(lhs), Self::Integer(rhs)) => Self::Integer(lhs * rhs),
            (Self::Integer(lhs), Self::Decimal(rhs)) | (Self::Decimal(rhs), Self::Integer(lhs)) => {
                match lhs.to_i128().and_then(|s| Decimal::from_i128(s)) {
                    Some(s) => Self::Decimal(s * rhs),
                    None => return fail_int2dec(lhs),
                }
            }
            (Self::Decimal(lhs), Self::Decimal(rhs)) => Self::Decimal(lhs * rhs),
            _ => return type_mismatch,
        };
        return Ok(out);
    }
}

impl Div for NotedownValue {
    type Output = QResult<Self>;

    fn div(self, other: Self) -> Self::Output {
        let msg = format!("Can not apply `/` on lhs: {}, rhs: {}", self.get_type_name(), other.get_type_name());
        let type_mismatch = Err(QError::runtime_error(msg));
        let out = match (self, other) {
            (Self::Integer(lhs), Self::Integer(rhs)) => Self::Integer(lhs / rhs),
            (Self::Integer(lhs), Self::Decimal(rhs)) | (Self::Decimal(rhs), Self::Integer(lhs)) => {
                match lhs.to_i128().and_then(|s| Decimal::from_i128(s)) {
                    Some(s) => Self::Decimal(s / rhs),
                    None => return fail_int2dec(lhs),
                }
            }
            (Self::Decimal(lhs), Self::Decimal(rhs)) => Self::Decimal(lhs / rhs),
            _ => return type_mismatch,
        };
        return Ok(out);
    }
}
