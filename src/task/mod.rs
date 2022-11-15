use std::{
    collections::HashMap,
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{Error, Result, ShellTaskLog};
use crossbeam_channel::{unbounded, Receiver, Sender};

mod behavior;
mod runner;

pub use behavior::ShellTaskBehavior;
use runner::ShellTaskRunner;

/// A [`ShellTask`] runs commands and provides a passthrough log handler
/// for each log line.
#[derive(Debug)]
pub struct ShellTask {
    bin: String,
    args: Vec<String>,
    current_dir: PathBuf,
    envs: HashMap<OsString, OsString>,
    full_command: String,
    log_sender: Sender<ShellTaskLog>,
    log_receiver: Receiver<ShellTaskLog>,
}

impl ShellTask {
    /// Create a new [`ShellTask`] with a log line handler.
    pub fn new(command: &str) -> Result<Self> {
        let current_dir =
            env::current_dir().map_err(|source| Error::CouldNotFindCurrentDirectory { source })?;
        let command = command.to_string();
        let args: Vec<&str> = command.split(' ').collect();
        let (bin, args) = match args.len() {
            0 => Err(Error::InvalidTask {
                task: command.to_string(),
                reason: "an empty string is not a command".to_string(),
            }),
            1 => Ok((args[0], Vec::new())),
            _ => Ok((args[0], Vec::from_iter(args[1..].iter()))),
        }?;

        if which::which(bin).is_err() {
            Err(Error::InvalidTask {
                task: command.to_string(),
                reason: format!("'{}' is not installed on this machine", &bin),
            })
        } else {
            let (log_sender, log_receiver) = unbounded();
            Ok(Self {
                bin: bin.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
                full_command: command,
                envs: HashMap::new(),
                current_dir,
                log_sender,
                log_receiver,
            })
        }
    }

    /// Adds an environment variable to the command run by [`ShellTask`].
    pub fn env<K, V>(&mut self, key: K, value: V) -> &mut ShellTask
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.envs
            .insert(key.as_ref().to_os_string(), value.as_ref().to_os_string());
        self
    }

    /// Sets the directory the command should be run in.
    pub fn current_dir<P>(&mut self, path: P)
    where
        P: AsRef<Path>,
    {
        self.current_dir = path.as_ref().to_path_buf();
    }

    /// Returns the full command that was used to instantiate this [`ShellTask`].
    pub fn descriptor(&self) -> String {
        self.full_command.to_string()
    }

    /// Returns the [`ShellTask::descriptor`] with the classic `$` shell prefix.
    pub fn bash_descriptor(&self) -> String {
        format!("$ {}", self.descriptor())
    }

    /// Returns the [`ShellTaskRunner`] from the internal configuration.
    fn get_command(&self) -> Command {
        let mut command = Command::new(&self.bin);
        command
            .args(&self.args)
            .envs(&self.envs)
            .current_dir(&self.current_dir);
        command
    }

    /// Run a [`ShellTask`], applying the log handler to each line.
    ///
    /// You can make the task terminate early if your `log_handler`
    /// returns [`ShellTaskBehavior::EarlyReturn<T>`]. When this variant
    /// is returned from a log handler, [`ShellTask::run`] will return [`Some<T>`].
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::anyhow;
    /// use shell_candy::{ShellTask, ShellTaskLog, ShellTaskBehavior};
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    ///     let result: Option<String> = ShellTask::new("rustc --version")?.run(|line| {
    ///         match line {
    ///             ShellTaskLog::Stderr(_) => {
    ///                 ShellTaskBehavior::Passthrough
    ///             },
    ///             ShellTaskLog::Stdout(message) => {
    ///                 eprintln!("{}", &message);
    ///                 ShellTaskBehavior::EarlyReturn(Ok(message))
    ///             }
    ///         }
    ///     })?;
    ///     assert!(result.is_some());
    ///     Ok(())
    /// }
    /// ```
    ///
    /// If your `log_handler` returns [`ShellTaskBehavior::Passthrough`] for
    /// the entire lifecycle of the task, [`ShellTask::run`] will return [`None`].
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::anyhow;
    /// use shell_candy::{ShellTask, ShellTaskLog, ShellTaskBehavior};
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    ///     let result: Option<()> = ShellTask::new("rustc --version")?.run(|line| {
    ///         match line {
    ///             ShellTaskLog::Stderr(message) | ShellTaskLog::Stdout(message) => {
    ///                 eprintln!("info: {}", &message);
    ///                 ShellTaskBehavior::Passthrough
    ///             }
    ///         }
    ///     })?;
    ///     assert!(result.is_none());
    ///     Ok(())
    /// }
    /// ```
    pub fn run<F, T>(&self, log_handler: F) -> Result<Option<T>>
    where
        F: Fn(ShellTaskLog) -> ShellTaskBehavior<T> + Send + Sync + 'static,
        T: Send + Sync + 'static,
    {
        let log_counter: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
        let log_decrementer = log_counter.clone();
        let log_incrementer = log_counter.clone();
        let log_receiver = self.log_receiver.clone();
        let full_command = self.full_command.to_string();

        let maybe_result = Arc::new(Mutex::new(None));
        let early_terminator = maybe_result.clone();
        rayon::spawn(move || {
            while let Ok(line) = log_receiver.recv() {
                if let Ok(mut log_decrementer) = log_decrementer.clone().lock() {
                    if let Some(count) = log_decrementer.as_mut() {
                        *count -= 1;
                    }
                    match (log_handler)(line) {
                        ShellTaskBehavior::EarlyReturn(early_return) => {
                            if let Ok(mut maybe_result) = early_terminator.lock() {
                                if maybe_result.is_none() {
                                    *maybe_result = Some(early_return);
                                    break;
                                }
                            }
                        }
                        ShellTaskBehavior::Passthrough => continue,
                    }
                } else if let Ok(mut maybe_result) = early_terminator.lock() {
                    if maybe_result.is_none() {
                        *maybe_result =
                            Some(Err(Box::new(Error::PoisonedLog { task: full_command })));
                        break;
                    }
                } else {
                    continue;
                }
            }
        });

        let mut task = ShellTaskRunner::run(
            self.get_command(),
            self.full_command.to_string(),
            self.log_sender.clone(),
            log_incrementer,
        )?;

        let exit_status = task.child.wait().map_err(|source| Error::CouldNotWait {
            task: self.full_command.to_string(),
            source,
        })?;

        // wait until the log counter reaches 0 so we know they've all been processed
        loop {
            std::thread::sleep(Duration::from_millis(200));
            match log_counter.try_lock().map(|l| *l) {
                Ok(Some(0)) => break,
                _ => continue,
            }
        }

        if exit_status.success() {
            if let Some(result) = maybe_result.clone().lock().unwrap().take() {
                result.map(|t| Some(t)).map_err(|e| e.into())
            } else {
                Ok(None)
            }
        } else {
            Err(Error::TaskFailure {
                task: self.full_command.to_string(),
                exit_status,
            })
        }
    }
}
