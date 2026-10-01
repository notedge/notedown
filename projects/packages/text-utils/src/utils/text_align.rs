/// Removes common leading whitespace from each line.
pub fn dedent(text: impl AsRef<str>) -> String {
    textwrap::dedent(text.as_ref())
}

/// Adds spaces to each non-empty line.
pub fn indent(text: impl AsRef<str>, space: usize) -> String {
    textwrap::indent(text.as_ref(), &" ".repeat(space))
}

/// Adds prefix to each non-empty line.
pub fn indent_with(text: impl AsRef<str>, prefix: &str) -> String {
    textwrap::indent(text.as_ref(), prefix)
}

/// Removes at most n leading whitespace from each line
pub fn dedent_less_than(text: impl AsRef<str>, max: usize) -> String {
    // https://stackoverflow.com/questions/60337455/how-to-trim-space-less-than-n-times
    text.as_ref()
        .lines()
        .map(|line| {
            let mut max = max;
            line.chars()
                // Skip while `c` is a whitespace and at most `max` spaces
                .skip_while(|c| {
                    if max == 0 {
                        false
                    }
                    else {
                        max -= 1;
                        c.is_whitespace()
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Calculate how much space the first line has
pub fn indent_count(text: impl AsRef<str>) -> usize {
    let mut spaces = 0;
    for c in text.as_ref().chars() {
        match c {
            ' ' => spaces += 1,
            _ => break,
        }
    }
    return spaces;
}
