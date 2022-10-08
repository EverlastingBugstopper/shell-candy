# 🍬 `shell-candy`

This crate wraps `std::process::Command`, providing an easier mechanism for handling individual log lines from external tools.

## Usage

```rust
use shell_candy::{ShellTaskLog, ShellTask};

let task = ShellTask::new("rustc --version");
task.run(|line| {
  match line {
    ShellTaskLog::Stdout(message) | ShellTaskLog::Stderr(message) => eprintln!("info: {}", &message),
  }
})
```

## More information

See [the docs](https://docs.rs/shell-candy) for more detailed information and example usage.
