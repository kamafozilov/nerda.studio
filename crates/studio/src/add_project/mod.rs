//! Add project: a palette over the window that browses to a folder as you type its
//! path. It opens at `~/`; the text up to the last separator names the folder listed,
//! the rest filters its subfolders. Enter on a highlighted folder opens it; Enter with
//! none highlighted, or Cmd-Enter, adds the typed path, creating the folder first if it
//! doesn't exist. The system's folder picker stays one click away in the footer.

mod view;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{
    App, AppContext, Context, DismissEvent, Entity, EventEmitter, Focusable, KeyBinding,
    PathPromptOptions, ScrollStrategy, SharedString, Subscription, UniformListScrollHandle, Window,
    actions,
};
use workspace::AddProjectError;
use workspace::browse::{BrowseQuery, ListError, Listing, Target, folder_text};

use crate::{
    sidebar::Sidebar,
    text_input::{Lines, TextInput, TextInputEvent},
};

actions!(
    add_project,
    [
        /// Moves the highlight down, wrapping at the end.
        SelectNext,
        /// Moves the highlight up, wrapping at the start.
        SelectPrevious,
        /// Opens the highlighted folder, else adds the typed path.
        Confirm,
        /// Adds the typed path even while a folder is highlighted.
        AddTyped,
        Close,
    ]
);

const CONTEXT: &str = "AddProject";
/// Where browsing starts.
const START: &str = "~/";
/// How long a folder may take to list before the palette says it is loading, so
/// quick ones never flash the message.
const LOADING_DELAY: Duration = Duration::from_millis(150);

pub fn bind_keys(cx: &mut App) {
    let add_typed = if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    };
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, Some(CONTEXT)),
        KeyBinding::new("up", SelectPrevious, Some(CONTEXT)),
        KeyBinding::new("enter", Confirm, Some(CONTEXT)),
        KeyBinding::new(add_typed, AddTyped, Some(CONTEXT)),
        KeyBinding::new("escape", Close, Some(CONTEXT)),
    ]);
}

/// A row of the list.
#[derive(Clone, Debug)]
enum Row {
    /// The parent folder, holding the text that lists it.
    Up(String),
    Folder(SharedString),
}

enum Load {
    Pending(Instant),
    Done(Result<Listing, ListError>),
}

