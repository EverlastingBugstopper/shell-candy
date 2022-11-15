use std::process::ExitStatus;

/// ShellTaskOutput is returned by `ShellTask::run` and contains information about the task on completion.
#[derive(Debug)]
pub enum ShellTaskOutput<T> {
    /// This variant is returned when the specified log handler returns early or runs into an unrecoverable error.
    EarlyReturn {
        /// The lines printed to `stdout` by the task up until the point of early return.
        stdout_lines: Vec<String>,

        /// The lines printed to `stderr` by the task up until the point of early return.
        stderr_lines: Vec<String>,

        /// The early return value.
        return_value: T,
    },

    /// This variant is returned when the specified log handler did not return early.
    CompleteOutput {
        /// The exit status of the task.
        status: ExitStatus,

        /// The lines printed to `stdout` by the task.
        stdout_lines: Vec<String>,

        /// The lines printed to `stderr` by the task.
        stderr_lines: Vec<String>,
    },
}
