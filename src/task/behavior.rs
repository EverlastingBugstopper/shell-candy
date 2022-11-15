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
