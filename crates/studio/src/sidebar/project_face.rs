//! A project as its sessions' rows show it: its icon, its name and its
//! checkout's branch. The icon is the project's own favicon or logo where it
//! keeps one, else its initials on its hue's badge; the computer's is its icon.

use std::{path::Path, sync::Arc};

use gpui::{
    AnyElement, FontWeight, Hsla, IntoElement, ObjectFit, ParentElement, SharedString, Styled,
    StyledImage, div, img, svg,
};
use theme::{
    Colors, ICON_SIZE_ROW, MONOSPACE_FONT, PROJECT_ICON, PROJECT_ICON_RADIUS,
    PROJECT_INITIAL_WIDTH, PROJECT_INITIALS_SIZE,
};
use workspace::{Head, Project, ProjectKind, Session, Timestamp, compact_age};

use super::{
    card::{FoundIcon, Row},
    project_icon::{ProjectIcon, Tone},
};
use crate::assets::Icon;

/// A project as its sessions' rows show it, worked out once per refresh and
/// shared by its rows.
#[derive(Clone)]
pub(super) struct ProjectFace {
    /// Its name as the sidebar shows it.
    pub name: SharedString,
    /// What its checkout is on; `None` outside git and for the computer.
    pub branch: Option<SharedString>,
    mark: Mark,
}

/// What stands in a project's icon slot.
#[derive(Clone)]
enum Mark {
    /// The project's own icon, as last found.
    Own(Arc<FoundIcon>),
    /// Two letters on a tile of the project's hue.
    Initials { letters: SharedString, hue: usize },
    /// A folder on that badge, for a name with no letter to take.
    Folder { hue: usize },
    /// The computer: no folder to draw or color it by.
    Computer,
}

impl ProjectFace {
    pub fn new(
        project: &Project,
        name: String,
        icon: Option<Arc<FoundIcon>>,
        head: Option<&Head>,
    ) -> Self {
        let mark = match project.kind {
            ProjectKind::Computer => Mark::Computer,
            ProjectKind::Folder => match icon {
                Some(found) if found.icon.is_some() => Mark::Own(found),
                _ => {
                    let hue = hue(&project.path);
                    match initials(&project.title) {
                        Some(letters) => Mark::Initials {
                            letters: letters.into(),
                            hue,
                        },
                        None => Mark::Folder { hue },
                    }
                }
            },
        };
        let branch = project.kind.is_folder().then_some(head).flatten();
        Self {
            name: name.into(),
            branch: branch.map(|head| match head {
                Head::Branch(name) | Head::Detached(name) => name.clone().into(),
            }),
            mark,
        }
    }

    pub fn row(&self, session: &Session, now: Timestamp) -> Row {
        Row {
            id: session.id,
            title: session.display_title().into(),
            activity: session.activity(),
            age: compact_age(session.activity(), now).into(),
            agent: session.choice.agent.clone(),
            project: self.clone(),
        }
    }

    /// The icon before the project's name in its `PROJECT_ICON` slot: the
    /// project's own, with `PROJECT_ICON_RADIUS` corners, set on a plate of
    /// the opposite tone when its mark would vanish on the glass; else its
    /// initials on a tile of its badge's initials color at 14%, in that color,
    /// which reads on the glass and on a row's surface; a folder the same
    /// color for a name without a letter; the computer in `secondary`, the
    /// line's color.
    pub fn icon(&self, colors: &Colors, secondary: Hsla) -> AnyElement {
        let slot = || {
            div()
                .size(PROJECT_ICON)
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
        };
        let glyph = |icon: Icon, color: Hsla| {
            slot()
                .child(
                    svg()
                        .path(icon.path())
                        .size(ICON_SIZE_ROW)
                        .text_color(color),
                )
                .into_any_element()
        };
        let color = |hue: usize| colors.badges[hue % colors.badges.len()].text;
        match &self.mark {
            Mark::Own(found) => match &found.icon {
                Some(icon) => own_icon(slot(), icon, colors),
                None => slot().into_any_element(),
            },
            Mark::Initials { letters, hue } => slot()
                .rounded(PROJECT_ICON_RADIUS)
                .bg(color(*hue).opacity(TILE_OPACITY))
                .font_family(MONOSPACE_FONT)
                .text_size(PROJECT_INITIALS_SIZE)
                .line_height(PROJECT_ICON)
                .font_weight(FontWeight::BOLD)
                .text_color(color(*hue))
                .children(letters.chars().map(|letter| {
                    div()
                        .w(PROJECT_INITIAL_WIDTH)
                        .flex()
                        .justify_center()
                        .child(SharedString::from(letter.to_string()))
                }))
                .into_any_element(),
            Mark::Folder { hue } => glyph(Icon::Folder, color(*hue)),
            Mark::Computer => glyph(Icon::Computer, secondary),
        }
    }
}