pub struct AddProjectPalette {
    sidebar: Entity<Sidebar>,
    input: Entity<TextInput>,
    query: String,
    /// The current project's folder, where `./` and `../` start.
    base: Option<PathBuf>,
    /// Folders listed since the palette opened, by resolved path.
    listings: HashMap<PathBuf, Load>,
    /// The rows on screen, in order.
    rows: Vec<Row>,
    /// The row Enter opens; `None` until an arrow key picks one, and after each edit.
    highlighted: Option<usize>,
    /// A folder to open once it has listed, so the list never shows it loading;
    /// an edit in the meantime drops it.
    pending_open: Option<(PathBuf, String)>,
    /// Why the last add failed; the next edit clears it.
    problem: Option<SharedString>,
    /// The system's folder picker is open: a second click must not open another.
    picking_folder: bool,
    scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for AddProjectPalette {}

impl AddProjectPalette {
    pub fn new(
        sidebar: Entity<Sidebar>,
        base: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            let mut input = TextInput::new(
                "Enter project path (e.g. ~/projects/my-app)",
                Lines::Single,
                cx,
            );
            input.set_text(START, cx);
            input
        });
        let subscription = cx.subscribe(&input, |palette, input, TextInputEvent::Changed, cx| {
            palette.query = input.read(cx).text().to_owned();
            palette.edited(cx);
        });
        window.focus(&input.focus_handle(cx), cx);
        let mut palette = Self {
            sidebar,
            input,
            query: START.to_owned(),
            base,
            listings: HashMap::new(),
            rows: Vec::new(),
            highlighted: None,
            pending_open: None,
            problem: None,
            picking_folder: false,
            scroll: UniformListScrollHandle::new(),
            _subscriptions: vec![subscription],
        };
        palette.load_query(cx);
        palette
    }

    /// The query, and the folder it lists; `None` when it is not a path, or is
    /// relative with no project to start from.
    fn browse(&self) -> Option<(BrowseQuery<'_>, PathBuf)> {
        let query = BrowseQuery::parse(self.query.trim())?;
        let directory = query.resolve_directory(self.base.as_deref())?;
        Some((query, directory))
    }

    /// The listing of the query's folder, once loaded.
    fn listing(&self) -> Option<(BrowseQuery<'_>, &PathBuf, Result<&Listing, ListError>)> {
        let (query, directory) = self.browse()?;
        let (directory, load) = self.listings.get_key_value(&directory)?;
        match load {
            Load::Done(result) => Some((query, directory, result.as_ref().map_err(|e| *e))),
            Load::Pending(_) => None,
        }
    }

    /// What adding would do, as far as the loaded listing tells.
    fn target(&self) -> Option<Target> {
        let (query, directory, listing) = self.listing()?;
        Some(Target::of(&query, directory, listing))
    }

    fn edited(&mut self, cx: &mut Context<Self>) {
        self.highlighted = None;
        self.pending_open = None;
        self.problem = None;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.load_query(cx);
        cx.notify();
    }

    fn load_query(&mut self, cx: &mut Context<Self>) {
        if let Some((_, directory)) = self.browse() {
            self.load(directory, cx);
        }
    }

    /// Lists `directory` off the UI thread, once per palette.
    fn load(&mut self, directory: PathBuf, cx: &mut Context<Self>) {
        if self.listings.contains_key(&directory) {
            return;
        }
        self.listings
            .insert(directory.clone(), Load::Pending(Instant::now()));
        cx.spawn(async move |palette, cx| {
            let read = directory.clone();
            let listing = cx
                .background_executor()
                .spawn(async move { Listing::read(&read) })
                .await;
            palette
                .update(cx, |palette, cx| {
                    palette
                        .listings
                        .insert(directory.clone(), Load::Done(listing));
                    if let Some((_, text)) = palette
                        .pending_open
                        .take_if(|(pending, _)| *pending == directory)
                    {
                        palette.set_query(text, cx);
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
        // Redraws when the loading message is due, in case the folder is still loading.
        cx.spawn(async move |palette, cx| {
            cx.background_executor().timer(LOADING_DELAY).await;
            palette.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    /// Still loading the query's folder, for long enough to say so.
    fn loading_shown(&self) -> bool {
        let Some((_, directory)) = self.browse() else {
            return false;
        };
        matches!(
            self.listings.get(&directory),
            Some(Load::Pending(since)) if since.elapsed() >= LOADING_DELAY
        )
    }

    fn set_query(&mut self, text: String, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text(text, cx));
    }

    /// Shows `text` once its folder has listed: at once if it already has.
    fn open_text(&mut self, text: String, cx: &mut Context<Self>) {
        let directory = BrowseQuery::parse(&text)
            .and_then(|query| query.resolve_directory(self.base.as_deref()));
        let Some(directory) = directory else {
            return;
        };
        if matches!(self.listings.get(&directory), Some(Load::Done(_))) {
            self.set_query(text, cx);
        } else {
            self.pending_open = Some((directory.clone(), text));
            self.load(directory, cx);
        }
    }

    /// Rebuilds `rows` from the loaded listing: the parent while the query ends in
    /// a separator, then the subfolders matching what follows it.
    fn refresh_rows(&mut self) {
        let mut rows = Vec::new();
        if let Some((query, _, Ok(listing))) = self.listing() {
            if query.leaf.is_empty()
                && let Some(parent) = listing.folder.parent()
            {
                // An absolute path stays absolute; `~/` and relative ones go up under `~`.
                let tilde = !Path::new(query.directory).is_absolute();
                rows.push(Row::Up(folder_text(parent, tilde)));
            }
            rows.extend(
                listing
                    .folders(query.leaf)
                    .map(|entry| Row::Folder(entry.name.clone().into())),
            );
        }
        if self.highlighted.is_some_and(|row| row >= rows.len()) {
            self.highlighted = None;
        }
        self.rows = rows;
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.move_highlight(1, cx);
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.move_highlight(-1, cx);
    }

    fn move_highlight(&mut self, step: isize, cx: &mut Context<Self>) {
        let count = self.rows.len();
        if count == 0 {
            return;
        }
        let next = match self.highlighted {
            None if step > 0 => 0,
            None => count - 1,
            Some(row) => (row as isize + step).rem_euclid(count as isize) as usize,
        };
        self.highlighted = Some(next);
        self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        match self.highlighted {
            Some(row) => self.open_row(row, cx),
            None => self.add_typed(cx),
        }
    }

    fn open_row(&mut self, row: usize, cx: &mut Context<Self>) {
        let text = match self.rows.get(row) {
            Some(Row::Up(text)) => text.clone(),
            Some(Row::Folder(name)) => match BrowseQuery::parse(self.query.trim()) {
                Some(query) => query.child(name),
                None => return,
            },
            None => return,
        };
        self.open_text(text, cx);
    }

    /// Adds the typed path, creating the folder if it doesn't exist.
    fn add_typed(&mut self, cx: &mut Context<Self>) {
        let Some((query, directory)) = self.browse() else {
            return;
        };
        // Listed again: the folder may have changed since the palette showed it.
        let listing = Listing::read(&directory);
        let target = Target::of(&query, &directory, listing.as_ref().map_err(|e| *e));
        self.listings.insert(directory, Load::Done(listing));
        match target {
            Target::Add(path) => self.add(&path, cx),
            Target::Create(path) => match std::fs::create_dir_all(&path) {
                Ok(()) => self.add(&path, cx),
                Err(error) => {
                    self.problem = Some(
                        format!(
                            "Couldn't create {} ({}). Choose a folder you can write to.",
                            path.display(),
                            error.kind()
                        )
                        .into(),
                    );
                }
            },
            // The list already says a file is in the way, as the disabled button does.
            Target::File(_) => {}
        }
        cx.notify();
    }

    /// Adds `path` and closes, or says why it couldn't.
    fn add(&mut self, path: &Path, cx: &mut Context<Self>) {
        let added = self
            .sidebar
            .update(cx, |sidebar, cx| sidebar.add_project_at(path, cx));
        match added {
            Ok(()) => cx.emit(DismissEvent),
            Err(error) => self.problem = Some(add_error_text(&error).into()),
        }
    }

    fn choose_in_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.picking_folder || !crate::background::allows_system_ui("the folder picker") {
            return;
        }
        self.picking_folder = true;
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add".into()),
        });
        cx.spawn_in(window, async move |palette, cx| {
            let picked = picked.await;
            palette
                .update(cx, |palette, cx| {
                    palette.picking_folder = false;
                    match picked {
                        Ok(Ok(Some(paths))) => {
                            if let Some(path) = paths.first() {
                                palette.add(path, cx);
                            }
                        }
                        Ok(Err(error)) => {
                            palette.problem = Some(
                                format!("Couldn't open the folder picker ({error}). Try again.")
                                    .into(),
                            );
                        }
                        // Cancelled, or the picker went away with the window.
                        Ok(Ok(None)) | Err(_) => {}
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn close(&mut self, _: &Close, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }
}

/// What went wrong adding a folder, and what to do instead.
fn add_error_text(error: &AddProjectError) -> String {
    match error {
        AddProjectError::NotFound(path) => format!(
            "Couldn't find {}. Choose a folder that exists.",
            path.display()
        ),
        AddProjectError::NotADirectory(path) => format!(
            "{} is a file. Choose a folder to add as a project.",
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
        ),
        AddProjectError::Io(path, error) => format!(
            "Couldn't open {} ({}). Check its permissions and try again.",
            path.display(),
            error.kind()
        ),
    }
}
