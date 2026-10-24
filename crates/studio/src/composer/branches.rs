//! The branch in the drawer under a draft's box: a control naming what the
//! project's checkout is on, whose menu lists where it could go, in one list
//! with no groups. A search field leads it; then the branch checked out,
//! checked; the other branches, newest commit first, each with its commit's
//! age, or, for a branch another checkout of the repository is on, that
//! checkout's folder; the checkouts on no branch, by folder; the remote
//! branches with no local one yet, quieter, last; and, for a search no
//! branch is named, Create branch. Picking a branch switches the checkout
//! to it, a remote branch makes a local one tracking it, and a branch in
//! another checkout, or a checkout, opens a draft there as a project of its
//! own.
//!
//! The repository is read off the UI thread each time the menu opens; the
//! last reading shows meanwhile. Only a draft has the menu, so no agent is
//! working in the checkout while it switches.

use std::path::PathBuf;

use gpui::{
    Anchor, App, ClickEvent, Context, Entity, Focusable, IntoElement, ParentElement, SharedString,
    Styled, Subscription, Task, Window, div, prelude::*, relative,
};
use theme::Theme;
use workspace::{
    AddedProject, Branches, Head, ProjectId, Switch, Timestamp, branch_name, compact_age,
    matches_search,
};

use super::{Composer, OpenProject, context::Side};
use crate::{
    assets::Icon,
    menu::{Entry, Item, Menu, Placement},
    text_input::{Lines, TextInput, TextInputEvent},
    tooltip::TooltipExt,
};

/// Most local branches, and most remote ones, the menu lists; a search
/// finds the rest.
const LIST_MAX: usize = 50;

pub(super) struct BranchPicker {
    menu: Menu<Composer, ()>,
    /// The repository as last read, or why it couldn't be.
    listing: Option<Result<Branches, SharedString>>,
    reading: Task<()>,
    /// A switch under way: picks wait until it is done.
    switching: Option<Task<()>>,
}

impl BranchPicker {
    pub fn new(cx: &mut Context<Composer>) -> (Self, Subscription) {
        let search: Entity<TextInput> =
            cx.new(|cx| TextInput::new("Search or create a branch", Lines::Single, cx));
        let changed = cx.subscribe(&search, |composer, _, TextInputEvent::Changed, cx| {
            composer.branches.menu.query_changed(cx)
        });
        let picker = Self {
            menu: Menu::with_search(|composer| &mut composer.branches.menu, search, cx),
            listing: None,
            reading: Task::ready(()),
            switching: None,
        };
        (picker, changed)
    }
}

impl Composer {
    /// The session's folder project and its folder; `None` for No project.
    fn checkout(&self, cx: &App) -> Option<(ProjectId, PathBuf)> {
        let workspace = self.store.read(cx).workspace();
        let project = workspace.project(workspace.session(self.session)?.project)?;
        project
            .kind
            .is_folder()
            .then(|| (project.id, project.path.clone()))
    }

