use std::fmt;
use std::io::prelude::*;
use std::io;

use term::color::{Color, BLACK, RED, GREEN, YELLOW, CYAN};
use term::{self, Terminal, TerminfoTerminal, color, Attr};

use self::AdequateTerminal::{NoColor, Colored};
use self::Verbosity::{Verbose, Quiet};
use self::ColorConfig::{Auto, Always, Never};

use util::errors::CargoResult;

#[derive(Clone, Copy, PartialEq)]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    color: Color,
    attr: Option<Attr>,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            color: BLACK,
            attr: None,
        }
    }
}

impl Style {
    fn from_str(config: &str, mut color: Color, mut attr: Option<Attr>) -> Self {
        let parts = config.split(";").collect::<Vec<_>>();
        if let Some(p) = parts.get(0) {
            if let Ok(p) = p.parse() {
                match p {
                    0 if attr == Some(Attr::Bold) => {
                        attr = None;
                    }
                    1 => {
                        attr = Some(Attr::Bold);
                    }
                    3 => {
                        attr = Some(Attr::Italic(true));
                    }
                    4 => {
                        attr = Some(Attr::Underline(true));
                    }
                    5 | 6 => {
                        attr = Some(Attr::Blink)
                    }
                    i if i >= 30 && i <= 39 => {
                        color = i
                    }
                    _ => {
                        // ignore everything else
                    }
                }
            }
        }
        if let Some(p) = parts.get(1) {
            if let Ok(p) = p.parse() {
                color = p;
            }
        }
        Style {
            color: color,
            attr: attr,
        }
    }

