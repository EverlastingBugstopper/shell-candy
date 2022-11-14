# 🍬 `shell-candy`

This crate wraps `std::process::Command`, providing an easier mechanism for handling individual log lines from external tools.

## Usage

This example shows the basic usage of `ShellTask`: create one from a POSIX-style command, and then run it with a log line handler that you write yourself. This handler can either continue for every line for the length of the program, or it can return early and shut down the program.

You could use this function to pass log lines through your own log formatter like so:

```rust
use anyhow::Result;
use shell_candy::{ShellTaskLog, ShellTaskBehavior, ShellTask};

fn main() -> Result<()> {
  let task = ShellTask::new("rustc --version")?;
  task.run(|line| {
    match line {
      ShellTaskLog::Stdout(message) | ShellTaskLog::Stderr(message) => eprintln!("info: {}", &message),
    }
    ShellTaskBehavior::<()>::Passthrough
  })?;
  Ok(())
}
```

You could also use this function to return early if a command meets a specific criteria (like encountering an unrecoverable error):

```rust
use anyhow::{anyhow, Error, Result};
use shell_candy::{ShellTaskLog, ShellTaskBehavior, ShellTask};

fn main() -> Result<()> {
  let task = ShellTask::new("git log")?;
  task.run(|line| {
    match line {
      ShellTaskLog::Stdout(message) | ShellTaskLog::Stderr(message) => {
        if message.contains("an error that is unlikely to be in your git logs but just might be") {
          return ShellTaskBehavior::<()>::EarlyReturn(Err(anyhow!("encountered an error while running 'git log'").into()));
        }
      },
    }
    ShellTaskBehavior::<()>::Passthrough
  })?;
  Ok(())
}
```

## More information

See [the docs](https://docs.rs/shell-candy) for more detailed information and example usage.