    pub(super) fn open_branches(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, folder)) = self.checkout(cx) else {
            return;
        };
        let text = self.input.focus_handle(cx);
        self.branches.menu.open_from((), Some(text), window, cx);
        let read = cx
            .background_executor()
            .spawn(async move { workspace::branches(&folder) });
        self.branches.reading = cx.spawn(async move |composer, cx| {
            let listing = read.await.map_err(SharedString::from);
            composer
                .update(cx, |composer, cx| {
                    composer.branches.listing = Some(listing);
                    cx.notify();
                })
                .ok();
        });
    }

    /// Puts the checkout on `to` off the UI thread; the chip shows the new
    /// head when it is done, the problem line why when git refused.
    fn switch_branch(&mut self, to: Switch, cx: &mut Context<Self>) {
        let Some((project, folder)) = self.checkout(cx) else {
            return;
        };
        if self.branches.switching.is_some() {
            return;
        }
        let switched = cx.background_executor().spawn(async move {
            let switched = workspace::switch(&folder, &to);
            (switched, workspace::head(&folder))
        });
        self.branches.switching = Some(cx.spawn(async move |composer, cx| {
            let (switched, head) = switched.await;
            composer
                .update(cx, |composer, cx| {
                    composer.branches.switching = None;
                    composer.branches.listing = None;
                    if let Err(problem) = switched {
                        composer.problem = Some(problem.into());
                    }
                    composer
                        .store
                        .update(cx, |store, cx| store.set_head(project, head, cx));
                    cx.notify();
                })
                .ok();
        }));
    }

    /// Opens a draft in another checkout of the repository, adding it as a
    /// project the first time.
    fn open_worktree(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let added = self.store.update(cx, |store, cx| {
            store.update(cx, |workspace| {
                workspace.add_project(&path, Timestamp::now())
            })
        });
        match added {
            Ok(AddedProject::Added(id) | AddedProject::Existing(id)) => cx.emit(OpenProject(id)),
            Err(problem) => {
                self.problem = Some(problem.to_string().into());
                cx.notify();
            }
        }
    }

    fn branch_entries(&self, cx: &App) -> Vec<Entry<Self>> {
        let colors = Theme::of(cx).colors;
        let listing = match &self.branches.listing {
            None => return vec![Entry::Section("Reading branches…".into())],
            Some(Err(problem)) => return vec![Entry::Section(problem.clone())],
            Some(Ok(listing)) => listing,
        };
        let query = self.branches.menu.query(cx);
        let now = Timestamp::now();
        let age = |committed: Option<Timestamp>| {
            committed.map_or_else(String::new, |time| compact_age(time, now))
        };
        let current = |name: &str| listing.head == Some(Head::Branch(name.to_owned()));
        let folder = |path: &std::path::Path| {
            path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            )
        };
        let mut entries: Vec<Entry<Self>> = Vec::new();

        if let Some(Head::Detached(id)) = &listing.head {
            let label = format!("Detached at {id}");
            if matches_search(&label, &query) {
                entries.push(Entry::Item(
                    Item::new("branch-detached", label, |_, _, _| {})
                        .icon(Icon::GitBranch, colors.text_secondary)
                        .checked_at_end(true),
                ));
            }
        }
        // The one checked out leads; it is checked rather than dated, so the
        // ages stay in one column. A branch another checkout is on names
        // that checkout's folder, and goes to it.
        let (here, others): (Vec<_>, Vec<_>) = (listing.local.iter())
            .filter(|b| matches_search(&b.name, &query))
            .partition(|b| current(&b.name));
        for (at, branch) in here.into_iter().chain(others).take(LIST_MAX).enumerate() {
            let here = current(&branch.name);
            let item = match &branch.worktree {
                Some(path) => {
                    let to = path.clone();
                    Item::new(
                        ("branch-local", at),
                        branch.name.clone(),
                        move |composer: &mut Composer,
                              _: &mut Window,
                              cx: &mut Context<Composer>| {
                            composer.open_worktree(to.clone(), cx)
                        },
                    )
                    .detail(folder(path))
                }
                None => {
                    let to = Switch::Branch(branch.name.clone());
                    Item::new(
                        ("branch-local", at),
                        branch.name.clone(),
                        move |composer: &mut Composer,
                              _: &mut Window,
                              cx: &mut Context<Composer>| {
                            if !here {
                                composer.switch_branch(to.clone(), cx);
                            }
                        },
                    )
                    .detail(if here {
                        String::new()
                    } else {
                        age(branch.committed)
                    })
                }
            };
            entries.push(Entry::Item(
                item.icon(Icon::GitBranch, colors.text_secondary)
                    .checked_at_end(here),
            ));
        }
        // A checkout on no branch is reached by its folder.
        for (at, worktree) in listing.worktrees.iter().enumerate() {
            let name = folder(&worktree.path);
            let on = match &worktree.head {
                Some(Head::Branch(name) | Head::Detached(name)) => name.clone(),
                None => String::new(),
            };
            if !matches_search(&name, &query) && !matches_search(&on, &query) {
                continue;
            }
            let path = worktree.path.clone();
            entries.push(Entry::Item(
                Item::new(
                    ("branch-worktree", at),
                    name,
                    move |composer: &mut Composer, _: &mut Window, cx: &mut Context<Composer>| {
                        composer.open_worktree(path.clone(), cx)
                    },
                )
                .icon(Icon::Folder, colors.text_secondary)
                .detail(on),
            ));
        }
        // Remote branches come last, quieter: picking one makes the branch.
        let remote = (listing.remote.iter()).filter(|b| matches_search(&b.full_name(), &query));
        for (at, branch) in remote.take(LIST_MAX).enumerate() {
            let to = Switch::Track(branch.clone());
            entries.push(Entry::Item(
                Item::new(
                    ("branch-remote", at),
                    branch.full_name(),
                    move |composer: &mut Composer, _: &mut Window, cx: &mut Context<Composer>| {
                        composer.switch_branch(to.clone(), cx)
                    },
                )
                .icon(Icon::GitBranch, colors.text_tertiary)
                .quiet()
                .detail(age(branch.committed)),
            ));
        }

        let name = branch_name(&query);
        let taken = listing.local.iter().any(|branch| branch.name == name);
        let listed = entries.iter().any(|entry| matches!(entry, Entry::Item(_)));
        if !listed {
            entries.push(Entry::Section("No matching branches".into()));
        }
        if !name.is_empty() && !taken {
            entries.push(Entry::Separator);
            let to = Switch::Create(name.clone());
            entries.push(Entry::Item(
                Item::new(
                    "branch-create",
                    format!("Create branch “{name}”"),
                    move |composer: &mut Composer, _: &mut Window, cx: &mut Context<Composer>| {
                        composer.switch_branch(to.clone(), cx)
                    },
                )
                .icon(Icon::NewTab, colors.text_secondary),
            ));
        }
        entries
    }

    /// The drawer's trailing side: the branch the checkout is on, or the
    /// commit, cut in the middle, at most 40% of the drawer, the whole of it
    /// in the tooltip. Its menu hangs downward from its trailing edge.
    pub(super) fn render_branch(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let (project, _) = self.checkout(cx)?;
        let branch = match self.store.read(cx).head(project)? {
            Head::Branch(name) | Head::Detached(name) => SharedString::from(name.clone()),
        };
        let colors = Theme::of(cx).colors;
        let open = self.branches.menu.opened().is_some();
        let menu = if open {
            let entries = self.branch_entries(cx);
            (self.branches.menu).render(
                "composer-branch-menu",
                entries,
                Placement::Hang(Anchor::TopRight),
                window,
                cx,
            )
        } else {
            None
        };
        let chip = Side {
            id: "composer-branch",
            icon: Icon::GitBranch,
            label: branch.clone(),
            cut_middle: true,
            open,
        }
        .render(&colors)
        .on_click(
            cx.listener(|composer, _: &ClickEvent, window, cx| composer.open_branches(window, cx)),
        )
        .with_tooltip(branch);
        Some(
            div()
                .relative()
                .flex_shrink_0()
                .min_w_0()
                .max_w(relative(0.4))
                .child(chip)
                .children(menu.map(|menu| div().absolute().right_0().bottom_0().child(menu))),
        )
    }
}
