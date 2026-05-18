#![forbid(unsafe_code)]

mod app;
mod cache;
mod fetch;
mod palette;
mod routes;
mod theme;
mod ui;

use std::{io, time::Duration};

use anyhow::{Result, anyhow};
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::app::App;

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, app))?;
        app.on_tick();

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key_event) = event::read()?
            && key_event.kind == KeyEventKind::Press
        {
            app.on_key(key_event);
        }
    }

    Ok(())
}

trait TerminalCleanupOps {
    fn disable_raw_mode(&mut self) -> Result<()>;
    fn leave_alternate_screen(&mut self) -> Result<()>;
    fn show_cursor(&mut self) -> Result<()>;
}

#[derive(Default)]
struct TerminalSession {
    raw_mode_enabled: bool,
    alternate_screen_enabled: bool,
    cursor_restore_needed: bool,
}

impl TerminalSession {
    fn enable_raw_mode(&mut self) -> Result<()> {
        enable_raw_mode()?;
        self.raw_mode_enabled = true;
        Ok(())
    }

    fn enter_alternate_screen(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        execute!(stdout, EnterAlternateScreen)?;
        self.alternate_screen_enabled = true;
        self.cursor_restore_needed = true;
        Ok(())
    }

    fn cleanup_with_ops(&mut self, ops: &mut impl TerminalCleanupOps) -> Result<()> {
        let mut errors = Vec::new();

        if self.raw_mode_enabled {
            if let Err(error) = ops.disable_raw_mode() {
                errors.push(format!("disable raw mode: {error:#}"));
            }
            self.raw_mode_enabled = false;
        }

        if self.alternate_screen_enabled {
            if let Err(error) = ops.leave_alternate_screen() {
                errors.push(format!("leave alternate screen: {error:#}"));
            }
            self.alternate_screen_enabled = false;
        }

        if self.cursor_restore_needed {
            if let Err(error) = ops.show_cursor() {
                errors.push(format!("show cursor: {error:#}"));
            }
            self.cursor_restore_needed = false;
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("terminal cleanup failed: {}", errors.join("; ")))
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let mut ops = StdoutCleanupOps;
        if let Err(error) = self.cleanup_with_ops(&mut ops) {
            eprintln!("{error:#}");
        }
    }
}

struct TerminalBackendCleanupOps<'terminal> {
    terminal: &'terminal mut Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalCleanupOps for TerminalBackendCleanupOps<'_> {
    fn disable_raw_mode(&mut self) -> Result<()> {
        disable_raw_mode()?;
        Ok(())
    }

    fn leave_alternate_screen(&mut self) -> Result<()> {
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<()> {
        self.terminal.show_cursor()?;
        Ok(())
    }
}

struct StdoutCleanupOps;

impl TerminalCleanupOps for StdoutCleanupOps {
    fn disable_raw_mode(&mut self) -> Result<()> {
        disable_raw_mode()?;
        Ok(())
    }

    fn leave_alternate_screen(&mut self) -> Result<()> {
        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen)?;
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<()> {
        let mut stdout = io::stdout();
        execute!(stdout, Show)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    let mut session = TerminalSession::default();
    session.enable_raw_mode()?;
    let mut stdout = io::stdout();
    session.enter_alternate_screen(&mut stdout)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();

    let run_result = run_app(&mut terminal, &mut app);
    let cleanup_result = session.cleanup_with_ops(&mut TerminalBackendCleanupOps {
        terminal: &mut terminal,
    });

    match (run_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(run_error), Err(cleanup_error)) => {
            eprintln!("{cleanup_error:#}");
            Err(run_error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingCleanupOps {
        calls: Vec<&'static str>,
        fail_disable_raw_mode: bool,
        fail_leave_alternate_screen: bool,
        fail_show_cursor: bool,
    }

    impl TerminalCleanupOps for RecordingCleanupOps {
        fn disable_raw_mode(&mut self) -> Result<()> {
            self.calls.push("disable_raw_mode");
            if self.fail_disable_raw_mode {
                Err(anyhow!("raw mode failure"))
            } else {
                Ok(())
            }
        }

        fn leave_alternate_screen(&mut self) -> Result<()> {
            self.calls.push("leave_alternate_screen");
            if self.fail_leave_alternate_screen {
                Err(anyhow!("alternate screen failure"))
            } else {
                Ok(())
            }
        }

        fn show_cursor(&mut self) -> Result<()> {
            self.calls.push("show_cursor");
            if self.fail_show_cursor {
                Err(anyhow!("show cursor failure"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn cleanup_attempts_all_steps_after_failures() {
        let mut session = TerminalSession {
            raw_mode_enabled: true,
            alternate_screen_enabled: true,
            cursor_restore_needed: true,
        };
        let mut ops = RecordingCleanupOps {
            fail_disable_raw_mode: true,
            fail_leave_alternate_screen: true,
            fail_show_cursor: true,
            ..RecordingCleanupOps::default()
        };

        let error = session.cleanup_with_ops(&mut ops).unwrap_err();

        assert_eq!(
            ops.calls,
            ["disable_raw_mode", "leave_alternate_screen", "show_cursor"]
        );
        assert!(error.to_string().contains("disable raw mode"));
        assert!(error.to_string().contains("leave alternate screen"));
        assert!(error.to_string().contains("show cursor"));
        assert!(!session.raw_mode_enabled);
        assert!(!session.alternate_screen_enabled);
        assert!(!session.cursor_restore_needed);
    }

    #[test]
    fn cleanup_after_raw_mode_only_setup_failure_only_disables_raw_mode() {
        let mut session = TerminalSession {
            raw_mode_enabled: true,
            alternate_screen_enabled: false,
            cursor_restore_needed: false,
        };
        let mut ops = RecordingCleanupOps::default();

        session.cleanup_with_ops(&mut ops).unwrap();

        assert_eq!(ops.calls, ["disable_raw_mode"]);
        assert!(!session.raw_mode_enabled);
    }
}
