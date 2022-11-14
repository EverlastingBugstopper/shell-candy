#[cfg(doc)]
use crate::ShellTask;

/// A log message emitted by a [`ShellTask`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShellTaskLog {
    /// A log message emitted to `stdout`
    Stdout(String),

    /// A log message emitted to `stderr`
    Stderr(String),
}
