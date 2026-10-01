use text_utils::{dedent, dedent_less_than, indent, indent_with, unescape, unescape_dec_chars, unescape_hex_chars, unescape_unchecked};

#[test]
fn test_dedent() {
    const INPUT: &str = "
 1
  2
   3
    4
";
    const OUTPUT: &str = "\n1\n 2\n  3\n   4\n";
    debug_assert_eq!(dedent(INPUT), OUTPUT)
}

#[test]
fn test_dedent_less_than() {
    const INPUT: &str = "
 1
  2
   3
    4
";
    const OUTPUT: &str = "\n1\n2\n 3\n  4";
    debug_assert_eq!(dedent_less_than(INPUT, 2), OUTPUT)
}

#[test]
fn test_indent() {
    const INPUT: &str = "
 1
  2
   3
    4
";
    const OUTPUT: &str = "\n     1\n      2\n       3\n        4\n";
    debug_assert_eq!(indent(INPUT, 4), OUTPUT)
}

#[test]
fn test_indent_with() {
    const INPUT: &str = "
 1
  2
   3
    4
";
    const OUTPUT: &str = ">\n>  1\n>   2\n>    3\n>     4\n";
    debug_assert_eq!(indent_with(INPUT, "> "), OUTPUT)
}

#[test]
fn unescape_chars() {
    println!("{}", unescape_dec_chars("56 53").unwrap_or_default());
    println!("{}", unescape_hex_chars("56 53").unwrap_or_default());
}

#[test]
fn unescape_ascii() {
    const INPUT: &str = "\\b\\c";

    println!("{:?}", unescape(INPUT));
    println!("{:?}", unsafe { unescape_unchecked(INPUT) });
}
