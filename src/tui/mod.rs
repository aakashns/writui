//! The terminal UI: terminal setup, the event loop, and switching screens.

mod dialog;
mod editor;
mod file_dialog;
mod hints;
mod menu;
mod list;
mod path_field;
mod settings;
mod setup;
mod textarea;
mod unlock;
mod widgets;

use std::io::stdout;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEventKind, KeyEventState, KeyModifiers, KeyboardEnhancementFlags,
    ModifierKeyCode, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::execute;
use ratatui::crossterm::style::Print;
use ratatui::crossterm::terminal::{SetTitle, supports_keyboard_enhancement};
use ratatui::{DefaultTerminal, Frame};
use zeroize::Zeroizing;

use crate::files;
use crate::vault::{ExportSettings, OpenError, Settings, Vault, title_from_first_line};

/// What a screen asks the app to do in response to an event.
pub enum Action {
    None,
    Quit,
    CreateVault(Zeroizing<String>),
    Unlock(Zeroizing<String>),
    NewPost,
    OpenPost(i64),
    /// Move to the Trash, then highlight `select` in the list.
    DeletePost { id: i64, select: Option<i64> },
    ShowTrash,
    RestorePost { id: i64, select: Option<i64> },
    DeletePostForever { id: i64, select: Option<i64> },
    /// Go back to the list, highlighting this post.
    BackToList(Option<i64>),
    /// Save a version of the open post.
    SaveVersion { name: String },
    /// Show the open post's history.
    ShowHistory,
    /// Open a version in the history, to read.
    LoadVersion(i64),
    /// Remember how the open post was just exported.
    RecordExport(ExportSettings),
    /// Import a markdown file as a post.
    Import { path: PathBuf, into: ImportInto },
    /// Find the posts matching a search on the list.
    Search(String),
    ShowSettings,
    SaveSettings(Settings),
    /// Remember how the list is sorted.
    SortBy(list::Sort),
    /// Lock writui: back to the password, then to the same screen.
    Lock,
}

/// Which post an imported file goes into.
pub enum ImportInto {
    /// A new one, unless the file was imported before: then ask.
    Check,
    New,
    /// This post, which the file was imported into before.
    Post(i64),
}

/// Whether the post is being stored because the editor is being left.
#[derive(Clone, Copy)]
enum Leaving {
    Yes,
    No,
}

enum Screen {
    Setup(setup::Setup),
    Unlock(unlock::Unlock),
    List(Box<list::List>),
    Editor(Box<editor::Editor>),
    Settings(Box<settings::Settings>),
}

pub fn run(vault_path: PathBuf) -> Result<()> {
    let mut terminal = ratatui::init();
    execute!(stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    // Keep the terminal's own title, to put back on the way out (where
    // the terminal can).
    execute!(stdout(), Print(PUSH_TITLE))?;
    // Terminals that speak the kitty keyboard protocol can report Ctrl
    // being pressed and let go on its own, so the editor can keep its hints
    // out of sight until Ctrl is held.
    let ctrl_reported = supports_keyboard_enhancement().unwrap_or(false);
    if ctrl_reported {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
            )
        )?;
    }
    // ratatui's own panic hook restores the screen; also give the terminal
    // its mouse and keyboard back.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        give_back_terminal(ctrl_reported);
        hook(info);
    }));
    let result = App::new(vault_path, ctrl_reported).run(&mut terminal);
    give_back_terminal(ctrl_reported);
    ratatui::restore();
    result
}

/// Save and restore the window title (xterm's title stack; terminals
/// without one ignore these).
const PUSH_TITLE: &str = "\x1b[22;0t";
const POP_TITLE: &str = "\x1b[23;0t";

fn give_back_terminal(ctrl_reported: bool) {
    if ctrl_reported {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(
        stdout(),
        DisableMouseCapture,
        DisableBracketedPaste,
        SetCursorStyle::DefaultUserShape,
        Print(POP_TITLE)
    );
}

/// Smooth over the keyboard protocol: with every key reported as a code,
/// Caps Lock no longer capitalises letters, so do that here. (Not with
/// Ctrl: Ctrl+Shift+C means something else than Ctrl+C.)
fn normalize(event: Event) -> Event {
    match event {
        Event::Key(mut key)
            if key.state.contains(KeyEventState::CAPS_LOCK)
                && !key.modifiers.intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL) =>
        {
            if let KeyCode::Char(ch) = key.code
                && ch.is_lowercase()
            {
                key.code = KeyCode::Char(ch.to_uppercase().next().unwrap_or(ch));
            }
            Event::Key(key)
        }
        event => event,
    }
}

