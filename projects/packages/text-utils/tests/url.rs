use text_utils::{url_decode, url_encode};

#[test]
fn test_url_decode() {
    const INPUT: &str = "http://www.sandal.tw/upload/Adventures%20in%20Group%20Theory_%20Rubik's%20Cube,%20Merlin's%20Machine,%20and%20Other%20Mathematical%20Toys%20by%20David%20Joyner.pdf";
    const OUTPUT: &str = "http://www.sandal.tw/upload/Adventures in Group Theory_ Rubik's Cube, Merlin's Machine, and Other Mathematical Toys by David Joyner.pdf";
    debug_assert_eq!(url_decode(INPUT), Some(String::from(OUTPUT)))
}

#[test]
#[rustfmt::skip]
fn test_url_decode_2() {
    const INPUT: &str = "https://bbs.emath.ac.cn/forum.php?mod=viewthread&tid=246&extra=page%3D2%26filter%3Ddigest%26digest%3D1";
    const OUTPUT: &str = "https://bbs.emath.ac.cn/forum.php?mod=viewthread&tid=246&extra=page=2&filter=digest&digest=1";
    debug_assert_eq!(url_decode(INPUT), Some(String::from(OUTPUT)))
}

#[test]
fn test_url_decode_3() {
    const INPUT: &str = "https://zh.wikipedia.org/wiki/%E8%B7%B3%E8%B7%83%E5%88%97%E8%A1%A8";
    const OUTPUT: &str = "https://zh.wikipedia.org/wiki/跳跃列表";
    debug_assert_eq!(url_decode(INPUT), Some(String::from(OUTPUT)))
}

#[test]
fn test_url_encode_3() {
    const INPUT: &str = "https://zh.wikipedia.org/wiki/跳跃列表";
    const OUTPUT: &str = "https://zh.wikipedia.org/wiki/%E8%B7%B3%E8%B7%83%E5%88%97%E8%A1%A8";
    debug_assert_eq!(url_encode(INPUT), OUTPUT)
}
