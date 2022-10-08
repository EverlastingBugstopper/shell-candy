#[cfg(doc)]
use crate::ShellTask;

use thiserror::Error as ThisError;

use std::{io, process::ExitStatus};

/// The result type used by a [`ShellTask`].
pub type Result<T> = std::result::Result<T, Error>;

/// The possible errors reported by a [`ShellTask`].
#[derive(ThisError, Debug)]
pub enum Error {
    /// This error occurs when a command exits with a status other than 0.
    #[error("'{task}' failed with {exit_status}.")]
    TaskFailure {
        /// The task that failed.
        task: String,

        /// The exit status that was returned.
        exit_status: ExitStatus,
    },

    /// This error occurs when a task could not be instantiated because it was malformed.
    /// This is a usage error, make sure you've typed the command correctly.
    #[error("'{task}' is not a valid command because {reason}.")]
    InvalidTask {
        /// The malformed task.
        task: String,

        /// The reason the task was malformed.
        reason: String,
    },

    /// This error occurs when a task could not spawn. Originates from [`std::process::Command::spawn`].
    #[error("could not spawn '{task}': {source}.")]
    CouldNotSpawn {
        /// The task that could not spawn.
        task: String,

        /// The [`io::Error`] that was reported by [`std::process::Command::spawn`].
        source: io::Error,
    },

    /// There was an error waiting for the task status. Originates from [`std::process::Child::wait`].
    #[error("could not wait for '{task}' to complete: {source}.")]
    CouldNotWait {
        /// The task that could not be waited for.
        task: String,

        /// /The [`io::Error`] that was reported by [`std::process::Child::wait`].
        source: io::Error,
    },

    /// This error can be returned from log handlers to terminate early.
    #[error(transparent)]
    EarlyReturn(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}