struct App {
    vault_path: PathBuf,
    vault: Option<Vault>,
    screen: Screen,
    /// Slow work (key derivation) to do right after the next draw, so the
    /// screen can say "Unlocking…" first.
    pending: Option<Action>,
    /// Storing the post failed on the last Ctrl+Q; the next one quits anyway.
    quit_unsaved: bool,
    /// The cursor is currently a bar (in the editor) rather than the
    /// terminal's own shape.
    bar_cursor: bool,
    /// The terminal reports Ctrl on its own (see `run`).
    ctrl_reported: bool,
    /// Ctrl is held down right now.
    ctrl_held: bool,
    /// The terminal window's title, as last set.
    title: String,
    /// The screen last remembered in the vault (see `screen_name`).
    remembered: Option<String>,
    quit: bool,
}

impl App {
    fn new(vault_path: PathBuf, ctrl_reported: bool) -> Self {
        let screen = if vault_path.exists() {
            Screen::Unlock(unlock::Unlock::default())
        } else {
            Screen::Setup(setup::Setup::new(vault_path.clone()))
        };
        App {
            vault_path,
            vault: None,
            screen,
            pending: None,
            quit_unsaved: false,
            bar_cursor: false,
            ctrl_reported,
            ctrl_held: false,
            title: String::new(),
            remembered: None,
            quit: false,
        }
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.quit {
            terminal.draw(|frame| self.render(frame))?;
            let bar = matches!(self.screen, Screen::Editor(_));
            if bar != self.bar_cursor {
                let style =
                    if bar { SetCursorStyle::BlinkingBar } else { SetCursorStyle::DefaultUserShape };
                execute!(stdout(), style)?;
                self.bar_cursor = bar;
            }
            let title = self.title();
            if title != self.title {
                execute!(stdout(), SetTitle(&title))?;
                self.title = title;
            }
            if let Some(action) = self.pending.take() {
                self.perform(action)?;
                continue;
            }
            // Wait for input, but wake up to store the post when it's due.
            let due = self.autosave_due();
            if due.is_some_and(|due| due <= Instant::now()) {
                self.store_post(Leaving::No);
                continue;
            }
            if let Some(due) = due
                && !event::poll(due.saturating_duration_since(Instant::now()))?
            {
                continue;
            }
            let action = self.handle(event::read()?);
            self.apply(action)?;
            self.remember_screen();
        }
        Ok(())
    }

    /// The screen showing, as remembered in the vault so the next run
    /// opens on it: `list`, `trash`, `settings` or `post:<id>`. `None`
    /// before unlocking.
    fn screen_name(&self) -> Option<String> {
        match &self.screen {
            Screen::Setup(_) | Screen::Unlock(_) => None,
            Screen::List(list) if list.trash_view() => Some("trash".into()),
            Screen::List(_) => Some("list".into()),
            Screen::Settings(_) => Some("settings".into()),
            Screen::Editor(editor) => Some(format!("post:{}", editor.post_id())),
        }
    }

    /// Remember the screen showing, if it changed. Only a convenience, so
    /// failing to is no reason to stop.
    fn remember_screen(&mut self) {
        let name = self.screen_name();
        if name.is_none() || name == self.remembered {
            return;
        }
        if let (Some(vault), Some(name)) = (&self.vault, &name)
            && vault.set_last_screen(name).is_ok()
        {
            self.remembered = Some(name.clone());
        }
    }

    /// Store the post being written, remember the screen, and forget the
    /// vault (closing it, so its key is gone): only the password opens it
    /// again, back on the same screen. If the post can't be stored, the
    /// editor says so and stays open.
    fn lock(&mut self) {
        if !self.store_post(Leaving::No) {
            return;
        }
        self.remember_screen();
        self.vault = None;
        self.quit_unsaved = false;
        self.screen = Screen::Unlock(unlock::Unlock::locked());
    }

