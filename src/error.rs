#[cfg(doc)]
use crate::ShellTask;

use thiserror::Error as ThisError;

use std::{io, process::ExitStatus};

/// The result type used by a [`ShellTask`].
pub type Result<T> = std::result::Result<T, Error>;

/// The possible errors reported by a [`ShellTask`].
#[derive(ThisError, Debug)]
pub enum Error {
    #[error("'{command}' failed with {exit_status}.")]
    CommandFailure {
        command: String,
        exit_status: ExitStatus,
    },

    #[error("'{command}' could not run because {reason}.")]
    InvalidCommand { command: String, reason: String },

    #[error("could not spawn '{command}': {source}.")]
    CouldNotSpawn { command: String, source: io::Error },

    #[error("could not wait for '{command}' to complete: {source}.")]
    CouldNotWait { command: String, source: io::Error },
}
