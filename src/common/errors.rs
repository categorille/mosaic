//! All things related to errors and error contexts.

use super::{AppInstruction, OPENCALLS};
use crate::pty_bus::PtyInstruction;
use crate::screen::ScreenInstruction;

use std::fmt::{Display, Error, Formatter};

const MAX_THREAD_CALL_STACK: usize = 6;

#[cfg(not(test))]
use super::SenderWithContext;
#[cfg(not(test))]
use std::panic::PanicInfo;
#[cfg(not(test))]
pub fn handle_panic(
    info: &PanicInfo<'_>,
    send_app_instructions: &SenderWithContext<AppInstruction>,
) {
    use backtrace::Backtrace;
    use std::{process, thread};
    let backtrace = Backtrace::new();
    let thread = thread::current();
    let thread = thread.name().unwrap_or("unnamed");

    let msg = match info.payload().downcast_ref::<&'static str>() {
        Some(s) => Some(*s),
        None => info.payload().downcast_ref::<String>().map(|s| &**s),
    };

    let err_ctx = OPENCALLS.with(|ctx| *ctx.borrow());

    let backtrace = match (info.location(), msg) {
        (Some(location), Some(msg)) => format!(
            "{}\n\u{1b}[0;0mError: \u{1b}[0;31mthread '{}' panicked at '{}': {}:{}\n\u{1b}[0;0m{:?}",
            err_ctx,
            thread,
            msg,
            location.file(),
            location.line(),
            backtrace
        ),
        (Some(location), None) => format!(
            "{}\n\u{1b}[0;0mError: \u{1b}[0;31mthread '{}' panicked: {}:{}\n\u{1b}[0;0m{:?}",
            err_ctx,
            thread,
            location.file(),
            location.line(),
            backtrace
        ),
        (None, Some(msg)) => format!(
            "{}\n\u{1b}[0;0mError: \u{1b}[0;31mthread '{}' panicked at '{}'\n\u{1b}[0;0m{:?}",
            err_ctx, thread, msg, backtrace
        ),
        (None, None) => format!(
            "{}\n\u{1b}[0;0mError: \u{1b}[0;31mthread '{}' panicked\n\u{1b}[0;0m{:?}",
            err_ctx, thread, backtrace
        ),
    };

    if thread == "main" {
        println!("{}", backtrace);
        process::exit(1);
    } else {
        send_app_instructions
            .send(AppInstruction::Error(backtrace))
            .unwrap();
    }
}

/// An [`ErrorContext`] struct contains a representation of the call stack
#[derive(Clone, Copy)]
pub struct ErrorContext {
    calls: [ContextType; MAX_THREAD_CALL_STACK],
}

impl ErrorContext {
    pub fn new() -> Self {
        Self {
            calls: [ContextType::Empty; MAX_THREAD_CALL_STACK],
        }
    }

    pub fn add_call(&mut self, call: ContextType) {
        for ctx in self.calls.iter_mut() {
            if *ctx == ContextType::Empty {
                *ctx = call;
                break;
            }
        }
        OPENCALLS.with(|ctx| *ctx.borrow_mut() = *self);
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for ErrorContext {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        writeln!(f, "Originating Thread(s):")?;
        for (index, ctx) in self.calls.iter().enumerate() {
            if *ctx == ContextType::Empty {
                break;
            }
            writeln!(f, "\u{1b}[0;0m{}. {}", index + 1, ctx)?;
        }
        Ok(())
    }
}

/// Different types of contexts that form an [`ErrorContext`] call stack.
///
/// Complex variants store a variant of a related enum, whose variants can be built from
/// the related custom Zellij MSPC instruction enum variants ([`ScreenInstruction`],
/// [`PtyInstruction`], [`AppInstruction`], etc.
#[derive(Copy, Clone, PartialEq)]
pub enum ContextType {
    Screen(ScreenContext),
    Pty(PtyContext),
    Plugin(PluginContext),
    App(AppContext),
    IPCServer,
    StdinHandler,
    AsyncTask,
    Empty,
}

// TODO use the `colored` crate for color formatting
impl Display for ContextType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        let purple = "\u{1b}[1;35m";
        let green = "\u{1b}[0;32m";
        match *self {
            ContextType::Screen(c) => write!(f, "{}screen_thread: {}{:?}", purple, green, c),
            ContextType::Pty(c) => write!(f, "{}pty_thread: {}{:?}", purple, green, c),

            ContextType::Plugin(c) => write!(f, "{}plugin_thread: {}{:?}", purple, green, c),
            ContextType::App(c) => write!(f, "{}main_thread: {}{:?}", purple, green, c),
            ContextType::IPCServer => write!(f, "{}ipc_server: {}AcceptInput", purple, green),
            ContextType::StdinHandler => {
                write!(f, "{}stdin_handler_thread: {}AcceptInput", purple, green)
            }
            ContextType::AsyncTask => {
                write!(f, "{}stream_terminal_bytes: {}AsyncTask", purple, green)
            }
            ContextType::Empty => write!(f, ""),
        }
    }
}

/// An element of the error context related to [`ScreenInstruction`]s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScreenContext {
    HandlePtyEvent,
    Render,
    NewPane,
    HorizontalSplit,
    VerticalSplit,
    WriteCharacter,
    ResizeLeft,
    ResizeRight,
    ResizeDown,
    ResizeUp,
    MoveFocus,
    MoveFocusLeft,
    MoveFocusDown,
    MoveFocusUp,
    MoveFocusRight,
    Quit,
    ScrollUp,
    ScrollDown,
    ClearScroll,
    CloseFocusedPane,
    ToggleActiveTerminalFullscreen,
    SetSelectable,
    SetInvisibleBorders,
    SetMaxHeight,
    ClosePane,
    ApplyLayout,
    NewTab,
    SwitchTabNext,
    SwitchTabPrev,
    CloseTab,
}