/// How much of the initials' color tints their tile.
const TILE_OPACITY: f32 = 0.14;

/// A project's own icon in its slot. A dark mark on the dark glass, or a
/// light one on the light, stands on a plate of `text_primary`, which is
/// the opposite tone, at 90%, inset by `PLATE_INSET` so its corners show.
fn own_icon(slot: gpui::Div, icon: &ProjectIcon, colors: &Colors) -> AnyElement {
    let dark_glass = colors.text_primary.l > 0.5;
    let plated = match icon.tone {
        Tone::Dark => dark_glass,
        Tone::Light => !dark_glass,
        Tone::Own => false,
    };
    let picture = img(icon.image.clone()).object_fit(ObjectFit::Contain);
    if plated {
        slot.rounded(PROJECT_ICON_RADIUS)
            .bg(colors.text_primary.opacity(0.9))
            .child(picture.size(PROJECT_ICON - PLATE_INSET * 2.))
            .into_any_element()
    } else {
        slot.child(picture.size_full().rounded(PROJECT_ICON_RADIUS))
            .into_any_element()
    }
}

/// How far a plated icon stands inside its plate.
const PLATE_INSET: gpui::Pixels = gpui::px(2.);

/// A project's hue, from its folder, so it keeps it across launches.
fn hue(path: &Path) -> usize {
    // FNV-1a: stable across builds, unlike the standard library's hasher.
    let hash = path
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    (hash % 8) as usize
}

/// Two letters for the project's own name, not the name the sidebar shows,
/// which may lead with a parent folder to tell two projects apart. Words are
/// runs of letters and digits. The first is the first word's first; the
/// second the first word's first digit after it, else the last word's
/// first, else the first word's last: "arcada.next" AN, "web" WB, "app2"
/// A2. Upper case; `None` for a name with no letter or digit.
pub(super) fn initials(name: &str) -> Option<String> {
    let words: Vec<Vec<char>> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.chars().collect())
        .collect();
    let first_word = words.first()?;
    let first = first_word[0];
    let second = first_word[1..]
        .iter()
        .copied()
        .find(|c| c.is_numeric())
        .or_else(|| (words.len() > 1).then(|| words[words.len() - 1][0]))
        .unwrap_or(first_word[first_word.len() - 1]);
    Some(
        [first, second]
            .into_iter()
            .flat_map(char::to_uppercase)
            .take(2)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::initials;

    #[test]
    fn initials_take_the_first_and_last_words() {
        assert_eq!(initials("arcada.next").as_deref(), Some("AN"));
        assert_eq!(initials("nerda-studio app").as_deref(), Some("NA"));
        assert_eq!(initials("  --web__  ").as_deref(), Some("WB"));
    }

    #[test]
    fn initials_of_one_word() {
        assert_eq!(initials("web").as_deref(), Some("WB"));
        assert_eq!(initials("app2 web").as_deref(), Some("A2"));
        assert_eq!(initials("a").as_deref(), Some("AA"));
    }

    #[test]
    fn initials_read_any_script() {
        assert_eq!(initials("oʻzbek loyiha").as_deref(), Some("OL"));
        assert_eq!(initials("лойиҳа.веб").as_deref(), Some("ЛВ"));
        assert_eq!(initials("🚀rocket").as_deref(), Some("RT"));
    }

    #[test]
    fn no_letters_no_initials() {
        assert_eq!(initials("..."), None);
        assert_eq!(initials(""), None);
        assert_eq!(initials("🚀"), None);
    }
}
