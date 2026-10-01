use crate::{Result, TextError};
use std::intrinsics::transmute;

/// Takes in a string with backslash escapes written out with literal backslash characters and
/// converts it to a string with the proper escaped characters.
///
/// | Escape | Unicode | Description                    |
///  | ------ | ------- | ------------------------------ |
///  | \b     | \u{08}  | Backspace                      |
///  | \v     | \u{0B}  | Vertical tab                   |
///  | \f     | \u{0C}  | Form feed                      |
///  | \n     | \u{0A}  | Newline                        |
///  | \r     | \u{0D}  | Carriage return                |
///  | \t     | \u{09}  | Tab                            |
///  | \\     | \u{5C}  | Backslash                      |
///  | \'     | \u{27}  | Single quote                   |
///  | \"     | \u{22}  | Double quote                   |
///  | \$     | \u{24}  | Dollar sign (sh compatibility) |
///  | \`     | \u{60}  | Backtick (sh compatibility)    |
///  | other  | self    | Just remove `\`                |
pub fn unescape(text: impl AsRef<str>) -> Result<String> {
    let text = text.as_ref();
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().enumerate();
    while let Some((index, c)) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        if let Some(next) = chars.next() {
            match escape_chars(next.1) {
                Some(c) => out.push(c),
                None => {
                    return TextError::unescape_error(index, format!("\\{}", next.1));
                }
            }
        }
    }
    Ok(out)
}

/// unchecked version of unescape
///
///
/// ### Safety
///
/// transmute unescape_unicode_char
pub unsafe fn unescape_unchecked(text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        if let Some(next) = chars.next() {
            match escape_chars(next) {
                Some(c) => out.push(c),
                None => out.push(next),
            }
        }
    }
    return out;
}

fn escape_chars(c: char) -> Option<char> {
    match c {
        'b' => Some('\u{0008}'),
        'f' => Some('\u{000C}'),
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        '\'' => Some('\''),
        '\"' => Some('\"'),
        '\\' => Some('\\'),
        _ => None,
    }
}

/// unescape_utf8
#[allow(unused_variables)]
pub fn unescape_utf8(text: impl AsRef<str>) {
    unimplemented!()
}
/// unescape_only
#[allow(unused_variables)]
pub fn unescape_only(text: impl AsRef<str>, c: char) {
    unimplemented!()
}

/// unescape \U{xx xx xx}
pub fn unescape_hex_chars(text: impl AsRef<str>) -> Option<String> {
    unsafe { unescape_unicode_char(text.as_ref(), 16) }
}

/// unescape \u{xx xx xx}
pub fn unescape_dec_chars(text: impl AsRef<str>) -> Option<String> {
    unsafe { unescape_unicode_char(text.as_ref(), 10) }
}

unsafe fn unescape_unicode_char(text: &str, radix: u32) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    for c in text.split_whitespace() {
        match u32::from_str_radix(c, radix) {
            Ok(o) => out.push(transmute::<u32, char>(o)),
            Err(_) => return None,
        }
    }
    Some(out)
}
