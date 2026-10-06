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

use anyhow::{Context, Result};
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEventKind, KeyModifiers,
};
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::execute;
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
}

enum Screen {
    Setup(setup::Setup),
    Unlock(unlock::Unlock),
    List(list::List),
    Editor(editor::Editor),
}

pub fn run(vault_path: PathBuf) -> Result<()> {
    let mut terminal = ratatui::init();
    execute!(stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    // ratatui's own panic hook restores the screen; also give the terminal
    // its mouse back.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(
            stdout(),
            DisableMouseCapture,
            DisableBracketedPaste,
            SetCursorStyle::DefaultUserShape
        );
        hook(info);
    }));
    let result = App::new(vault_path).run(&mut terminal);
    let _ = execute!(
        stdout(),
        DisableMouseCapture,
        DisableBracketedPaste,
        SetCursorStyle::DefaultUserShape
    );
    ratatui::restore();
    result
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
    quit: bool,
}

impl App {
    fn new(vault_path: PathBuf) -> Self {
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
            let action = self.handle(event::read()?);
            self.apply(action)?;
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        match &mut self.screen {
            Screen::Setup(screen) => screen.render(frame),
            Screen::Unlock(screen) => screen.render(frame),
            Screen::List(screen) => screen.render(frame),
            Screen::Editor(screen) => screen.render(frame),
        }
    }

    fn handle(&mut self, event: Event) -> Action {
        if let Event::Key(key) = &event {
            if key.kind != KeyEventKind::Press {
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
                if self.store_draft() || self.quit_unsaved {
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
                if self.store_draft() {
                    self.show_list(select, None)?;
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
    fn store_draft(&mut self) -> bool {
        let Screen::Editor(editor) = &mut self.screen else {
            return true;
        };
        let vault = self.vault.as_ref().context("the vault is locked");
        if editor.changed() {
            let text = editor.text();
            match vault.and_then(|vault| vault.update_post_body(editor.post_id(), &text)) {
                Ok(()) => editor.mark_stored(text),
                Err(err) => {
                    editor.set_error(format!(
                        "Couldn't save the draft: {err:#}. Ctrl+Q again quits without saving."
                    ));
                    return false;
                }
            }
        }
        // Only a convenience: failing to remember it shouldn't keep you in
        // the editor.
        if let Some(vault) = &self.vault {
            let _ = vault.set_post_cursor(editor.post_id(), editor.cursor());
        }
        true
    }

    fn open_post(&mut self, id: i64) -> Result<()> {
        let post = self.vault()?.post(id)?;
        self.screen = Screen::Editor(editor::Editor::new(post));
        Ok(())
    }
}
