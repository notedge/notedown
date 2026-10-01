mod add;
mod index;
mod mul;

use crate::NotedownValue;
use diagnostic_quick::{QError, QResult};
use num::{BigInt, Signed, ToPrimitive};
use std::ops::{Add, Div, Mul, Neg, Shr, Sub};

fn fail_int2dec(n: BigInt) -> QResult<NotedownValue> {
    Err(QError::runtime_error(format!("Can not convert `Integer` {} to `Decimal`", n)))
}
