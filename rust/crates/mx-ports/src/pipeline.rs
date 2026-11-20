use crate::proc::{ProcError, ProcExit};
use crate::{Effect, EffectLog, Script, lock};
use mx_core::NonEmpty;
use std::fmt;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

/// Where one stage of a pipeline reads its standard input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// An empty input, like `< /dev/null`.
    Empty,
    /// The bytes of a file, like `< file`. The file is opened by the adapter,
    /// so a large prompt never passes through the caller's memory or argv.
    File(PathBuf),
    /// The standard output of the stage before this one, like the right side of
    /// a `|`. Valid only for a stage that is not first.
    PreviousStage,
}

/// Where one stage writes a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sink {
    /// Discarded, like `> /dev/null`.
    Null,
    /// Appended to a file, like `>> file`. The file is created when missing.
    /// The adapter opens it with `O_APPEND`, so two writers interleave whole
    /// writes the way the shell's `>>` does.
    AppendFile(PathBuf),
    /// The standard input of the next stage, like the left side of a `|`.
    /// Valid only for standard output, and only for a stage that is not last.
    NextStage,
    /// The same destination as standard output, like `2>&1`. Valid only for
    /// standard error.
    SameAsStdout,
}

/// One program of a pipeline.
///
/// The argument vector is never a shell string. `env` is the whole environment
/// of the program besides `PATH` and `LC_ALL`, which belong to the adapter: the
/// adapter clears the environment first, so the caller decides, name by name,
/// what the child sees. A secret goes here and never in `argv`.
#[derive(Clone, PartialEq, Eq)]
pub struct StageSpec {
    pub argv: NonEmpty<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
    pub stdin: Source,
    pub stdout: Sink,
    pub stderr: Sink,
}

impl StageSpec {
    /// A stage with an empty input and both streams discarded.
    #[must_use]
    pub fn new(argv: NonEmpty<String>) -> Self {
        Self {
            argv,
            env: Vec::new(),
            cwd: None,
            stdin: Source::Empty,
            stdout: Sink::Null,
            stderr: Sink::Null,
        }
    }
}

impl fmt::Debug for StageSpec {
    /// Prints the names of the variables and never their values.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.env.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("StageSpec")
            .field("argv", &self.argv)
            .field("env_names", &names)
            .field("cwd", &self.cwd)
            .field("stdin", &self.stdin)
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .finish()
    }
}

/// A program, or programs joined by pipes, run to the end with its streams
/// connected to files. This is the shape of `backend_run` in the bash: the
/// Claude backend is two stages (`claude ... | python3 transcript.py`) and the
/// other three are one stage with `>>"$log" 2>&1`.
///
/// Every stage starts in its own process group and the whole pipeline runs
/// under one deadline: when `timeout` passes, every group is killed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineSpec {
    pub stages: NonEmpty<StageSpec>,
    pub timeout: Duration,
}

/// How each stage of a finished pipeline ended, in stage order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineOutcome {
    pub exits: Vec<ProcExit>,
}

impl PipelineOutcome {
    /// The exit status of the pipeline under `set -o pipefail`: the status of
    /// the rightmost stage that did not exit 0, or 0 when every stage did.
    /// A stage that timed out is never a success.
    #[must_use]
    pub fn pipefail(&self) -> ProcExit {
        self.exits.iter().rev().find(|exit| !exit.success()).copied().unwrap_or(ProcExit::Code(0))
    }
}

/// Runs a [`PipelineSpec`].
///
/// A separate port from [`Proc`](crate::Proc) because the two answer different
/// questions: `Proc` captures a short program's output for the caller to read,
/// and `Pipe` lets a long program write to files while the caller waits.
pub trait Pipe {
    /// Runs every stage and waits for all of them, or for the timeout.
    fn run_pipeline(&self, spec: &PipelineSpec) -> Result<PipelineOutcome, ProcError>;
}

/// One call to a [`Pipe`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipeEffect {
    Run(PipelineSpec),
}

