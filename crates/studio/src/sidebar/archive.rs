//! A session's actions from its row: the button the pointer shows at the
//! row's end, which archives it, or restores it in the archive, and the menu
//! a right click opens. The archive is a view of the list the sessions' menu
//! switches to: the sessions archived, newest first, each to restore.

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, Pixels, Point, SharedString, Window, div,
    prelude::*, svg,
};
use theme::{ICON_SIZE_ROW, SESSION_LINE_HEIGHT, Theme};
use workspace::{ProjectKind, SessionId, Tab, Timestamp};

use super::Sidebar;
use crate::{
    assets::Icon,
    menu::{Entry, Item, Placement},
    tooltip::TooltipExt,
};

/// The session a row's menu opened on, and where.
pub(super) struct RowMenu {
    session: SessionId,
    position: Point<Pixels>,
}

impl Sidebar {
    /// Shows the archive in the list, or the sessions again.
    pub(super) fn toggle_archive(&mut self, cx: &mut Context<Self>) {
        self.archive_open = !self.archive_open;
        self.stale = true;
        cx.notify();
    }

    /// Archives the session, or restores it while the archive shows.
    fn archive_or_restore(&mut self, session: SessionId, cx: &mut Context<Self>) {
        let restore = self.archive_open;
        self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| {
                if restore {
                    workspace.unarchive_session(session);
                } else {
                    workspace.archive_session(session, Timestamp::now());
                }
            })
        });
        // The row under the pointer is another session now: it shows its
        // button once the pointer moves.
        self.pointer.row = None;
        cx.notify();
    }

    /// The button at the end of the row under the pointer: Archive, or
    /// Restore in the archive. A double click acts once, so its second click
    /// never reaches the row that moved up under it.
    pub(super) fn render_row_action(
        &self,
        session: SessionId,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let colors = Theme::of(cx).colors;
        let (icon, label) = if self.archive_open {
            (Icon::Unarchive, "Restore")
        } else {
            (Icon::Archive, "Archive")
        };
        // Only the icon shows: a wash as wide as a control would crowd the
        // project's line it stands on. The pointer lightens it and turns to a
        // hand over it.
        let id = SharedString::from(format!("row-action-{}", session.0));
        div()
            .id(id.clone())
            .group(id.clone())
            .size(SESSION_LINE_HEIGHT)
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .child(
                svg()
                    .path(icon.path())
                    .size(ICON_SIZE_ROW)
                    .text_color(colors.text_secondary)
                    .group_hover(id, |icon| icon.text_color(colors.text_primary)),
            )
            .on_click(cx.listener(move |sidebar, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                let repeat =
                    matches!(event, ClickEvent::Mouse(click) if click.down.click_count > 1);
                if !repeat {
                    sidebar.archive_or_restore(session, cx);
                }
            }))
            .with_tooltip(label)
    }

    /// The session whose menu is open, which keeps its row's wash.
    pub(super) fn row_menu_session(&self) -> Option<SessionId> {
        self.row_menu.opened().map(|menu| menu.session)
    }

    pub(super) fn open_row_menu(
        &mut self,
        session: SessionId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.row_menu
            .open(RowMenu { session, position }, window, cx);
    }

    /// The row's menu: a new session in its project, its folder in the
    /// Finder, closing its tab while it has one, then Archive or Restore.
    pub(super) fn render_row_menu(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.row_menu.opened()?;
        let (session, position) = (menu.session, menu.position);
        let muted = Theme::of(cx).colors.text_tertiary;
        let workspace = self.store.read(cx).workspace();
        let project = workspace.project(workspace.session(session)?.project)?;
        let (project_id, folder) = (project.id, project.path.clone());
        let mut entries = vec![Entry::Item(
            Item::new(
                "row-new-session",
                "New Session in Project",
                move |sidebar: &mut Sidebar, _, cx| sidebar.new_session_in(project_id, cx),
            )
            .icon(Icon::NewSession, muted),
        )];
        if project.kind == ProjectKind::Folder {
            entries.push(Entry::Item(
                Item::new(
                    "row-reveal",
                    "Show in Finder",
                    move |_: &mut Sidebar, _, cx| {
                        if crate::background::allows_system_ui("the Finder") {
                            cx.reveal_path(&folder)
                        }
                    },
                )
                .icon(Icon::FolderOpen, muted),
            ));
        }
        let tab = Tab::Session(session);
        if workspace.open_tabs().contains(&tab) {
            entries.push(Entry::Item(
                Item::new(
                    "row-close-tab",
                    "Close Tab",
                    move |sidebar: &mut Sidebar, _, cx| {
                        sidebar.store.update(cx, |store, cx| {
                            store.update(cx, |workspace| workspace.close_tab(tab, Timestamp::now()))
                        })
                    },
                )
                .enabled(workspace.can_close_tab(tab))
                .icon(Icon::Close, muted),
            ));
        }
        let (id, label, icon) = if self.archive_open {
            ("row-restore", "Restore", Icon::Unarchive)
        } else {
            ("row-archive", "Archive", Icon::Archive)
        };
        entries.push(Entry::Separator);
        entries.push(Entry::Item(
            Item::new(id, label, move |sidebar: &mut Sidebar, _, cx| {
                sidebar.archive_or_restore(session, cx)
            })
            .icon(icon, muted),
        ));
        self.row_menu
            .render("row-menu", entries, Placement::At(position), window, cx)
    }
}
