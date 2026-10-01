use percent_encoding::{percent_decode, percent_encode, AsciiSet, CONTROLS};

const NON_URL: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'-')
    //  .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    //  .add(b'?')
    .add(b'↯')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'_')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b'~');

/// url_encode
pub fn url_encode(text: impl AsRef<str>) -> String {
    percent_encode(text.as_ref().as_bytes(), NON_URL).to_string()
}

/// url_decode
pub fn url_decode(text: impl AsRef<str>) -> Option<String> {
    match percent_decode(text.as_ref().as_bytes()).decode_utf8() {
        Ok(cow) => Some(cow.to_string()),
        Err(_) => None,
    }
}
// pub fn html_encode() {
//
// }
//
// pub fn html_decode() {
//
// }