    fn apply(&self, shell: &mut Shell) -> CargoResult<()> {
        if self.color != BLACK { shell.fg(self.color)?; }
        if let Some(attr) = self.attr {
            if shell.supports_attr(attr) {
                shell.attr(attr)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Styles {
    pub status: Style,
    pub warning: Style,
    pub error: Style,
    pub default: Style,
    pub blocked: Style,
}


impl Default for Styles {
    fn default() -> Self {
        Styles {
            status: Style {
                color: GREEN,
                attr: Some(Attr::Bold),
            },
            warning: Style {
                color: YELLOW,
                attr: Some(Attr::Bold),
            },
            error: Style {
                color: RED,
                attr: Some(Attr::Bold),
            },
            default: Style::default(),
            blocked: Style {
                color: CYAN,
                attr: Some(Attr::Bold),
            }
        }
    }
}

impl Styles {
    fn from_env() -> Styles {
        use std::env::var;
        let mut ret = Styles::default();
        if let Ok(config) = var("CARGO_COLORS") {
            for p in config.split(":") {
                if p.starts_with("status=") {
                    ret.status = Style::from_str(&p[7..], ret.status.color, ret.status.attr);
                } else if p.starts_with("warning=") {
                    ret.warning = Style::from_str(&p[8..], ret.warning.color, ret.warning.attr);
                } else if p.starts_with("error=") {
                    ret.error = Style::from_str(&p[6..], ret.error.color, ret.error.attr);
                } else if p.starts_with("default=") {
                    ret.default = Style::from_str(&p[8..], ret.default.color, ret.default.attr);
                } else if p.starts_with("blocked=") {
                    ret.blocked = Style::from_str(&p[8..], ret.blocked.color, ret.blocked.attr);
                }
            }
        }
        ret
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ColorConfig {
    Auto,
    Always,
    Never
}

impl fmt::Display for ColorConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ColorConfig::Auto => "auto",
            ColorConfig::Always => "always",
            ColorConfig::Never => "never",
        }.fmt(f)
    }
}

#[derive(Clone, Copy)]
pub struct ShellConfig {
    pub color_config: ColorConfig,
    pub tty: bool
}

enum AdequateTerminal {
    NoColor(Box<Write + Send>),
    Colored(Box<Terminal<Output=Box<Write + Send>> + Send>)
}

pub struct Shell {
    terminal: AdequateTerminal,
    config: ShellConfig,
}

pub struct MultiShell {
    out: Shell,
    err: Shell,
    verbosity: Verbosity,
    pub styles: Styles,
}

impl MultiShell {
    pub fn new(out: Shell, err: Shell, verbosity: Verbosity) -> MultiShell {
        MultiShell { out: out, err: err, verbosity: verbosity, styles: Styles::from_env() }
    }

    // Create a quiet, basic shell from supplied writers.
    pub fn from_write(out: Box<Write + Send>, err: Box<Write + Send>) -> MultiShell {
        let config = ShellConfig { color_config: ColorConfig::Never, tty: false };
        let out = Shell { terminal: NoColor(out), config: config.clone() };
        let err = Shell { terminal: NoColor(err), config: config };
        MultiShell {
            out: out,
            err: err,
            verbosity: Verbosity::Quiet,
            styles: Styles::from_env(),
        }
    }

    pub fn out(&mut self) -> &mut Shell {
        &mut self.out
    }

    pub fn err(&mut self) -> &mut Shell {
        &mut self.err
    }

    pub fn say<T: ToString>(&mut self, message: T, style: Style)
                            -> CargoResult<()> {
        match self.verbosity {
            Quiet => Ok(()),
            _ => self.out().say(message, style)
        }
    }

    pub fn status_with_color<T, U>(&mut self, status: T, message: U, color: Style)
                                   -> CargoResult<()>
        where T: fmt::Display, U: fmt::Display
    {
        match self.verbosity {
            Quiet => Ok(()),
            _ => self.err().say_status(status, message, color, true)
        }
    }

    pub fn status<T, U>(&mut self, status: T, message: U) -> CargoResult<()>
        where T: fmt::Display, U: fmt::Display
    {
	let color = self.styles.status;
        self.status_with_color(status, message, color)
    }

    pub fn verbose<F>(&mut self, mut callback: F) -> CargoResult<()>
        where F: FnMut(&mut MultiShell) -> CargoResult<()>
    {
        match self.verbosity {
            Verbose => callback(self),
            _ => Ok(())
        }
    }

    pub fn concise<F>(&mut self, mut callback: F) -> CargoResult<()>
        where F: FnMut(&mut MultiShell) -> CargoResult<()>
    {
        match self.verbosity {
            Verbose => Ok(()),
            _ => callback(self)
        }
    }

    pub fn error<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        let color = self.styles.error;
        self.err().say_status("error:", message, color, false)
    }

    pub fn warn<T: fmt::Display>(&mut self, message: T) -> CargoResult<()> {
        match self.verbosity {
            Quiet => Ok(()),
            _ => {
                let color = self.styles.warning;
                self.err().say_status("warning:", message, color, false)
            },
        }
    }

    pub fn set_verbosity(&mut self, verbosity: Verbosity) {
        self.verbosity = verbosity;
    }

    pub fn set_color_config(&mut self, color: Option<&str>) -> CargoResult<()> {
        let cfg = match color {
            Some("auto") => Auto,
            Some("always") => Always,
            Some("never") => Never,

            None => Auto,

            Some(arg) => bail!("argument for --color must be auto, always, or \
                                never, but found `{}`", arg),
        };
        self.out.set_color_config(cfg);
        self.err.set_color_config(cfg);
        Ok(())
    }

    pub fn get_verbose(&self) -> Verbosity {
        self.verbosity
    }

    pub fn color_config(&self) -> ColorConfig {
        assert!(self.out.config.color_config == self.err.config.color_config);
        self.out.config.color_config
    }
}

impl Shell {
    pub fn create<T: FnMut() -> Box<Write + Send>>(mut out_fn: T, config: ShellConfig) -> Shell {
        let term = match Shell::get_term(out_fn()) {
            Ok(t) => t,
            Err(_) => NoColor(out_fn())
        };

        Shell {
            terminal: term,
            config: config,
        }
    }

    #[cfg(any(windows))]
    fn get_term(out: Box<Write + Send>) -> CargoResult<AdequateTerminal> {
        // Check if the creation of a console will succeed
        if ::term::WinConsole::new(vec![0u8; 0]).is_ok() {
            let t = ::term::WinConsole::new(out)?;
            if !t.supports_color() {
                Ok(NoColor(Box::new(t)))
            } else {
                Ok(Colored(Box::new(t)))
            }
        } else {
            // If we fail to get a windows console, we try to get a `TermInfo` one
            Ok(Shell::get_terminfo_term(out))
        }
    }

    #[cfg(any(unix))]
    fn get_term(out: Box<Write + Send>) -> CargoResult<AdequateTerminal> {
        Ok(Shell::get_terminfo_term(out))
    }

    fn get_terminfo_term(out: Box<Write + Send>) -> AdequateTerminal {
        // Use `TermInfo::from_env()` and `TerminfoTerminal::supports_color()`
        // to determine if creation of a TerminfoTerminal is possible regardless
        // of the tty status. --color options are parsed after Shell creation so
        // always try to create a terminal that supports color output. Fall back
        // to a no-color terminal regardless of whether or not a tty is present
        // and if color output is not possible.
        match ::term::terminfo::TermInfo::from_env() {
            Ok(ti) => {
                let term = TerminfoTerminal::new_with_terminfo(out, ti);
                if !term.supports_color() {
                    NoColor(term.into_inner())
                } else {
                    // Color output is possible.
                    Colored(Box::new(term))
                }
            },
            Err(_) => NoColor(out),
        }
    }

    pub fn set_color_config(&mut self, color_config: ColorConfig) {
        self.config.color_config = color_config;
    }

    pub fn say<T: ToString>(&mut self, message: T, style: Style) -> CargoResult<()> {
        self.reset()?;
        style.apply(self)?;
        write!(self, "{}\n", message.to_string())?;
        self.reset()?;
        self.flush()?;
        Ok(())
    }

    pub fn say_status<T, U>(&mut self,
                            status: T,
                            message: U,
                            style: Style,
                            justified: bool)
                            -> CargoResult<()>
        where T: fmt::Display, U: fmt::Display
    {
        self.reset()?;
        style.apply(self)?;
        if justified {
            write!(self, "{:>12}", status.to_string())?;
        } else {
            write!(self, "{}", status)?;
        }
        self.reset()?;
        write!(self, " {}\n", message)?;
        self.flush()?;
        Ok(())
    }

    fn fg(&mut self, color: color::Color) -> CargoResult<bool> {
        let colored = self.colored();

        match self.terminal {
            Colored(ref mut c) if colored => c.fg(color)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn attr(&mut self, attr: Attr) -> CargoResult<bool> {
        let colored = self.colored();

        match self.terminal {
            Colored(ref mut c) if colored => c.attr(attr)?,
            _ => return Ok(false)
        }
        Ok(true)
    }

    fn supports_attr(&self, attr: Attr) -> bool {
        let colored = self.colored();

        match self.terminal {
            Colored(ref c) if colored => c.supports_attr(attr),
            _ => false
        }
    }

    fn reset(&mut self) -> term::Result<()> {
        let colored = self.colored();

        match self.terminal {
            Colored(ref mut c) if colored => c.reset()?,
            _ => ()
        }
        Ok(())
    }

    fn colored(&self) -> bool {
        self.config.tty && Auto == self.config.color_config
            || Always == self.config.color_config
    }
}

impl Write for Shell {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.terminal {
            Colored(ref mut c) => c.write(buf),
            NoColor(ref mut n) => n.write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.terminal {
            Colored(ref mut c) => c.flush(),
            NoColor(ref mut n) => n.flush()
        }
    }
}
