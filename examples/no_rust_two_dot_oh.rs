use anyhow::{anyhow, Context, Result};
use semver::Version;
use shell_candy::{ShellTask, ShellTaskBehavior, ShellTaskLog};

fn main() -> Result<()> {
    // Create a task to check the current version of `rustc` that is installed
    let task = ShellTask::new("rustc --version")?;

    // print $ rustc --version to the terminal
    eprintln!("{}", task.bash_descriptor());

    // Run the command with a handler closure that redirects the output to `stderr`
    let rustc_version = task.run(|line| {
    if let ShellTaskLog::Stdout(rustc_version) = line {
      // output looks like this:
      // $ rustc --version
      // rustc 1.63.0 (4b91a6ea7 2022-08-08)
      let rustc_version_parts: Vec<String> = rustc_version.split(' ').map(|s| s.to_string()).collect();
      let result: std::result::Result<Version, anyhow::Error> = if let [bin, version, _, _] = &rustc_version_parts[..] {
          match (bin.as_str(), &version) {
            ("rustc", version) => {
              Version::parse(version).with_context(|| format!("'{version}' is not a valid version"))
            },
            _ => Err(anyhow!("'{version}' does not appear to be output from rustc"))
          }
      } else {
        Err(anyhow!("`rustc --version` output was malformed: expected 4 words separated by spaces, got '{rustc_version}'."))
      };

      ShellTaskBehavior::EarlyReturn(result.map_err(|e| Box::from(e)))
    } else {
      ShellTaskBehavior::Passthrough
    }
  })?;

    if let Some(rustc_version) = rustc_version {
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
    } else {
        // this should be unreachable, since we returned something from our handler
        Err(anyhow!(
            "could not process the output from `rustc --version`"
        ))
    }
}
