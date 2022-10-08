//! This library provides [`ShellTask`], a type that wraps [`std::process::Command`]
//! to allow for functional control over lines printed to `stderr` and `stdout` by POSIX style tools.
//!
//! # Examples
//!
//! ```
//! use shell_candy::{ShellTask, ShellTaskLog, ShellTaskBehavior};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!   let task = ShellTask::new("rustc --version")?;
//!
//!   let _: Option<()> = task.run(|line| {
//!    match line {
//!       ShellTaskLog::Stderr(message) | ShellTaskLog::Stdout(message) => {
//!         eprintln!("{}", &message);
//!         ShellTaskBehavior::Passthrough
//!       },
//!    }
//!   })?;
//!   Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

mod error;
mod log;
mod task;

pub use error::*;
pub use log::*;
pub use task::*;