    /// Right after unlocking: the screen writui was left on, if it's
    /// still there, or else the list.
    fn resume(&mut self) -> Result<()> {
        let last = self.vault()?.last_screen().unwrap_or_default();
        self.remembered = last.clone();
        let post = last.as_deref().and_then(|name| name.strip_prefix("post:")).and_then(|id| id.parse::<i64>().ok());
        let exists = |id| -> Result<bool> { Ok(self.vault()?.list_posts()?.iter().any(|p| p.id == id)) };
        match (last.as_deref(), post) {
            (_, Some(id)) if exists(id)? => self.open_post(id),
            (Some("trash"), _) => self.show_trash(None, None),
            (Some("settings"), _) => self.show_settings(),
            _ => self.show_list(None, None),
        }
    }

    /// The terminal window's title: the post being written, or "writui".
    fn title(&self) -> String {
        match &self.screen {
            Screen::Editor(editor) => editor.title(),
            _ => "writui".to_string(),
        }
    }

    fn autosave_due(&self) -> Option<Instant> {
        match &self.screen {
            Screen::Editor(editor) => editor.autosave_due(),
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        match &mut self.screen {
            Screen::Setup(screen) => screen.render(frame),
            Screen::Unlock(screen) => screen.render(frame),
            Screen::List(screen) => screen.render(frame),
            Screen::Settings(screen) => screen.render(frame),
            Screen::Editor(screen) => {
                // Hints stay hidden unless Ctrl is held,
                // where the terminal can tell us.
                screen.show_chrome = !self.ctrl_reported || self.ctrl_held;
                screen.zen = self.ctrl_reported;
                screen.render(frame)
            }
        }
    }

    fn handle(&mut self, event: Event) -> Action {
        let event = normalize(event);
        if let Event::Mouse(mouse) = &event {
            self.ctrl_held = mouse.modifiers.contains(KeyModifiers::CONTROL);
        }
        if let Event::Key(key) = &event {
            if let KeyCode::Modifier(ModifierKeyCode::LeftControl | ModifierKeyCode::RightControl) =
                key.code
            {
                self.ctrl_held = key.kind != KeyEventKind::Release;
                return Action::None;
            }
            // Every key says which modifiers are down, so a missed release
            // (e.g. while another window had focus) puts itself right.
            self.ctrl_held = key.modifiers.contains(KeyModifiers::CONTROL);
            // Held-down keys repeat; other modifiers on their own do nothing.
            if key.kind == KeyEventKind::Release || matches!(key.code, KeyCode::Modifier(_)) {
                return Action::None;
            }
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            if ctrl && key.code == KeyCode::Char('q') {
                return Action::Quit;
            }
            // Ctrl+O (or Cmd+O) locks, from anywhere once unlocked.
            if key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
                && key.code == KeyCode::Char('o')
                && self.vault.is_some()
            {
                return Action::Lock;
            }
            // Ctrl+C will mean "copy" in the editor; elsewhere it quits.
            if ctrl && key.code == KeyCode::Char('c') && !matches!(self.screen, Screen::Editor(_)) {
                return Action::Quit;
            }
        }
        match &mut self.screen {
            Screen::Setup(screen) => screen.handle(event),
            Screen::Unlock(screen) => screen.handle(event),
            Screen::List(screen) => screen.handle(event),
            Screen::Settings(screen) => screen.handle(event),
            Screen::Editor(screen) => screen.handle(event),
        }
    }

    fn apply(&mut self, action: Action) -> Result<()> {
        match action {
            Action::None => {}
            Action::Quit => {
                if self.store_post(Leaving::Yes) || self.quit_unsaved {
                    self.quit = true;
                } else {
                    self.quit_unsaved = true;
                }
            }
            Action::CreateVault(_) | Action::Unlock(_) => {
                match &mut self.screen {
                    Screen::Setup(screen) => screen.set_busy(true),
                    Screen::Unlock(screen) => screen.set_busy(true),
                    _ => {}
                }
                self.pending = Some(action);
            }
            Action::NewPost => {
                let id = self.vault()?.create_post()?;
                self.open_post(id)?;
            }
            Action::OpenPost(id) => self.open_post(id)?,
            Action::DeletePost { id, select } => {
                let title = self.title_of(id)?;
                self.vault()?.delete_post(id)?;
                self.show_list(select, Some(format!("Moved “{title}” to the Trash.")))?;
            }
            Action::ShowTrash => self.show_trash(None, None)?,
            Action::RestorePost { id, select } => {
                let title = self.title_of(id)?;
                self.vault()?.restore_post(id)?;
                self.show_trash(select, Some(format!("Restored “{title}”.")))?;
            }
            Action::DeletePostForever { id, select } => {
                let title = self.title_of(id)?;
                self.vault()?.delete_post_forever(id)?;
                self.show_trash(select, Some(format!("Deleted “{title}” forever.")))?;
            }
            Action::BackToList(select) => {
                if self.store_post(Leaving::Yes) {
                    self.show_list(select, None)?;
                }
            }
            Action::SaveVersion { name } => {
                let Screen::Editor(editor) = &mut self.screen else { return Ok(()) };
                let vault = self.vault.as_ref().context("the vault is locked");
                match vault.and_then(|vault| vault.create_version(editor.post_id(), &name, &editor.text())) {
                    Ok(version) => editor.version_saved(version),
                    Err(err) => editor.set_error(format!("Couldn't save the version: {err:#}")),
                }
                // The post matches the version, so store that too.
                self.store_post(Leaving::No);
            }
            Action::ShowHistory | Action::LoadVersion(_) => {
                let Screen::Editor(editor) = &mut self.screen else { return Ok(()) };
                let vault = self.vault.as_ref().context("the vault is locked");
                let result = match action {
                    Action::LoadVersion(id) => vault.and_then(|v| v.version(id)).map(|v| editor.show_version(v)),
                    _ => vault.and_then(|v| v.list_versions(editor.post_id())).map(|v| editor.show_history(v)),
                };
                if let Err(err) = result {
                    editor.set_error(format!("Couldn't load the history: {err:#}"));
                }
            }
            Action::Import { path, into } => {
                if let Err(err) = self.import(&path, into)
                    && let Screen::List(list) = &mut self.screen
                {
                    list.import_failed(err);
                }
            }
            Action::Search(filter) => {
                let found = self.vault()?.search(&filter)?;
                if let Screen::List(list) = &mut self.screen {
                    list.show_matches(&filter, &found);
                }
            }
            Action::ShowSettings => self.show_settings()?,
            Action::Lock => self.lock(),
            // Only a convenience: not worth an error.
            Action::SortBy(sort) => {
                let _ = self.vault()?.set_list_sort(sort.name());
            }
            Action::SaveSettings(settings) => {
                if let Err(err) = self.vault()?.set_settings(&settings)
                    && let Screen::Settings(screen) = &mut self.screen
                {
                    screen.set_error(format!("Couldn't save the settings: {err:#}"));
                }
            }
            Action::RecordExport(settings) => {
                let Screen::Editor(editor) = &mut self.screen else { return Ok(()) };
                let vault = self.vault.as_ref().context("the vault is locked");
                if let Err(err) = vault.and_then(|vault| vault.record_export(editor.post_id(), &settings)) {
                    editor.set_error(format!("Exported, but couldn't remember the export: {err:#}"));
                }
            }
        }
        Ok(())
    }

    fn perform(&mut self, action: Action) -> Result<()> {
        match action {
            Action::CreateVault(password) => {
                self.vault = Some(Vault::create(&self.vault_path, &password)?);
                self.show_list(None, None)
            }
            Action::Unlock(password) => match Vault::open(&self.vault_path, &password) {
                Ok(vault) => {
                    self.vault = Some(vault);
                    self.resume()
                }
                Err(OpenError::WrongPassword) => {
                    if let Screen::Unlock(screen) = &mut self.screen {
                        screen.wrong_password();
                    }
                    Ok(())
                }
                Err(OpenError::Other(err)) => Err(err),
            },
            _ => self.apply(action),
        }
    }

    /// Import the file at `path` (see `Action::Import`), then open the
    /// post. If that doesn't work, says why.
    fn import(&mut self, path: &Path, into: ImportInto) -> Result<(), String> {
        let file = files::read(path)?;
        let vault = self.vault.as_ref().ok_or("The vault is locked.")?;
        let failed = |err: anyhow::Error| format!("Couldn't import {}: {err:#}", files::display(path));
        let into = match into {
            ImportInto::Check => {
                let paths = vault.export_paths().map_err(failed)?;
                if let Some(&(id, _)) = paths.iter().find(|(_, p)| files::same_file(p, path)) {
                    let title = self.title_of(id).map_err(failed)?;
                    if let Screen::List(list) = &mut self.screen {
                        list.ask_reimport(path.to_path_buf(), id, &title);
                    }
                    return Ok(());
                }
                None
            }
            ImportInto::New => None,
            ImportInto::Post(id) => Some(id),
        };
        let today = jiff::Zoned::now().date();
        // A file without a date of its own is dated when it was made.
        let made = file.created.map_or(today, |time| time.to_zoned(jiff::tz::TimeZone::system()).date());
        let author = vault.settings().map_err(failed)?.author;
        let suggestion = files::front_matter::suggest(&file.title, &file.body, made, today, &author);
        let front_matter = files::front_matter::with_date(&file.front_matter, &suggestion);
        let settings = ExportSettings {
            path: path.to_path_buf(),
            suggested: files::front_matter::suggested_of(&suggestion, &front_matter),
            front_matter,
            with_front_matter: file.had_front_matter,
            keep_title: file.had_title_line,
        };
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let version_name: String = format!("Before importing {name}").chars().take(80).collect();
        let id = vault.import_post(&file.body, &settings, into, &version_name, file.created).map_err(failed)?;
        self.open_post(id).map_err(failed)?;
        if let Screen::Editor(editor) = &mut self.screen {
            let notice = match file.converted {
                true => format!("Imported {name}. Its TOML front matter is YAML now."),
                false => format!("Imported {name}."),
            };
            editor.set_notice(notice);
        }
        Ok(())
    }

    fn vault(&self) -> Result<&Vault> {
        self.vault.as_ref().context("the vault is locked")
    }

    fn title_of(&self, id: i64) -> Result<String> {
        let post = self.vault()?.post(id)?;
        let title = title_from_first_line(post.body.lines().next().unwrap_or_default());
        Ok(list::display_title(&title).to_string())
    }

    fn show_list(&mut self, select: Option<i64>, notice: Option<String>) -> Result<()> {
        let vault = self.vault()?;
        let mode = list::Mode::Posts { deleted_count: vault.count_deleted()? };
        let folder = files::display_folder(&files::last_folder(vault.latest_export_settings()?.as_ref()));
        let sort = list::Sort::from_name(&vault.list_sort()?.unwrap_or_default());
        let list = list::List::new(mode, vault.list_posts()?, select).with_import_folder(folder).with_sort(sort);
        self.show(list, notice);
        Ok(())
    }

    fn show_trash(&mut self, select: Option<i64>, notice: Option<String>) -> Result<()> {
        let posts = self.vault()?.list_deleted()?;
        self.show(list::List::new(list::Mode::Trash, posts, select), notice);
        Ok(())
    }

    fn show_settings(&mut self) -> Result<()> {
        let values = self.vault()?.settings()?;
        self.screen = Screen::Settings(Box::new(settings::Settings::new(values)));
        Ok(())
    }

    fn show(&mut self, list: list::List, notice: Option<String>) {
        self.screen = Screen::List(Box::new(match notice {
            Some(notice) => list.with_notice(notice),
            None => list,
        }));
    }

    /// Store the open post in the vault, if it changed, and where the cursor
    /// is. If storing it fails, the editor stays open and
    /// says so, so no writing is lost. Returns whether it's safe to leave
    /// the editor.
    fn store_post(&mut self, leaving: Leaving) -> bool {
        let Screen::Editor(editor) = &mut self.screen else {
            return true;
        };
        let vault = self.vault.as_ref().context("the vault is locked");
        if editor.changed() {
            let text = editor.text();
            match vault.and_then(|vault| vault.update_post_body(editor.post_id(), &text)) {
                Ok(()) => editor.mark_stored(Some(text)),
                Err(err) => {
                    editor.store_failed(match leaving {
                        Leaving::Yes => format!(
                            "Couldn't save the post: {err:#}. Ctrl+Q again quits without saving."
                        ),
                        Leaving::No => {
                            format!("Couldn't save the post: {err:#}. Trying again shortly.")
                        }
                    });
                    return false;
                }
            }
        } else {
            // E.g. undone back to what's stored.
            editor.mark_stored(None);
        }
        // Only a convenience: failing to remember it shouldn't keep you in
        // the editor.
        if let Some(vault) = &self.vault {
            let _ = vault.set_post_cursor(editor.post_id(), editor.cursor());
        }
        self.quit_unsaved = false;
        true
    }

    fn open_post(&mut self, id: i64) -> Result<()> {
        let vault = self.vault()?;
        let post = vault.post(id)?;
        let last_version = vault.latest_version(id)?;
        let exports =
            editor::Exports { this_post: vault.export_settings(id)?, latest: vault.latest_export_settings()? };
        let settings = vault.settings()?;
        self.screen = Screen::Editor(Box::new(editor::Editor::new(post, last_version, exports, settings)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyEvent;

    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> Event {
        Event::Key(KeyEvent::new_with_kind(code, modifiers, kind))
    }

    #[test]
    fn ctrl_is_tracked_on_its_own() {
        let mut app = App::new(PathBuf::from("/nonexistent/writui.db"), true);
        let ctrl = KeyCode::Modifier(ModifierKeyCode::LeftControl);
        app.handle(key(ctrl, KeyModifiers::CONTROL, KeyEventKind::Press));
        assert!(app.ctrl_held);
        app.handle(key(ctrl, KeyModifiers::NONE, KeyEventKind::Release));
        assert!(!app.ctrl_held);
        // A missed release puts itself right on the next key.
        app.handle(key(ctrl, KeyModifiers::CONTROL, KeyEventKind::Press));
        app.handle(key(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Press));
        assert!(!app.ctrl_held);
    }

    #[test]
    fn imports_make_a_post_and_ask_before_updating_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("writui.db"), false);
        app.perform(Action::CreateVault(Zeroizing::new("pw".into()))).unwrap();
        let file = dir.path().join("hello.md");
        std::fs::write(&file, "+++\ntitle = \"Hello\"\ndate = 2026-01-01\n+++\n\nFirst text.\n").unwrap();

        app.apply(Action::Import { path: file.clone(), into: ImportInto::Check }).unwrap();
        let Screen::Editor(editor) = &app.screen else { panic!("not opened") };
        let id = editor.post_id();
        assert_eq!(editor.text(), "# Hello\n\nFirst text.");
        let vault = app.vault.as_ref().unwrap();
        let settings = vault.export_settings(id).unwrap().unwrap();
        assert_eq!(settings.path, file);
        assert_eq!(settings.front_matter, "title: \"Hello\"\ndate: 2026-01-01");
        assert!(settings.with_front_matter);
        // Started when the file was.
        let metadata = std::fs::metadata(&file).unwrap();
        let made = metadata.created().or_else(|_| metadata.modified()).unwrap();
        let made = jiff::Timestamp::try_from(made).unwrap();
        assert_eq!(vault.post(id).unwrap().created_at.as_millisecond(), made.as_millisecond());

        // Importing it again asks first.
        std::fs::write(&file, "---\ntitle: Hello\n---\nSecond text.\n").unwrap();
        app.apply(Action::BackToList(None)).unwrap();
        app.apply(Action::Import { path: file.clone(), into: ImportInto::Check }).unwrap();
        assert!(matches!(&app.screen, Screen::List(_)));
        assert_eq!(app.vault.as_ref().unwrap().list_posts().unwrap().len(), 1);
        // Updating keeps the old text as a version.
        app.apply(Action::Import { path: file.clone(), into: ImportInto::Post(id) }).unwrap();
        let Screen::Editor(editor) = &app.screen else { panic!("not opened") };
        assert_eq!((editor.post_id(), editor.text().as_str()), (id, "# Hello\n\nSecond text."));
        let vault = app.vault.as_ref().unwrap();
        let versions = vault.list_versions(id).unwrap();
        assert_eq!(versions[0].name, "Before importing hello.md");
        assert_eq!(vault.version(versions[0].id).unwrap().body, "# Hello\n\nFirst text.");
        // Or it can be a new post.
        app.apply(Action::BackToList(None)).unwrap();
        app.apply(Action::Import { path: file.clone(), into: ImportInto::New }).unwrap();
        assert_eq!(app.vault.as_ref().unwrap().list_posts().unwrap().len(), 2);
        // A file with no date is dated when it was made.
        let plain = dir.path().join("plain.txt");
        std::fs::write(&plain, "Just text.\n").unwrap();
        app.apply(Action::BackToList(None)).unwrap();
        app.apply(Action::Import { path: plain.clone(), into: ImportInto::Check }).unwrap();
        let Screen::Editor(editor) = &app.screen else { panic!("not opened") };
        let settings = app.vault.as_ref().unwrap().export_settings(editor.post_id()).unwrap().unwrap();
        let metadata = std::fs::metadata(&plain).unwrap();
        let made = jiff::Timestamp::try_from(metadata.created().or_else(|_| metadata.modified()).unwrap()).unwrap();
        let made = made.to_zoned(jiff::tz::TimeZone::system()).date();
        assert_eq!(files::front_matter::get(&settings.front_matter, "date"), Some(made.to_string()));
        assert!(!settings.with_front_matter);
        // A file that can't be read leaves the list showing why.
        app.apply(Action::BackToList(None)).unwrap();
        app.apply(Action::Import { path: dir.path().join("gone.md"), into: ImportInto::Check }).unwrap();
        assert!(matches!(&app.screen, Screen::List(_)));
    }

    #[test]
    fn opens_on_the_screen_it_was_left_on() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("writui.db");
        let reopen = || {
            let mut app = App::new(path.clone(), false);
            app.perform(Action::Unlock(Zeroizing::new("pw".into()))).unwrap();
            app
        };
        let mut app = App::new(path.clone(), false);
        app.perform(Action::CreateVault(Zeroizing::new("pw".into()))).unwrap();
        app.apply(Action::NewPost).unwrap();
        app.remember_screen();
        let Screen::Editor(editor) = &app.screen else { panic!("not opened") };
        let id = editor.post_id();
        drop(app);

        let mut app = reopen();
        assert!(matches!(&app.screen, Screen::Editor(editor) if editor.post_id() == id));
        app.apply(Action::ShowSettings).unwrap();
        app.remember_screen();
        let mut app = reopen();
        assert!(matches!(&app.screen, Screen::Settings(_)));
        // A post that's gone since: the list.
        let vault = app.vault.as_ref().unwrap();
        vault.set_last_screen(&format!("post:{id}")).unwrap();
        vault.delete_post(id).unwrap();
        app.apply(Action::BackToList(None)).unwrap();
        app.remember_screen();
        let app = reopen();
        assert!(matches!(&app.screen, Screen::List(list) if !list.trash_view()));
    }

    #[test]
    fn ctrl_o_locks_and_unlocking_comes_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("writui.db"), false);
        app.perform(Action::CreateVault(Zeroizing::new("pw".into()))).unwrap();
        app.apply(Action::NewPost).unwrap();
        app.remember_screen();
        app.handle(key(KeyCode::Char('x'), KeyModifiers::NONE, KeyEventKind::Press));
        let Screen::Editor(editor) = &app.screen else { panic!("not opened") };
        let id = editor.post_id();
        // Ctrl+O: the typing is stored, the vault closed.
        let action = app.handle(key(KeyCode::Char('o'), KeyModifiers::CONTROL, KeyEventKind::Press));
        app.apply(action).unwrap();
        assert!(matches!(app.screen, Screen::Unlock(_)) && app.vault.is_none());
        assert_eq!(app.title(), "writui");
        // Locked, Ctrl+O does nothing; the password opens the same post.
        assert!(matches!(app.handle(key(KeyCode::Char('o'), KeyModifiers::CONTROL, KeyEventKind::Press)), Action::None));
        app.perform(Action::Unlock(Zeroizing::new("pw".into()))).unwrap();
        let Screen::Editor(editor) = &app.screen else { panic!("not reopened") };
        assert_eq!((editor.post_id(), editor.text().as_str()), (id, "# x"));
    }

    #[test]
    fn caps_lock_capitalises() {
        let mut event = KeyEvent::new_with_kind(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Press);
        event.state = KeyEventState::CAPS_LOCK;
        let Event::Key(key) = normalize(Event::Key(event)) else { panic!() };
        assert_eq!(key.code, KeyCode::Char('A'));
    }
}
