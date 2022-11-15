use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

use crossbeam_channel::Sender;

use crate::{task::ShellTaskLog, Error, Result};

/// Runs a [`ShellTask`] in the background, reporting all logs and errors
#[derive(Debug)]
pub(crate) struct ShellTaskRunner {
    pub(crate) child: Child,
}

impl ShellTaskRunner {
    pub(crate) fn run(
        command: Command,
        command_string: String,
        log_sender: Sender<ShellTaskLog>,
        log_incrementer: Arc<Mutex<Vec<ShellTaskLog>>>,
    ) -> Result<Self> {
        let mut command = command;
        command.env("SHELL_CANDY", "true");
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|source| Error::CouldNotSpawn {
            task: command_string,
            source,
        })?;

        let stdout_incrementer = log_incrementer.clone();
        let stderr_incrementer = log_incrementer;

        if let Some(stdout) = child.stdout.take() {
            let log_sender = log_sender.clone();
            rayon::spawn(move || {
                let stdout = BufReader::new(stdout);
                stdout.lines().for_each(|line| {
                    if let Ok(line) = line {
                        let guard = stdout_incrementer.clone();

                        match guard.lock() {
                            Ok(mut guard) => guard.push(ShellTaskLog::Stdout(line.to_string())),
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
                            Ok(mut guard) => guard.push(ShellTaskLog::Stderr(line.to_string())),
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
