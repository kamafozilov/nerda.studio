//! Web addresses written out bare, made links: an agent writes
//! `https://…` in running text as often as it writes a Markdown link.

use super::Inline;

/// Makes each bare `http://` or `https://` address in `inline` a link to
/// itself, outside code and the links it has. Punctuation that ends a
/// sentence, and a closing bracket without its opening one, stay out.
pub fn find_bare(inline: &mut Inline) {
    let text = &inline.text;
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = next_scheme(&text[from..]).map(|at| from + at) {
        let rest = &text[at..];
        let mut end = at
            + rest
                .find(|c: char| c.is_whitespace() || "<>\"`".contains(c))
                .unwrap_or(rest.len());
        end = trimmed(&text[at..end]) + at;
        let starts_word = text[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let covered = (inline.spans.iter())
            .filter(|(_, style)| style.code)
            .map(|(range, _)| range)
            .chain(inline.links.iter().map(|(range, _)| range))
            .any(|range| range.start < end && at < range.end);
        let scheme = if rest.starts_with("https://") { 8 } else { 7 };
        if starts_word && !covered && end > at + scheme {
            found.push((at..end, text[at..end].to_owned()));
        }
        from = end.max(at + scheme);
    }
    inline.links.extend(found);
    inline.links.sort_by_key(|(range, _)| range.start);
}

fn next_scheme(text: &str) -> Option<usize> {
    let http = text.find("http://");
    let https = text.find("https://");
    match (http, https) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// How much of `address` is the address: without the punctuation after it,
/// or a closing bracket it doesn't open.
fn trimmed(address: &str) -> usize {
    let mut end = address.len();
    loop {
        let Some(last) = address[..end].chars().next_back() else {
            return end;
        };
        let unbalanced = |open: char, close: char| {
            last == close
                && address[..end].matches(open).count() < address[..end].matches(close).count()
        };
        if ".,;:!?'\"*_".contains(last) || unbalanced('(', ')') || unbalanced('[', ']') {
            end -= last.len_utf8();
        } else {
            return end;
        }
    }
}
