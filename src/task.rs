use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::{Error, FnTaskLogHandler, Result, ShellTaskLog};

/// A [`ShellTask`] runs commands and provides a passthrough log handler
/// for each log line.
/// # Examples
///
/// ```
/// use shell_candy::{Result, ShellTaskLog, ShellTask};
///
/// fn main() -> Result<()> {
///   // Create a task to check the current version of `rustc` that is installed
///   let task = ShellTask::new("rustc --version", |line| {
///     match line {
///       // print all log lines with an "info: " prefix
///       ShellTaskLog::Stderr(message) | ShellTaskLog::Stdout(message) => eprintln!("info: {}", &message),
///     }
///   })?;
///
///   // Run the task
///   task.run()?;
///
///   Ok(())
/// }
/// ```
pub struct ShellTask {
    bin: String,
    args: Vec<String>,
    full_command: String,
    log_sender: Sender<ShellTaskLog>,
    log_receiver: Receiver<ShellTaskLog>,
    log_handler: Arc<FnTaskLogHandler>,
}

impl ShellTask {
    /// Create a new [`ShellTask`] with a log line handler.
    pub fn new<F>(command: &str, log_handler: F) -> Result<Self>
    where
        F: Fn(ShellTaskLog) + Send + Sync + 'static,
    {
        let command = command.to_string();
        let args: Vec<&str> = command.split(' ').collect();
        let (bin, args) = match args.len() {
            0 => Err(Error::InvalidCommand {
                command: command.to_string(),
                reason: "it is not a valid command".to_string(),
            }),
            1 => Ok((args[0], Vec::new())),
            _ => Ok((args[0], Vec::from_iter(args[1..].iter()))),
        }?;

        if which::which(bin).is_err() {
            Err(Error::InvalidCommand {
                command: command.to_string(),
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
                log_handler: Arc::new(Box::new(log_handler)),
            })
        }
    }

    /// Run a [`ShellTask`], applying the log handler to each line.
    pub fn run(&self) -> Result<()> {
        let log_counter: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));

        let log_decrementer = log_counter.clone();
        let log_incrementer = log_counter.clone();
        let log_handler = self.log_handler.clone();
        let log_receiver = self.log_receiver.clone();

        rayon::spawn(move || loop {
            match log_receiver.recv() {
                Ok(line) => {
                    (log_handler)(line);
                    if let Some(count) = log_decrementer.clone().lock().unwrap().as_mut() {
                        *count -= 1;
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
            command: self.full_command.to_string(),
            source,
        })?;

        loop {
            std::thread::sleep(Duration::from_millis(200));
            match log_counter.try_lock().map(|l| *l) {
                Ok(Some(0)) => break,
                _ => continue,
            }
        }

        if exit_status.success() {
            Ok(())
        } else {
            Err(Error::CommandFailure {
                command: self.full_command.to_string(),
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
            command: full_command,
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
