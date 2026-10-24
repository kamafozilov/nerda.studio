//! A code block's colors: its fence's language parsed by its grammar, each
//! token given a role (keyword, string, comment…) that the theme colors in
//! the appearance on screen. A block still streaming is parsed a line at a
//! time: each line that ended is parsed once, the parser's state carried to
//! the next, and only the line still coming is parsed again.

use std::{collections::HashMap, ops::Range, rc::Rc, str::FromStr, sync::OnceLock};

use syntect::{
    highlighting::{
        Color, HighlightState, Highlighter, RangedHighlightIterator, ScopeSelectors, StyleModifier,
        Theme, ThemeItem, ThemeSettings,
    },
    parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet},
};

/// What a token is, for its color; text without a role keeps the body's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Keyword,
    String,
    /// Numbers, `true`, constants.
    Constant,
    Function,
    Type,
    Comment,
    /// A diff's added and removed lines.
    Added,
    Removed,
}

/// The tokens of a code block that have a role, by byte range.
pub type Runs = Rc<[(Range<usize>, Role)]>;

/// Each code block's colors, by the block's id, kept as it streams.
#[derive(Default)]
pub struct Cache {
    blocks: HashMap<u64, Block>,
}

struct Block {
    syntax: usize,
    /// The lines parsed for good: the text up to the end of the last line
    /// that ended, and the parser's state after it.
    done: String,
    parse: ParseState,
    highlight: HighlightState,
    runs: Vec<(Range<usize>, Role)>,
    /// What was last handed out, with the text it was for.
    last: Option<(usize, Runs)>,
}

impl Cache {
    /// The colors of block `id`, `text` in `language`; none when no grammar
    /// knows the language.
    pub fn runs(&mut self, id: u64, language: Option<&str>, text: &str) -> Option<Runs> {
        let set = syntaxes();
        let syntax = find(set, language?)?;
        let index = set
            .syntaxes()
            .iter()
            .position(|each| std::ptr::eq(each, syntax))?;
        let block = match self.blocks.get_mut(&id) {
            Some(block) if block.syntax == index && text.starts_with(&block.done) => block,
            _ => {
                self.blocks.insert(id, Block::new(index, syntax));
                self.blocks.get_mut(&id)?
            }
        };
        if let Some((length, runs)) = &block.last
            && *length == text.len()
        {
            return Some(runs.clone());
        }
        let highlighter = Highlighter::new(theme());
        // Each line that ended since, for good.
        while let Some(end) = text[block.done.len()..].find('\n') {
            let start = block.done.len();
            let line = &text[start..start + end + 1];
            let runs = line_runs(line, &mut block.parse, &mut block.highlight, &highlighter);
            block.runs.extend(shifted(runs, start));
            block.done.push_str(line);
        }
        let mut runs = block.runs.clone();
        let start = block.done.len();
        if start < text.len() {
            let (mut parse, mut highlight) = (block.parse.clone(), block.highlight.clone());
            let line = &text[start..];
            runs.extend(shifted(
                line_runs(line, &mut parse, &mut highlight, &highlighter),
                start,
            ));
        }
        let runs: Runs = runs.into();
        block.last = Some((text.len(), runs.clone()));
        Some(runs)
    }
}

impl Block {
    fn new(index: usize, syntax: &SyntaxReference) -> Self {
        let highlighter = Highlighter::new(theme());
        Self {
            syntax: index,
            done: String::new(),
            parse: ParseState::new(syntax),
            highlight: HighlightState::new(&highlighter, ScopeStack::new()),
            runs: Vec::new(),
            last: None,
        }
    }
}

fn shifted(
    runs: Vec<(Range<usize>, Role)>,
    by: usize,
) -> impl Iterator<Item = (Range<usize>, Role)> {
    (runs.into_iter()).map(move |(range, role)| (range.start + by..range.end + by, role))
}

/// One line's tokens with a role; a line the grammar fails on has none.
fn line_runs(
    line: &str,
    parse: &mut ParseState,
    highlight: &mut HighlightState,
    highlighter: &Highlighter,
) -> Vec<(Range<usize>, Role)> {
    let Ok(ops) = parse.parse_line(line, syntaxes()) else {
        return Vec::new();
    };
    let mut runs: Vec<(Range<usize>, Role)> = Vec::new();
    for (style, _, range) in RangedHighlightIterator::new(highlight, &ops, line, highlighter) {
        let Some(role) = role(style.foreground) else {
            continue;
        };
        match runs.last_mut() {
            Some((last, same)) if *same == role && last.end == range.start => last.end = range.end,
            _ => runs.push((range, role)),
        }
    }
    runs
}

/// Every grammar, read once, on first use.
fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

/// Reads the grammars on a thread of their own, so the first code block
/// doesn't wait for them.
pub fn prewarm() {
    std::thread::spawn(|| {
        syntaxes();
        theme();
    });
}

/// The grammar for a fence's language, by its name or a file extension,
/// and the other names agents give some.
fn find<'a>(set: &'a SyntaxSet, language: &str) -> Option<&'a SyntaxReference> {
    let language = language.to_ascii_lowercase();
    let token = match language.as_str() {
        "shell" | "console" | "shellsession" | "sh-session" | "terminal" => "bash",
        "jsonc" | "json5" => "json",
        "c#" | "csharp" => "cs",
        "f#" | "fsharp" => "fs",
        "objective-c" | "objc" => "m",
        "patch" => "diff",
        "text" | "txt" | "plain" | "plaintext" => return None,
        other => other,
    };
    set.find_syntax_by_token(token)
}

