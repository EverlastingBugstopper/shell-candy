use shell_candy::{ShellTask, ShellTaskBehavior, ShellTaskLog};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let task = ShellTask::new("rustc --version")?;
    // print $ rustc --version to the terminal
    eprintln!("{}", task.bash_descriptor());
    let _: Option<()> = task.run(|line| match line {
        ShellTaskLog::Stderr(message) | ShellTaskLog::Stdout(message) => {
            eprintln!("{}", &message);
            ShellTaskBehavior::Passthrough
        }
    })?;
    Ok(())
}
