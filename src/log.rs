#[cfg(doc)]
use crate::ShellTask;

/// A log message emitted by a [`ShellTask`].
#[derive(Debug)]
pub enum ShellTaskLog {
    /// A log message emitted to `stdout`
    Stdout(String),

    /// A log message emitted to `stderr`
    Stderr(String),
}

/// A function to handle [`ShellTaskLog`]
pub(crate) type FnTaskLogHandler = Box<dyn Fn(ShellTaskLog) + Send + Sync + 'static>;
