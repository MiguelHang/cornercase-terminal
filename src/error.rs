use std::path::PathBuf;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to open a pty")]
    OpenPty(#[source] BoxError),
    #[error("failed to spawn shell `{shell}`")]
    SpawnShell {
        shell: String,
        #[source]
        source: BoxError,
    },
    #[error("failed to attach to the pty")]
    AttachPty(#[source] BoxError),
    #[error("failed to create the terminal emulator")]
    Emulator(#[source] BoxError),
    #[error("failed to listen on `{path}`")]
    Listen {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("a cornercase server is already running on `{0}`")]
    ServerRunning(PathBuf),
    #[error("failed to start the cornercase server; see `{log}`")]
    ServerStart {
        log: PathBuf,
        #[source]
        source: Option<std::io::Error>,
    },
    #[error("cornercase is already running in this terminal; nesting it is not supported")]
    Nested,
    #[error("{0}")]
    Rejected(String),
    #[error("failed to register signal handlers")]
    Signals(#[source] std::io::Error),
    #[error("failed to run git: {0}")]
    RunGit(std::io::Error),
    #[error("{0}")]
    Git(String),
    #[error("failed to run gh: {0}")]
    RunGh(std::io::Error),
    #[error("{0}")]
    Gh(String),
    #[error("gh printed something unexpected: {0}")]
    GhOutput(serde_json::Error),
    #[error("{0}")]
    Api(String),
    #[error("{0}")]
    Usage(String),
    #[error("`{0}` already exists")]
    PathExists(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
