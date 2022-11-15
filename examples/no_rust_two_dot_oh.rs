use anyhow::{anyhow, Context, Result};
use semver::Version;
use shell_candy::{ShellTask, ShellTaskBehavior, ShellTaskLog, ShellTaskOutput};

fn main() -> Result<()> {
    // Create a task to check the current version of `rustc` that is installed
    let task = ShellTask::new("rustc --version")?;

    // print $ rustc --version to the terminal
    eprintln!("{}", task.bash_descriptor());

    // Run the command with a handler closure that swallows all output
    let task_result = task.run(|line| ShellTaskBehavior::<()>::Passthrough)?;

    let rustc_version = match task_result {
        ShellTaskOutput::CompleteOutput { stdout_lines, .. }
        | ShellTaskOutput::EarlyReturn { stdout_lines, .. } => {
            let num_stdout_lines = stdout_lines.len();
            if num_stdout_lines != 1 {
                Err(anyhow!("`rustc --version` output was malformed: printed {} lines to stdout instead of the expected 1", num_stdout_lines))
            } else {
                // output looks like this:
                // $ rustc --version
                // rustc 1.63.0 (4b91a6ea7 2022-08-08)
                let rustc_version = &stdout_lines[0];
                let rustc_version_parts: Vec<String> =
                    rustc_version.split(' ').map(|s| s.to_string()).collect();
                if let [bin, version, _, _] = &rustc_version_parts[..] {
                    match (bin.as_str(), &version) {
                        ("rustc", version) => Ok(Version::parse(version)
                            .with_context(|| format!("'{version}' is not a valid version"))?),
                        _ => Err(anyhow!(
                            "'{version}' does not appear to be output from rustc"
                        )),
                    }
                } else {
                    Err(anyhow!("`rustc --version` output was malformed: expected 4 words separated by spaces, got '{rustc_version}'."))
                }
            }
        }
    }?;

    // i don't think there will be a rust 2
    // but let's check
    if rustc_version.major == 1 {
        eprintln!("🦀 rustc v{}", &rustc_version);
        Ok(())
    } else {
        Err(anyhow!(
            "invalid `rustc --version` output: v{rustc_version}. the major version must be 1"
        ))
    }
}
