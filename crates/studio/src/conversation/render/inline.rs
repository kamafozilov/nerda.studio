//! A run of inline Markdown as text: its styled runs by weight, slant or a
//! line through; inline code and the files it names as chips
//! ([`chips`](super::chips)); web links in the link color, opening in the
//! browser; and the parts of its end still streaming in faded.

use std::ops::Range;

use gpui::{
    AnyElement, AnyView, AppContext, Context, ElementId, FontStyle, FontWeight, HighlightStyle,
    InteractiveText, IntoElement, ParentElement, Render, SharedString, StrikethroughStyle, Styled,
    StyledText, Window, div,
};
use theme::{
    CONTROL_RADIUS, Colors, HAIRLINE, MONOSPACE_FONT, SPACE_1, SPACE_2, TEXT_SMALL, Theme,
};

use super::{
    Env, Tail,
    chip_paint::{Chipped, Paint},
    chips::{self, Chip, Kind},
};
use crate::{
    assets::Icon,
    background,
    conversation::{
        markdown::Inline,
        selection::{Piece, Target},
    },
};

/// `inline` as an element: its chips under it, its links and files opening
/// on a click, a file's whole path in its tooltip.
pub fn text(inline: &Inline, id: ElementId, tail: &Tail, env: &mut Env) -> AnyElement {
    let colors = env.colors;
    let (inline, chips) = chips::decorate(inline, env.folder.as_deref(), &env.crowded);
    let styled = faded(&inline, tail, Some(colors), &chips);
    let layout = styled.layout().clone();
    let mut targets: Vec<(Range<usize>, Target)> = (inline.links.iter())
        .filter(|(_, target)| opens(target))
        .map(|(range, target)| (range.clone(), Target::Web(target.clone())))
        .collect();
    let mut tips: Vec<(Range<usize>, SharedString)> = Vec::new();
    for chip in &chips {
        if let Kind::Mention { path, mention } = &chip.kind {
            targets.push((chip.range.clone(), Target::File(path.clone())));
            tips.push((chip.range.clone(), tip(path, mention.folder).into()));
        }
    }
    targets.sort_by_key(|(range, _)| range.start);
    let piece = Piece {
        text: inline.text.clone(),
        before: "",
        sources: (chips.iter())
            .map(|chip| (chip.range.clone(), chip.source.clone()))
            .collect(),
    };
    let select = env.piece(piece, targets.clone().into());
    let paint = (chips.iter())
        .map(|chip| match &chip.kind {
            Kind::Code => Paint {
                range: chip.range.clone(),
                fill: colors.field_background,
                border: colors.border,
                icon: None,
            },
            Kind::Mention { mention, .. } => Paint {
                range: chip.range.clone(),
                fill: colors.mention_background,
                border: colors.mention_border,
                icon: chip.icon.map(|at| {
                    let icon = if mention.folder {
                        Icon::Folder
                    } else {
                        Icon::File
                    };
                    (at, SharedString::new_static(icon.path()), colors.mention)
                }),
            },
        })
        .collect();
    let element = if targets.is_empty() {
        styled.into_any_element()
    } else {
        let ranges = targets.iter().map(|(range, _)| range.clone()).collect();
        InteractiveText::new(id, styled)
            .on_click(ranges, move |at, _, cx| match targets.get(at) {
                Some((_, Target::Web(target))) if background::allows_system_ui("A link") => {
                    cx.open_url(target)
                }
                Some((_, Target::File(path))) if background::allows_system_ui("A file") => {
                    cx.open_with_system(path)
                }
                _ => {}
            })
            .tooltip(move |at, _, cx| {
                let (_, text) = tips.iter().find(|(range, _)| range.contains(&at))?;
                let text = text.clone();
                Some(AnyView::from(cx.new(|_| PathTip { text })))
            })
            .into_any_element()
    };
    let chipped = Chipped::new(element, layout, paint);
    match select {
        Some(select) => chipped.selectable(select).into_any_element(),
        None => chipped.into_any_element(),
    }
}