impl From<&ScreenInstruction> for ScreenContext {
    fn from(screen_instruction: &ScreenInstruction) -> Self {
        match *screen_instruction {
            ScreenInstruction::Pty(..) => ScreenContext::HandlePtyEvent,
            ScreenInstruction::Render => ScreenContext::Render,
            ScreenInstruction::NewPane(_) => ScreenContext::NewPane,
            ScreenInstruction::HorizontalSplit(_) => ScreenContext::HorizontalSplit,
            ScreenInstruction::VerticalSplit(_) => ScreenContext::VerticalSplit,
            ScreenInstruction::WriteCharacter(_) => ScreenContext::WriteCharacter,
            ScreenInstruction::ResizeLeft => ScreenContext::ResizeLeft,
            ScreenInstruction::ResizeRight => ScreenContext::ResizeRight,
            ScreenInstruction::ResizeDown => ScreenContext::ResizeDown,
            ScreenInstruction::ResizeUp => ScreenContext::ResizeUp,
            ScreenInstruction::MoveFocus => ScreenContext::MoveFocus,
            ScreenInstruction::MoveFocusLeft => ScreenContext::MoveFocusLeft,
            ScreenInstruction::MoveFocusDown => ScreenContext::MoveFocusDown,
            ScreenInstruction::MoveFocusUp => ScreenContext::MoveFocusUp,
            ScreenInstruction::MoveFocusRight => ScreenContext::MoveFocusRight,
            ScreenInstruction::Quit => ScreenContext::Quit,
            ScreenInstruction::ScrollUp => ScreenContext::ScrollUp,
            ScreenInstruction::ScrollDown => ScreenContext::ScrollDown,
            ScreenInstruction::ClearScroll => ScreenContext::ClearScroll,
            ScreenInstruction::CloseFocusedPane => ScreenContext::CloseFocusedPane,
            ScreenInstruction::ToggleActiveTerminalFullscreen => {
                ScreenContext::ToggleActiveTerminalFullscreen
            }
            ScreenInstruction::SetSelectable(..) => ScreenContext::SetSelectable,
            ScreenInstruction::SetInvisibleBorders(..) => ScreenContext::SetInvisibleBorders,
            ScreenInstruction::SetMaxHeight(..) => ScreenContext::SetMaxHeight,
            ScreenInstruction::ClosePane(_) => ScreenContext::ClosePane,
            ScreenInstruction::ApplyLayout(_) => ScreenContext::ApplyLayout,
            ScreenInstruction::NewTab(_) => ScreenContext::NewTab,
            ScreenInstruction::SwitchTabNext => ScreenContext::SwitchTabNext,
            ScreenInstruction::SwitchTabPrev => ScreenContext::SwitchTabPrev,
            ScreenInstruction::CloseTab => ScreenContext::CloseTab,
        }
    }
}

/// An element of the error context related to [`PtyInstruction`]s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PtyContext {
    SpawnTerminal,
    SpawnTerminalVertically,
    SpawnTerminalHorizontally,
    NewTab,
    ClosePane,
    CloseTab,
    Quit,
}

impl From<&PtyInstruction> for PtyContext {
    fn from(pty_instruction: &PtyInstruction) -> Self {
        match *pty_instruction {
            PtyInstruction::SpawnTerminal(_) => PtyContext::SpawnTerminal,
            PtyInstruction::SpawnTerminalVertically(_) => PtyContext::SpawnTerminalVertically,
            PtyInstruction::SpawnTerminalHorizontally(_) => PtyContext::SpawnTerminalHorizontally,
            PtyInstruction::ClosePane(_) => PtyContext::ClosePane,
            PtyInstruction::CloseTab(_) => PtyContext::CloseTab,
            PtyInstruction::NewTab => PtyContext::NewTab,
            PtyInstruction::Quit => PtyContext::Quit,
        }
    }
}

// FIXME: This whole pattern *needs* a macro eventually, it's soul-crushing to write

use crate::wasm_vm::PluginInstruction;

/// An element of the error context related to [`PluginInstruction`]s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PluginContext {
    Load,
    Draw,
    Input,
    GlobalInput,
    Unload,
    Quit,
}

impl From<&PluginInstruction> for PluginContext {
    fn from(plugin_instruction: &PluginInstruction) -> Self {
        match *plugin_instruction {
            PluginInstruction::Load(..) => PluginContext::Load,
            PluginInstruction::Draw(..) => PluginContext::Draw,
            PluginInstruction::Input(..) => PluginContext::Input,
            PluginInstruction::GlobalInput(_) => PluginContext::GlobalInput,
            PluginInstruction::Unload(_) => PluginContext::Unload,
            PluginInstruction::Quit => PluginContext::Quit,
        }
    }
}

/// An element of the error context related to [`AppInstruction`]s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppContext {
    GetState,
    SetState,
    Exit,
    Error,
}

impl From<&AppInstruction> for AppContext {
    fn from(app_instruction: &AppInstruction) -> Self {
        match *app_instruction {
            AppInstruction::GetState(_) => AppContext::GetState,
            AppInstruction::SetState(_) => AppContext::SetState,
            AppInstruction::Exit => AppContext::Exit,
            AppInstruction::Error(_) => AppContext::Error,
        }
    }
}
