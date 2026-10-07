//! The terminal UI: terminal setup, the event loop, and switching screens.

mod dialog;
mod editor;
mod hints;
mod list;
mod setup;
mod unlock;
mod widgets;

use std::io::stdout;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEventKind, KeyEventState, KeyModifiers, KeyboardEnhancementFlags,
    ModifierKeyCode, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::{DefaultTerminal, Frame};
use zeroize::Zeroizing;

use crate::vault::{OpenError, Vault, title_from_first_line};

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
    /// Record a save of the open post.
    SavePost { name: String },
    /// Show the open post's history.
    ShowHistory,
    /// Open a save in the history, to read.
    LoadSave(i64),
}

/// Whether the draft is being stored because the editor is being left.
#[derive(Clone, Copy)]
enum Leaving {
    Yes,
    No,
}

enum Screen {
    Setup(setup::Setup),
    Unlock(unlock::Unlock),
    List(list::List),
    Editor(Box<editor::Editor>),
}

pub fn run(vault_path: PathBuf) -> Result<()> {
    let mut terminal = ratatui::init();
    execute!(stdout(), EnableMouseCapture, EnableBracketedPaste)?;
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

fn give_back_terminal(ctrl_reported: bool) {
    if ctrl_reported {
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(
        stdout(),
        DisableMouseCapture,
        DisableBracketedPaste,
        SetCursorStyle::DefaultUserShape
    );
}

/// Smooth over the keyboard protocol: with every key reported as a code,
/// Caps Lock no longer capitalises letters, so do that here.
fn normalize(event: Event) -> Event {
    match event {
        Event::Key(mut key)
            if key.state.contains(KeyEventState::CAPS_LOCK)
                && !key.modifiers.contains(KeyModifiers::SHIFT) =>
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
    /// Storing the draft failed on the last Ctrl+Q; the next one quits anyway.
    quit_unsaved: bool,
    /// The cursor is currently a bar (in the editor) rather than the
    /// terminal's own shape.
    bar_cursor: bool,
    /// The terminal reports Ctrl on its own (see `run`).
    ctrl_reported: bool,
    /// Ctrl is held down right now.
    ctrl_held: bool,
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
            if let Some(action) = self.pending.take() {
                self.perform(action)?;
                continue;
            }
            // Wait for input, but wake up to store the draft when it's due.
            let due = self.autosave_due();
            if due.is_some_and(|due| due <= Instant::now()) {
                self.store_draft(Leaving::No);
                continue;
            }
            if let Some(due) = due
                && !event::poll(due.saturating_duration_since(Instant::now()))?
            {
                continue;
            }
            let action = self.handle(event::read()?);
            self.apply(action)?;
        }
        Ok(())
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
            Screen::Editor(screen) => {
                // Hints and the saved state stay hidden unless Ctrl is held,
                // where the terminal can tell us.
                screen.show_chrome = !self.ctrl_reported || self.ctrl_held;
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
            // Ctrl+C will mean "copy" in the editor; elsewhere it quits.
            if ctrl && key.code == KeyCode::Char('c') && !matches!(self.screen, Screen::Editor(_)) {
                return Action::Quit;
            }
        }
        match &mut self.screen {
            Screen::Setup(screen) => screen.handle(event),
            Screen::Unlock(screen) => screen.handle(event),
            Screen::List(screen) => screen.handle(event),
            Screen::Editor(screen) => screen.handle(event),
        }
    }

    fn apply(&mut self, action: Action) -> Result<()> {
        match action {
            Action::None => {}
            Action::Quit => {
                if self.store_draft(Leaving::Yes) || self.quit_unsaved {
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
                if self.store_draft(Leaving::Yes) {
                    self.show_list(select, None)?;
                }
            }
            Action::SavePost { name } => {
                let Screen::Editor(editor) = &mut self.screen else { return Ok(()) };
                let vault = self.vault.as_ref().context("the vault is locked");
                match vault.and_then(|vault| vault.create_save(editor.post_id(), &name, &editor.text())) {
                    Ok(save) => editor.saved(save),
                    Err(err) => editor.set_error(format!("Couldn't save: {err:#}")),
                }
                // The draft matches the save, so store that too.
                self.store_draft(Leaving::No);
            }
            Action::ShowHistory | Action::LoadSave(_) => {
                let Screen::Editor(editor) = &mut self.screen else { return Ok(()) };
                let vault = self.vault.as_ref().context("the vault is locked");
                let result = match action {
                    Action::LoadSave(id) => vault.and_then(|v| v.save(id)).map(|s| editor.show_save(s)),
                    _ => vault.and_then(|v| v.list_saves(editor.post_id())).map(|s| editor.show_history(s)),
                };
                if let Err(err) = result {
                    editor.set_error(format!("Couldn't load the history: {err:#}"));
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
                    self.show_list(None, None)
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
        self.show(list::List::new(mode, vault.list_posts()?, select), notice);
        Ok(())
    }

    fn show_trash(&mut self, select: Option<i64>, notice: Option<String>) -> Result<()> {
        let posts = self.vault()?.list_deleted()?;
        self.show(list::List::new(list::Mode::Trash, posts, select), notice);
        Ok(())
    }

    fn show(&mut self, list: list::List, notice: Option<String>) {
        self.screen = Screen::List(match notice {
            Some(notice) => list.with_notice(notice),
            None => list,
        });
    }

    /// Store the open post's draft in the vault, if it changed, and where
    /// the cursor is. If storing the draft fails, the editor stays open and
    /// says so, so no writing is lost. Returns whether it's safe to leave
    /// the editor.
    fn store_draft(&mut self, leaving: Leaving) -> bool {
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
                            "Couldn't save the draft: {err:#}. Ctrl+Q again quits without saving."
                        ),
                        Leaving::No => {
                            format!("Couldn't save the draft: {err:#}. Trying again shortly.")
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
        let last_save = vault.latest_save(id)?;
        self.screen = Screen::Editor(Box::new(editor::Editor::new(post, last_save)));
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
    fn caps_lock_capitalises() {
        let mut event = KeyEvent::new_with_kind(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Press);
        event.state = KeyEventState::CAPS_LOCK;
        let Event::Key(key) = normalize(Event::Key(event)) else { panic!() };
        assert_eq!(key.code, KeyCode::Char('A'));
    }
}
