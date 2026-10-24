//! A run of inline Markdown as text: its styled runs by weight, slant, a
//! line through or the monospaced font on a wash, its web links in the
//! link color and opening in the browser, and the parts of its end still
//! streaming in faded.

use std::ops::Range;

use gpui::{
    AnyElement, ElementId, FontStyle, FontWeight, HighlightStyle, InteractiveText, IntoElement,
    SharedString, StrikethroughStyle, StyledText,
};
use theme::{Colors, HAIRLINE, MONOSPACE_FONT};

use super::Tail;
use crate::{background, conversation::markdown::Inline};

/// `inline` as an element: text, or text whose web links open on a click.
pub fn text(inline: &Inline, id: ElementId, tail: &Tail, colors: Colors) -> AnyElement {
    let web: Vec<(Range<usize>, String)> = (inline.links.iter())
        .filter(|(_, target)| opens(target))
        .cloned()
        .collect();
    let styled = faded(inline, tail, Some(colors));
    if web.is_empty() {
        return styled.into_any_element();
    }
    let ranges = web.iter().map(|(range, _)| range.clone()).collect();
    InteractiveText::new(id, styled)
        .on_click(ranges, move |at, _, cx| {
            if let Some((_, target)) = web.get(at)
                && background::allows_system_ui("A link")
            {
                cx.open_url(target);
            }
        })
        .into_any_element()
}

/// Whether a link opens: a web or mail address does; a path, which the
/// agent means in its own folder, doesn't yet.
fn opens(target: &str) -> bool {
    ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| target.starts_with(scheme))
}

/// The text with its runs styled, in the color and size around it, and the
/// parts of its end in `tail` faded as far as they have yet to come in.
/// With `colors`, inline code sits on a wash and web links take the link
/// color; without, as in a problem line, only the font sets them apart.
pub fn faded(inline: &Inline, tail: &Tail, colors: Option<Colors>) -> StyledText {
    let links: Vec<&Range<usize>> = match colors {
        Some(_) => (inline.links.iter())
            .filter(|(_, target)| opens(target))
            .map(|(range, _)| range)
            .collect(),
        None => Vec::new(),
    };
    let mut cuts: Vec<usize> = vec![0, inline.text.len()];
    let ranges = (inline.spans.iter().map(|(range, _)| range)).chain(links.iter().copied());
    for range in ranges {
        cuts.extend([range.start, range.end]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let within = |range: &Range<usize>, start: usize| range.start <= start && start < range.end;
    let styles = cuts.windows(2).filter_map(|cut| {
        let (start, end) = (cut[0], cut[1]);
        let style = (inline.spans.iter())
            .find(|(range, _)| within(range, start))
            .map(|(_, style)| *style)
            .unwrap_or_default();
        let link = links.iter().any(|range| within(range, start));
        let highlight = HighlightStyle {
            color: colors.filter(|_| link).map(|colors| colors.link),
            font_weight: style.bold.then_some(FontWeight::BOLD),
            font_style: style.italic.then_some(FontStyle::Italic),
            background_color: (colors.filter(|_| style.code)).map(|colors| colors.field_background),
            strikethrough: style.strike.then_some(StrikethroughStyle {
                thickness: HAIRLINE,
                color: None,
            }),
            ..HighlightStyle::default()
        };
        (highlight != HighlightStyle::default()).then_some((start..end, highlight))
    });
    let monospaced = (inline.spans.iter())
        .filter(|(_, style)| style.code)
        .map(|(range, _)| (range.clone(), SharedString::new_static(MONOSPACE_FONT)));
    with_fades(&inline.text, styles.collect(), tail)
        .with_font_family_overrides(monospaced.collect::<Vec<_>>())
}

/// `text` with `styles`, their ranges in order and apart, and the parts of
/// its end in `tail` faded as far as they have yet to come in. The styles
/// are cut where a fade starts or stops, so each part keeps its own.
pub fn with_fades(
    text: &str,
    styles: Vec<(Range<usize>, HighlightStyle)>,
    tail: &Tail,
) -> StyledText {
    let length = text.len();
    let boundary = |from_end: usize| {
        let mut at = length.saturating_sub(from_end);
        while !text.is_char_boundary(at) {
            at += 1;
        }
        at
    };
    let fades: Vec<(Range<usize>, f32)> = (tail.iter())
        .map(|&(far, near, amount)| (boundary(far)..boundary(near), amount))
        .filter(|(range, amount)| !range.is_empty() && *amount < 1.)
        .collect();
    let Some(from) = fades.iter().map(|(range, _)| range.start).min() else {
        return StyledText::new(text.to_owned()).with_highlights(styles);
    };
    // Only the styles that reach the fading end are cut.
    let mut highlights = Vec::with_capacity(styles.len() + fades.len() * 2);
    let mut later = Vec::new();
    for (range, style) in styles {
        if range.end <= from {
            highlights.push((range, style));
        } else if range.start < from {
            highlights.push((range.start..from, style));
            later.push((from..range.end, style));
        } else {
            later.push((range, style));
        }
    }
    let mut cuts: Vec<usize> = vec![from, length];
    for range in (later.iter().map(|(range, _)| range)).chain(fades.iter().map(|(range, _)| range))
    {
        cuts.extend([range.start, range.end]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    for cut in cuts.windows(2) {
        let (start, end) = (cut[0], cut[1]);
        let mut style = (later.iter())
            .find(|(range, _)| range.start <= start && start < range.end)
            .map(|(_, style)| *style)
            .unwrap_or_default();
        let shown: f32 = (fades.iter())
            .filter(|(range, _)| range.start <= start && start < range.end)
            .map(|(_, amount)| *amount)
            .product();
        if shown < 1. {
            style.fade_out = Some(1. - shown);
        }
        if style != HighlightStyle::default() {
            highlights.push((start..end, style));
        }
    }
    StyledText::new(text.to_owned()).with_highlights(highlights)
}

/// A line of inline Markdown, such as a problem's words with a command in
/// backticks.
pub fn line(text: &str) -> StyledText {
    faded(&crate::conversation::markdown::inline(text), &[], None)
}
