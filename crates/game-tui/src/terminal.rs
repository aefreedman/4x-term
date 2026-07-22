use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io::{self, stdout};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalEvent {
    Key(KeyEvent),
    Resize { width: u16, height: u16 },
    Wake,
}

/// Synchronous event boundary used by the production crossterm loop and tests.
pub trait EventSource {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;
    fn read(&mut self) -> io::Result<TerminalEvent>;
}

#[derive(Default)]
pub struct CrosstermEvents;

impl EventSource for CrosstermEvents {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    fn read(&mut self) -> io::Result<TerminalEvent> {
        match event::read()? {
            Event::Key(key) => Ok(TerminalEvent::Key(key)),
            Event::Resize(width, height) => Ok(TerminalEvent::Resize { width, height }),
            Event::FocusGained | Event::FocusLost | Event::Mouse(_) | Event::Paste(_) => {
                Ok(TerminalEvent::Wake)
            }
        }
    }
}

/// Every terminal mode transition is injectable and independently fallible.
pub trait TerminalOps {
    fn enable_raw(&mut self) -> io::Result<()>;
    fn disable_raw(&mut self) -> io::Result<()>;
    fn enter_alternate_screen(&mut self) -> io::Result<()>;
    fn leave_alternate_screen(&mut self) -> io::Result<()>;
    fn hide_cursor(&mut self) -> io::Result<()>;
    fn show_cursor(&mut self) -> io::Result<()>;
}

#[derive(Default)]
pub struct CrosstermTerminalOps;

impl TerminalOps for CrosstermTerminalOps {
    fn enable_raw(&mut self) -> io::Result<()> {
        enable_raw_mode()
    }

    fn disable_raw(&mut self) -> io::Result<()> {
        disable_raw_mode()
    }

    fn enter_alternate_screen(&mut self) -> io::Result<()> {
        execute!(stdout(), EnterAlternateScreen).map(|_| ())
    }

    fn leave_alternate_screen(&mut self) -> io::Result<()> {
        execute!(stdout(), LeaveAlternateScreen).map(|_| ())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(stdout(), Hide).map(|_| ())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(stdout(), Show).map(|_| ())
    }
}

/// Staged RAII acquisition. The guard exists before the first mode is acquired,
/// so failure at any later stage cleans up completed stages in reverse order.
pub struct TerminalGuard<O: TerminalOps> {
    ops: O,
    raw: bool,
    alternate: bool,
    cursor_hidden: bool,
}

impl<O: TerminalOps> TerminalGuard<O> {
    pub fn acquire(ops: O) -> io::Result<Self> {
        let mut guard = Self {
            ops,
            raw: false,
            alternate: false,
            cursor_hidden: false,
        };
        guard.ops.enable_raw()?;
        guard.raw = true;
        guard.ops.enter_alternate_screen()?;
        guard.alternate = true;
        guard.ops.hide_cursor()?;
        guard.cursor_hidden = true;
        Ok(guard)
    }
}

impl<O: TerminalOps> Drop for TerminalGuard<O> {
    fn drop(&mut self) {
        if self.cursor_hidden {
            let _ = self.ops.show_cursor();
            self.cursor_hidden = false;
        }
        if self.alternate {
            let _ = self.ops.leave_alternate_screen();
            self.alternate = false;
        }
        if self.raw {
            let _ = self.ops.disable_raw();
            self.raw = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone)]
    struct MockOps {
        calls: Rc<RefCell<Vec<&'static str>>>,
        fail_at: Option<&'static str>,
    }

    impl MockOps {
        fn call(&self, name: &'static str) -> io::Result<()> {
            self.calls.borrow_mut().push(name);
            if self.fail_at == Some(name) {
                Err(io::Error::other("injected setup failure"))
            } else {
                Ok(())
            }
        }
    }

    impl TerminalOps for MockOps {
        fn enable_raw(&mut self) -> io::Result<()> {
            self.call("raw+")
        }
        fn disable_raw(&mut self) -> io::Result<()> {
            self.call("raw-")
        }
        fn enter_alternate_screen(&mut self) -> io::Result<()> {
            self.call("alt+")
        }
        fn leave_alternate_screen(&mut self) -> io::Result<()> {
            self.call("alt-")
        }
        fn hide_cursor(&mut self) -> io::Result<()> {
            self.call("cursor-")
        }
        fn show_cursor(&mut self) -> io::Result<()> {
            self.call("cursor+")
        }
    }

    fn run(fail_at: Option<&'static str>) -> Vec<&'static str> {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let result = TerminalGuard::acquire(MockOps {
            calls: calls.clone(),
            fail_at,
        });
        drop(result);
        calls.borrow().clone()
    }

    #[test]
    fn normal_cleanup_is_reverse_order() {
        assert_eq!(
            run(None),
            ["raw+", "alt+", "cursor-", "cursor+", "alt-", "raw-"]
        );
    }

    #[test]
    fn every_partial_setup_failure_cleans_completed_stages() {
        assert_eq!(run(Some("raw+")), ["raw+"]);
        assert_eq!(run(Some("alt+")), ["raw+", "alt+", "raw-"]);
        assert_eq!(
            run(Some("cursor-")),
            ["raw+", "alt+", "cursor-", "alt-", "raw-"]
        );
    }
}
