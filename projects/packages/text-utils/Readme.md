# Text Toolbox

**All designs only consider spaces and carriage returns, so sorry for `Tab` and `CRLF`.**

**All interfaces are function calls instead of traits.**

**Most interfaces return `String` instead of `Cow<str>`**

## Functions

### Escaping

- `unescape`:
- `unescape_utf8`:
- `unescape_only`:
- `url_encode`: encoding strings with `%`
- `url_decode`: decoding strings of `%`


### Align

- `indent`: adds spaces to each non-empty line
- `indent_with`: adds prefix to each non-empty line
- `indent_count`:
- `dedent`: removes leading whitespace from each line
- `dedent_less_than`: removes at most n leading whitespace from each line

### Table

-
-