/// The scopes each role colors. A token takes the most specific selector
/// that matches it, so operators and punctuation stay plain.
const SELECTORS: [(&str, Option<Role>); 12] = [
    (
        "comment, punctuation.definition.comment",
        Some(Role::Comment),
    ),
    (
        "string, constant.character.escape, string.regexp",
        Some(Role::String),
    ),
    (
        "constant.numeric, constant.language, constant.character, constant.other, support.constant",
        Some(Role::Constant),
    ),
    (
        "keyword, storage, variable.language, entity.name.tag",
        Some(Role::Keyword),
    ),
    (
        "keyword.operator, punctuation.separator, punctuation.terminator",
        None,
    ),
    (
        "entity.name.function, support.function, variable.function, meta.function-call.identifier",
        Some(Role::Function),
    ),
    (
        "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, \
         entity.name.trait, entity.name.interface, support.type, support.class, \
         entity.other.inherited-class, storage.type.primitive",
        Some(Role::Type),
    ),
    (
        "entity.other.attribute-name, support.type.property-name, meta.mapping.key string",
        Some(Role::Function),
    ),
    ("markup.heading, markup.bold", Some(Role::Keyword)),
    (
        "markup.inserted, meta.diff.header.to-file",
        Some(Role::Added),
    ),
    (
        "markup.deleted, meta.diff.header.from-file",
        Some(Role::Removed),
    ),
    ("meta.diff.range, markup.changed", Some(Role::Constant)),
];

const ROLES: [Role; 8] = [
    Role::Keyword,
    Role::String,
    Role::Constant,
    Role::Function,
    Role::Type,
    Role::Comment,
    Role::Added,
    Role::Removed,
];

/// The theme the grammars' scopes go through: each role's "color" is its
/// number, so the colors themselves follow the appearance at drawing time.
fn theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| Theme {
        settings: ThemeSettings {
            foreground: Some(code(None)),
            ..ThemeSettings::default()
        },
        scopes: (SELECTORS.iter())
            .filter_map(|(selector, role)| {
                Some(ThemeItem {
                    scope: ScopeSelectors::from_str(selector).ok()?,
                    style: StyleModifier {
                        foreground: Some(code(*role)),
                        ..StyleModifier::default()
                    },
                })
            })
            .collect(),
        ..Theme::default()
    })
}

fn code(role: Option<Role>) -> Color {
    let number = role.and_then(|role| ROLES.iter().position(|each| *each == role));
    Color {
        r: number.map_or(0, |at| at as u8 + 1),
        g: 0,
        b: 0,
        a: 255,
    }
}

fn role(color: Color) -> Option<Role> {
    ROLES.get((color.r as usize).checked_sub(1)?).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(language: &str, text: &str) -> Vec<(String, Role)> {
        let runs = Cache::default()
            .runs(1, Some(language), text)
            .expect("a grammar");
        (runs.iter())
            .map(|(range, role)| (text[range.clone()].to_owned(), *role))
            .collect()
    }

    #[test]
    fn tokens_take_their_roles() {
        let rust = tokens("rust", "// note\nfn main() { let a = \"hi\"; 42 }\n");
        assert!(
            rust.contains(&("// note\n".to_owned(), Role::Comment)),
            "{rust:?}"
        );
        assert!(rust.contains(&("fn".to_owned(), Role::Keyword)), "{rust:?}");
        assert!(
            rust.contains(&("main".to_owned(), Role::Function)),
            "{rust:?}"
        );
        assert!(
            rust.contains(&("\"hi\"".to_owned(), Role::String)),
            "{rust:?}"
        );
        assert!(
            rust.contains(&("42".to_owned(), Role::Constant)),
            "{rust:?}"
        );
        assert!(!rust.iter().any(|(text, _)| text.contains('=')), "{rust:?}");
        let ts = tokens("tsx", "const a: number = 1;");
        assert!(ts.contains(&("const".to_owned(), Role::Keyword)), "{ts:?}");
        let diff = tokens("diff", "-old\n+new\n");
        assert_eq!(
            diff,
            [
                ("-old\n".to_owned(), Role::Removed),
                ("+new\n".to_owned(), Role::Added)
            ]
        );
    }

    #[test]
    fn an_unknown_language_has_no_colors() {
        let mut cache = Cache::default();
        assert!(cache.runs(1, Some("nosuchlanguage"), "text").is_none());
        assert!(cache.runs(1, Some("text"), "text").is_none());
        assert!(cache.runs(1, None, "text").is_none());
        assert!(cache.runs(1, Some("Shell"), "echo hi").is_some());
    }

    #[test]
    fn streaming_gives_what_a_whole_parse_does() {
        let text = "/* a\n b */ let s = `x\n${1}`;\nfunction f() {}\n";
        let whole = Cache::default()
            .runs(1, Some("js"), text)
            .expect("a grammar");
        let mut cache = Cache::default();
        let mut streamed = None;
        for end in (1..=text.len()).filter(|end| text.is_char_boundary(*end)) {
            streamed = cache.runs(1, Some("js"), &text[..end]);
        }
        assert_eq!(streamed.as_deref(), Some(&whole[..]));
        // A block that changed under its parsed lines starts over.
        let again = cache.runs(1, Some("js"), "let b = 2;").expect("a grammar");
        assert!(again.iter().all(|(range, _)| range.end <= 10));
    }
}
