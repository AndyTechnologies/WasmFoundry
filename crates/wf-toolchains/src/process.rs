//! Running external programs.
//!
//! Introduced in PHASE 6, when the first toolchain starts a process. The seam exists for
//! one reason: a toolchain's *command line* is its contract, and without a seam the only
//! way to check what it builds is to run the toolchain and hope. With one, the exact
//! arguments are asserted without invoking anything.

use std::path::PathBuf;
use std::process::Command;

/// What to run.
///
/// Data rather than a constructed `Command`, so a test can read the intent: the program
/// name, the argument list and the working directory are visible in a value that no
/// process has touched yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    program: String,
    args: Vec<String>,
    cwd: Option<PathBuf>,
}

impl CommandSpec {
    /// Builds a specification for `program`.
    pub fn new(program: impl Into<String>) -> Self {
        CommandSpec {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
        }
    }

    /// Appends several arguments in order.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Sets the directory the program runs in.
    pub fn current_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    /// The program to run.
    pub fn program(&self) -> &str {
        &self.program
    }

    /// The arguments, in order, as the program will receive them.
    ///
    /// Named `arguments` because `args` is taken by the builder: Rust has no
    /// overloading, and one of the two would have to be a different operation under a
    /// different name anyway.
    pub fn arguments(&self) -> &[String] {
        &self.args
    }

    /// The working directory, when one was set.
    pub fn cwd(&self) -> Option<&std::path::Path> {
        self.cwd.as_deref()
    }

    /// The full command line, rendered the way a human would retype it.
    ///
    /// Only for messages: it quotes nothing that would survive a shell, and it is never
    /// used to execute anything.
    pub fn display(&self) -> String {
        let mut line = self.program().to_owned();
        for arg in self.arguments() {
            line.push(' ');
            line.push_str(arg);
        }
        if let Some(cwd) = &self.cwd {
            line = format!("(in {}) {line}", cwd.display());
        }
        line
    }
}

/// What happened when a program ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

impl CommandOutput {
    /// Builds an outcome.
    ///
    /// Test-only: a double must be able to describe a program's behaviour without
    /// starting it, because a toolchain's command line has to be checkable without a
    /// compiler present.
    #[cfg(test)]
    pub fn new(status: Option<i32>, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        CommandOutput {
            status,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    /// The exit code, or `None` when the program was terminated by a signal or could not
    /// start at all.
    ///
    /// Reported alongside a failure because a non-zero code and a missing binary are
    /// different problems.
    pub fn status(&self) -> Option<i32> {
        self.status
    }

    /// Standard output, lossily decoded.
    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    /// Standard error, lossily decoded.
    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    /// Whether the program reported success.
    pub fn success(&self) -> bool {
        self.status == Some(0)
    }
}

/// Why a program could not be run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessError {
    /// The program could not be started at all.
    FailedToStart {
        /// The program name that was attempted.
        program: String,
        /// What the OS reported.
        message: String,
    },
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::FailedToStart { program, message } => {
                write!(f, "cannot start `{program}`: {message}")
            }
        }
    }
}

impl std::error::Error for ProcessError {}

/// Runs external programs.
///
/// One method, and no process hierarchy: a toolchain needs to start a program, wait for
/// it and read what it said. Everything else — environment, redirection, timeouts —
/// arrives when a toolchain actually requires it.
pub trait ProcessRunner {
    /// Runs `spec` to completion.
    fn run(&self, spec: &CommandSpec) -> Result<CommandOutput, ProcessError>;
}

/// The real thing: start the process and wait.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdRunner;

impl ProcessRunner for StdRunner {
    fn run(&self, spec: &CommandSpec) -> Result<CommandOutput, ProcessError> {
        let mut command = Command::new(&spec.program);
        command.args(&spec.args);
        if let Some(cwd) = spec.cwd() {
            command.current_dir(cwd);
        }

        let output = command
            .output()
            .map_err(|error| ProcessError::FailedToStart {
                program: spec.program.clone(),
                message: error.to_string(),
            })?;

        Ok(CommandOutput {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The specification must stay readable, because asserting on it is the whole point
    /// of having one.
    #[test]
    fn a_spec_keeps_program_arguments_and_directory_in_order() {
        let spec = CommandSpec::new("cargo")
            .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
            .current_dir("/tmp/guest");

        assert_eq!(spec.program(), "cargo");
        assert_eq!(
            spec.arguments(),
            ["build", "--release", "--target", "wasm32-unknown-unknown"]
        );
        assert_eq!(spec.cwd(), Some(std::path::Path::new("/tmp/guest")));
    }

    #[test]
    fn a_spec_without_a_directory_has_none() {
        let spec = CommandSpec::new("cargo");
        assert!(spec.cwd().is_none());
    }

    #[test]
    fn the_display_is_readable_so_a_failing_command_can_be_reported() {
        let spec = CommandSpec::new("cargo").args(["build", "--release"]);
        assert_eq!(spec.display(), "cargo build --release");

        let with_dir = spec.current_dir("/tmp/guest");
        assert!(
            with_dir.display().contains("/tmp/guest"),
            "{}",
            with_dir.display()
        );
    }

    /// A missing program is a distinct failure from a program that ran and failed.
    #[test]
    fn a_program_that_does_not_exist_cannot_be_started() {
        let spec = CommandSpec::new("wf-program-that-cannot-exist-xyz").args(["--flag"]);
        let error = StdRunner.run(&spec).expect_err("it cannot start");
        match &error {
            ProcessError::FailedToStart { program, message } => {
                assert!(program.contains("wf-program-that-cannot-exist"));
                assert!(!message.is_empty());
            }
        }
        assert!(error.to_string().contains("cannot start"));
    }

    /// A real run reports its exit code and separates success from failure.
    #[test]
    fn a_program_that_runs_reports_its_exit_code() {
        let spec = CommandSpec::new("sh").args(["-c", "echo out; echo err >&2; exit 3"]);
        let output = StdRunner.run(&spec).expect("sh exists");

        assert!(!output.success());
        assert_eq!(output.status(), Some(3));
        assert!(output.stdout().contains("out"), "{}", output.stdout());
        assert!(output.stderr().contains("err"), "{}", output.stderr());
    }

    #[test]
    fn a_successful_program_is_reported_as_successful() {
        let output = StdRunner
            .run(&CommandSpec::new("sh").args(["-c", "exit 0"]))
            .expect("sh exists");
        assert!(output.success());
        assert_eq!(output.status(), Some(0));
    }

    /// A directory that does not exist must fail rather than silently run somewhere else.
    #[test]
    fn a_missing_working_directory_is_reported_as_a_failure_to_start() {
        let spec = CommandSpec::new("sh")
            .args(["-c", "exit 0"])
            .current_dir("/no/such/dir");
        let error = StdRunner
            .run(&spec)
            .expect_err("the directory is not there");
        assert!(error.to_string().contains("cannot start"), "{error}");
    }
}