/// A scripted [`Pipe`]: answers by call order and records its calls. It writes
/// nothing to any file.
#[derive(Debug)]
pub struct FakePipe {
    log: EffectLog,
    answers: Mutex<Script<Result<PipelineOutcome, ProcError>>>,
}

impl FakePipe {
    /// A fake that answers no call.
    #[must_use]
    pub fn new(log: &EffectLog) -> Self {
        Self { log: log.clone(), answers: Mutex::new(Script::none()) }
    }

    /// Sets the answers for the calls that follow.
    pub fn script(&self, answers: Script<Result<PipelineOutcome, ProcError>>) {
        *lock(&self.answers) = answers;
    }

    /// An outcome where every stage exited with the given codes.
    #[must_use]
    pub fn exited(codes: &[i32]) -> PipelineOutcome {
        PipelineOutcome { exits: codes.iter().map(|code| ProcExit::Code(*code)).collect() }
    }
}

impl Pipe for FakePipe {
    fn run_pipeline(&self, spec: &PipelineSpec) -> Result<PipelineOutcome, ProcError> {
        self.log.push(Effect::Pipe(PipeEffect::Run(spec.clone())));
        lock(&self.answers).next(&format!("Pipe::run_pipeline {:?}", spec.stages.first().argv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(codes: &[i32]) -> PipelineOutcome {
        FakePipe::exited(codes)
    }

    #[test]
    fn pipefail_is_the_rightmost_stage_that_failed() {
        assert_eq!(outcome(&[0, 0]).pipefail(), ProcExit::Code(0));
        assert_eq!(outcome(&[3, 0]).pipefail(), ProcExit::Code(3));
        assert_eq!(outcome(&[0, 4]).pipefail(), ProcExit::Code(4));
        assert_eq!(outcome(&[3, 4]).pipefail(), ProcExit::Code(4));
    }

    #[test]
    fn pipefail_never_calls_a_timeout_a_success() {
        let outcome = PipelineOutcome { exits: vec![ProcExit::Code(0), ProcExit::TimedOut] };
        assert_eq!(outcome.pipefail(), ProcExit::TimedOut);
    }

    #[test]
    fn the_fake_records_the_call_and_replays_the_scripted_answer() {
        let log = EffectLog::new();
        let pipe = FakePipe::new(&log);
        pipe.script(Script::sequence([Ok(outcome(&[0]))]));
        let mut stage = StageSpec::new(NonEmpty::new("codex".to_string(), ["exec".to_string()]));
        stage.stdin = Source::File(PathBuf::from("/prompt"));
        stage.stdout = Sink::AppendFile(PathBuf::from("/log"));
        stage.stderr = Sink::SameAsStdout;
        let spec =
            PipelineSpec { stages: NonEmpty::new(stage, []), timeout: Duration::from_secs(5) };
        assert_eq!(pipe.run_pipeline(&spec).unwrap().pipefail(), ProcExit::Code(0));
        assert_eq!(log.entries(), vec![Effect::Pipe(PipeEffect::Run(spec))]);
    }

    #[test]
    #[should_panic(expected = "unscripted call")]
    fn the_fake_refuses_a_call_nobody_scripted() {
        let log = EffectLog::new();
        let pipe = FakePipe::new(&log);
        let stage = StageSpec::new(NonEmpty::new("true".to_string(), []));
        let spec =
            PipelineSpec { stages: NonEmpty::new(stage, []), timeout: Duration::from_secs(1) };
        let _ = pipe.run_pipeline(&spec);
    }

    #[test]
    fn debug_shows_variable_names_and_never_values() {
        let mut stage = StageSpec::new(NonEmpty::new("opencode".to_string(), []));
        stage.env.push(("GROQ_API_KEY".to_string(), "gsk_supersecret".to_string()));
        let shown = format!("{stage:?}");
        assert!(shown.contains("GROQ_API_KEY"));
        assert!(!shown.contains("supersecret"));
    }
}
