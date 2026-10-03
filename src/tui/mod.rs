//! The terminal UI: terminal setup, the event loop, and switching screens.

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
use ratatui::crossterm::execute;
use ratatui::{DefaultTerminal, Frame};
use zeroize::Zeroizing;

use crate::vault::{OpenError, Vault};

/// What a screen asks the app to do in response to an event.
pub enum Action {
    None,
    Quit,
    CreateVault(Zeroizing<String>),
    Unlock(Zeroizing<String>),
    NewPost,
    OpenPost(i64),
    DeletePost(i64),
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
        let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
        hook(info);
    }));
    let result = App::new(vault_path).run(&mut terminal);
    let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
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
    quit: bool,
}

impl App {
    fn new(vault_path: PathBuf) -> Self {
        let screen = if vault_path.exists() {
            Screen::Unlock(unlock::Unlock::default())
        } else {
            Screen::Setup(setup::Setup::new(vault_path.clone()))
        };
        App { vault_path, vault: None, screen, pending: None, quit: false }
    }

    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.quit {
            terminal.draw(|frame| self.render(frame))?;
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
            Action::Quit => self.quit = true,
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
            Action::DeletePost(id) => {
                self.vault()?.delete_post(id)?;
                self.show_list(None)?;
            }
            Action::BackToList(select) => self.show_list(select)?,
        }
        Ok(())
    }

    fn perform(&mut self, action: Action) -> Result<()> {
        match action {
            Action::CreateVault(password) => {
                self.vault = Some(Vault::create(&self.vault_path, &password)?);
                self.show_list(None)
            }
            Action::Unlock(password) => match Vault::open(&self.vault_path, &password) {
                Ok(vault) => {
                    self.vault = Some(vault);
                    self.show_list(None)
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

    fn show_list(&mut self, select: Option<i64>) -> Result<()> {
        let posts = self.vault()?.list_posts()?;
        self.screen = Screen::List(list::List::new(posts, select));
        Ok(())
    }

    fn open_post(&mut self, id: i64) -> Result<()> {
        let post = self.vault()?.post(id)?;
        self.screen = Screen::Editor(editor::Editor::new(post));
        Ok(())
    }
}