/// A file's tooltip: its path from the home folder, and its size, or that
/// it isn't there.
fn tip(path: &std::path::Path, folder: bool) -> String {
    let shown = workspace::home_relative(path);
    match std::fs::metadata(path) {
        Ok(_) if folder => shown,
        Ok(metadata) => format!("{shown} · {}", size(metadata.len())),
        Err(_) => format!("{shown} · Not found"),
    }
}

fn size(bytes: u64) -> String {
    match bytes {
        0..1_000 => format!("{bytes} B"),
        1_000..1_000_000 => format!("{} KB", bytes.div_ceil(1_000)),
        _ => format!("{:.1} MB", bytes as f64 / 1_000_000.),
    }
}

/// A tooltip with one line, the whole path of a file the text names.
struct PathTip {
    text: SharedString,
}

impl Render for PathTip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Theme::of(cx).colors;
        div()
            .px(SPACE_2)
            .py(SPACE_1)
            .rounded(CONTROL_RADIUS)
            .border(HAIRLINE)
            .border_color(colors.popover_border)
            .bg(colors.popover_background)
            .shadow(colors.popover_shadows())
            .font_family(MONOSPACE_FONT)
            .text_size(TEXT_SMALL)
            .text_color(colors.text_primary)
            .child(self.text.clone())
    }
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
pub fn faded(inline: &Inline, tail: &Tail, colors: Option<Colors>, chips: &[Chip]) -> StyledText {
    let links: Vec<&Range<usize>> = match colors {
        Some(_) => (inline.links.iter())
            .filter(|(_, target)| opens(target))
            .map(|(range, _)| range)
            .collect(),
        None => Vec::new(),
    };
    let labels: Vec<&Range<usize>> = (chips.iter())
        .filter(|chip| matches!(chip.kind, Kind::Mention { .. }))
        .map(|chip| &chip.label)
        .collect();
    let places: Vec<&Range<usize>> = chips
        .iter()
        .filter_map(|chip| chip.place.as_ref())
        .collect();
    let mut cuts: Vec<usize> = vec![0, inline.text.len()];
    let ranges = (inline.spans.iter().map(|(range, _)| range))
        .chain(links.iter().copied())
        .chain(labels.iter().copied())
        .chain(places.iter().copied());
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
        let label = labels.iter().any(|range| within(range, start));
        let place = places.iter().any(|range| within(range, start));
        let color = match colors {
            Some(colors) if link => Some(colors.link),
            Some(colors) if place => Some(colors.text_secondary),
            _ => None,
        };
        let highlight = HighlightStyle {
            color,
            font_weight: if style.bold {
                Some(FontWeight::BOLD)
            } else {
                label.then_some(FontWeight::MEDIUM)
            },
            font_style: style.italic.then_some(FontStyle::Italic),
            strikethrough: style.strike.then_some(StrikethroughStyle {
                thickness: HAIRLINE,
                color: None,
            }),
            ..HighlightStyle::default()
        };
        // Code keeps a run of its own even unstyled, for its font.
        (style.code || highlight != HighlightStyle::default()).then_some((start..end, highlight))
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
        let styled = (later.iter()).find(|(range, _)| range.start <= start && start < range.end);
        let mut style = styled.map(|(_, style)| *style).unwrap_or_default();
        let shown: f32 = (fades.iter())
            .filter(|(range, _)| range.start <= start && start < range.end)
            .map(|(_, amount)| *amount)
            .product();
        if shown < 1. {
            style.fade_out = Some(1. - shown);
        }
        if styled.is_some() || style != HighlightStyle::default() {
            highlights.push((start..end, style));
        }
    }
    StyledText::new(text.to_owned()).with_highlights(highlights)
}

/// A line of inline Markdown, such as a problem's words with a command in
/// backticks.
pub fn line(text: &str) -> StyledText {
    faded(&crate::conversation::markdown::inline(text), &[], None, &[])
}
