//! A file or folder the agent names: in inline code, as `src/app.rs:12`, or
//! as a link's target, as `[app.rs](/repo/src/app.rs#L12)`. A path counts
//! when it has a folder in it and its last part an extension, or it ends
//! in `/`, or it carries a line; anything with spaces, a web address, or
//! code-like characters doesn't.

/// A path the text names, with the line and column it points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    pub path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub folder: bool,
}

impl Mention {
    /// The file's or folder's own name.
    pub fn name(&self) -> &str {
        let path = self.path.trim_end_matches(['/', '\\']);
        path.rsplit(['/', '\\']).next().unwrap_or(path)
    }

    /// The folder it is in, by its own name, when it has one.
    pub fn parent(&self) -> Option<&str> {
        let path = self.path.trim_end_matches(['/', '\\']);
        let (parent, _) = path.rsplit_once(['/', '\\'])?;
        let name = parent.rsplit(['/', '\\']).next()?;
        (!name.is_empty() && name != "." && name != "~").then_some(name)
    }

    /// Where in the file it points, as `L12` or `L12:C3`.
    pub fn place(&self) -> Option<String> {
        match (self.line, self.column) {
            (Some(line), Some(column)) => Some(format!("L{line}:C{column}")),
            (Some(line), None) => Some(format!("L{line}")),
            _ => None,
        }
    }
}

/// The path `text` names, if it names one.
pub fn mention(text: &str) -> Option<Mention> {
    let text = text.trim();
    let text = text.strip_prefix("file://").unwrap_or(text);
    if text.is_empty()
        || text.contains("://")
        || text.starts_with('-')
        || text
            .chars()
            .any(|c| c.is_whitespace() || "()[]{}<>*$|=,;\"'`!?&^%@".contains(c))
    {
        return None;
    }
    let (path, line, column) = place(text);
    let folder = path.ends_with('/') || path.ends_with('\\');
    let separated = path.trim_end_matches(['/', '\\']).contains(['/', '\\']);
    let named = Mention {
        path: path.to_owned(),
        line,
        column,
        folder,
    };
    let extension = has_extension(named.name());
    let counts = (separated && (extension || folder)) || (line.is_some() && extension);
    (counts && named.name().chars().any(char::is_alphabetic)).then_some(named)
}

/// `text` without the place it points at, and that place: `#L12`,
/// `#L12-L20`, `:12`, `:12:3` or `:12-20`.
fn place(text: &str) -> (&str, Option<u32>, Option<u32>) {
    if let Some((path, anchor)) = text.rsplit_once("#L") {
        let line = anchor
            .split(['-', 'C', ':'])
            .next()
            .and_then(|n| n.parse().ok());
        return (path, line, None);
    }
    let mut path = text;
    let mut numbers = Vec::new();
    while let Some((before, after)) = path.rsplit_once(':') {
        let first = after.split('-').next().unwrap_or(after);
        match first.parse::<u32>() {
            Ok(number) if numbers.len() < 2 => {
                numbers.push(number);
                path = before;
            }
            _ => break,
        }
    }
    numbers.reverse();
    (path, numbers.first().copied(), numbers.get(1).copied())
}

/// Whether a file's name ends in an extension: letters or digits after
/// its last dot, not only digits, as a version's are. A dotfile's whole
/// name counts.
fn has_extension(name: &str) -> bool {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    !extension.is_empty()
        && extension.len() <= 12
        && extension.chars().all(|c| c.is_ascii_alphanumeric())
        && !extension.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Option<(String, Option<u32>, Option<u32>, bool)> {
        mention(text).map(|m| (m.path, m.line, m.column, m.folder))
    }

    #[test]
    fn paths_with_a_folder_and_an_extension_count() {
        let some = |path: &str, line, column, folder| Some((path.to_owned(), line, column, folder));
        assert_eq!(at("src/app.rs"), some("src/app.rs", None, None, false));
        assert_eq!(
            at("src/app.rs:12"),
            some("src/app.rs", Some(12), None, false)
        );
        assert_eq!(
            at("src/app.rs:12:3"),
            some("src/app.rs", Some(12), Some(3), false)
        );
        assert_eq!(
            at("/repo/src/app.rs#L40-L52"),
            some("/repo/src/app.rs", Some(40), None, false)
        );
        assert_eq!(at("docs/adr/"), some("docs/adr/", None, None, true));
        assert_eq!(at("~/.zshrc"), some("~/.zshrc", None, None, false));
        assert_eq!(at("README.md:3"), some("README.md", Some(3), None, false));
        assert_eq!(
            at("C:\\work\\main.cs"),
            some("C:\\work\\main.cs", None, None, false)
        );
        assert_eq!(
            at("file:///tmp/a.txt"),
            some("/tmp/a.txt", None, None, false)
        );
    }

    #[test]
    fn code_and_words_dont() {
        for text in [
            "README.md",
            "Arcada.Backend.Identity",
            "and/or",
            "client/server",
            "https://x.io/a.js",
            "cargo run -p dev",
            "--for",
            "a.b/1.2.3",
            "fn(a/b.c)",
            "v1.2/3.4",
            "x = a/b.c",
            "",
        ] {
            assert_eq!(at(text), None, "{text}");
        }
    }

    #[test]
    fn a_mention_has_its_name_parent_and_place() {
        let Some(mention) = mention("crates/studio/src/main.rs:12:3") else {
            panic!();
        };
        assert_eq!(mention.name(), "main.rs");
        assert_eq!(mention.parent(), Some("src"));
        assert_eq!(mention.place().as_deref(), Some("L12:C3"));
        let folder = super::mention("docs/adr/").unwrap();
        assert_eq!((folder.name(), folder.parent()), ("adr", Some("docs")));
    }
}
