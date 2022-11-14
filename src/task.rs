use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{Error, Result, ShellTaskLog};
use crossbeam_channel::{unbounded, Receiver, Sender};

/// A [`ShellTask`] runs commands and provides a passthrough log handler
/// for each log line.
#[derive(Debug)]
pub struct ShellTask {
    bin: String,
    args: Vec<String>,
    full_command: String,
    log_sender: Sender<ShellTaskLog>,
    log_receiver: Receiver<ShellTaskLog>,
}

/// The type of error that can be returned by log handlers when running tasks.
type UserDefinedError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// The result that can be returned by log handlers when running tasks.
type UserDefinedResult<T> = std::result::Result<T, UserDefinedError>;

/// [`ShellTaskBehavior`] allows you to terminate a process
/// early, or to continue inside your log handler.
#[derive(Debug)]
pub enum ShellTaskBehavior<T> {
    /// When a log handler returns this variant after processing a log line,
    /// the underlying process is terminated and the underlying [`Result`] is returned.
    EarlyReturn(UserDefinedResult<T>),

    /// When a log handler returns this variant after processing a log line,
    /// the process is allowed to continue.
    Passthrough,
}

impl ShellTask {
    /// Create a new [`ShellTask`] with a log line handler.
    pub fn new(command: &str) -> Result<Self> {
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
                log_sender,
                log_receiver,
            })
        }
    }

    /// Returns the full command that was used to instantiate this [`ShellTask`].
    pub fn descriptor(&self) -> String {
        self.full_command.to_string()
    }

    /// Returns the [`ShellTask::descriptor`] with the classic `$` shell prefix.
    pub fn bash_descriptor(&self) -> String {
        format!("$ {}", self.descriptor())
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

        let maybe_result = Arc::new(Mutex::new(None));
        let early_terminator = maybe_result.clone();
        rayon::spawn(move || loop {
            match log_receiver.recv() {
                Ok(line) => {
                    if let Some(count) = log_decrementer.clone().lock().unwrap().as_mut() {
                        *count -= 1;
                    }
                    match (log_handler)(line) {
                        ShellTaskBehavior::EarlyReturn(early_return) => {
                            let mut maybe_result = early_terminator.lock().unwrap();
                            if maybe_result.is_none() {
                                *maybe_result = Some(early_return);
                                break;
                            }
                        }
                        ShellTaskBehavior::Passthrough => continue,
                    }
                }
                Err(_) => break,
            }
        });

        let mut task = ShellTaskRunner::run(
            &self.bin,
            &self.args,
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

/// Runs a [`ShellTask`] in the background, reporting all logs and errors
#[derive(Debug)]
struct ShellTaskRunner {
    child: Child,
}

impl ShellTaskRunner {
    fn run(
        bin: &str,
        args: &Vec<String>,
        full_command: String,
        log_sender: Sender<ShellTaskLog>,
        log_incrementer: Arc<Mutex<Option<usize>>>,
    ) -> Result<Self> {
        let mut command = Command::new(bin);
        command.args(args).env("SHELL_CANDY", "true");

        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|source| Error::CouldNotSpawn {
            task: full_command,
            source,
        })?;

        let stdout_incrementer = log_incrementer.clone();
        let stderr_incrementer = log_incrementer.clone();

        if let Some(stdout) = child.stdout.take() {
            let log_sender = log_sender.clone();
            rayon::spawn(move || {
                let stdout = BufReader::new(stdout);
                stdout.lines().for_each(|line| {
                    if let Ok(line) = line {
                        let guard = stdout_incrementer.clone();

                        match guard.lock() {
                            Ok(mut guard) => match guard.as_mut() {
                                Some(s) => {
                                    *s += 1;
                                }
                                None => {
                                    *guard = Some(1);
                                }
                            },
                            Err(e) => panic!("{}", e),
                        }

                        log_sender
                            .send(ShellTaskLog::Stdout(line))
                            .expect("could not update stdout logs for command");
                    }
                });
            });
        }

        if let Some(stderr) = child.stderr.take() {
            rayon::spawn(move || {
                let stderr = BufReader::new(stderr);
                stderr.lines().for_each(|line| {
                    if let Ok(line) = line {
                        let guard = stderr_incrementer.clone();

                        match guard.lock() {
                            Ok(mut guard) => match guard.as_mut() {
                                Some(s) => {
                                    *s += 1;
                                }
                                None => {
                                    *guard = Some(1);
                                }
                            },
                            Err(e) => panic!("{}", e),
                        }

                        log_sender
                            .send(ShellTaskLog::Stderr(line))
                            .expect("could not update stderr logs for command");
                    }
                });
            });
        }

        Ok(Self { child })
    }
}
