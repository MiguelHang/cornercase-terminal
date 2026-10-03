use std::fs::OpenOptions;
use std::io::{self, Write, stdin, stdout};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use ratatui::DefaultTerminal;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

use crate::error::{Error, Result};
use crate::host_theme::{HostTheme, ThemeProbe};
use crate::protocol::{self, ClientMessage, Hello, ServerMessage};

const THEME_QUERY_TIMEOUT: Duration = Duration::from_secs(1);
const SERVER_START_TIMEOUT: Duration = Duration::from_secs(5);
const SERVER_EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);
const INCOMPATIBLE: &str = "the running cornercase server is incompatible with this build. \
    Run `cornercase kill-server` (it closes all its terminals) and start cornercase again";

pub fn run() -> Result<()> {
    if std::env::var_os(protocol::NESTED_ENV).is_some() {
        return Err(Error::Nested);
    }
    let stream = connect_or_start(&protocol::socket_path())?;

    let terminal = ratatui::init();
    let _ = execute!(
        stdout(),
        EnableBracketedPaste,
        EnableMouseCapture,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    install_panic_hook();
    let result = attach(stream, &terminal);
    restore_input_modes();
    ratatui::restore();
    result
}

pub fn kill_server() -> Result<bool> {
    let path = protocol::socket_path();
    let Ok(mut stream) = UnixStream::connect(&path) else { return Ok(false) };
    protocol::send(&mut stream, &ClientMessage::KillServer)?;
    while let Ok(Some(_)) = protocol::recv::<ServerMessage>(&mut stream) {}
    let deadline = Instant::now() + SERVER_EXIT_TIMEOUT;
    while matches!(protocol::try_lock(&path), Ok(None)) && Instant::now() < deadline {
        thread::sleep(POLL);
    }
    Ok(true)
}

fn connect_or_start(path: &Path) -> Result<UnixStream> {
    if let Ok(stream) = UnixStream::connect(path) {
        return Ok(stream);
    }
    let log = protocol::log_path(path);
    let start_error = |e| Error::ServerStart { log: log.clone(), source: Some(e) };
    let deadline = Instant::now() + SERVER_START_TIMEOUT;
    let mut server = start_server(path, &log).map_err(start_error)?;
    loop {
        if let Ok(stream) = UnixStream::connect(path) {
            reap(server);
            return Ok(stream);
        }
        if Instant::now() >= deadline {
            reap(server);
            return Err(Error::ServerStart { log, source: None });
        }
        if matches!(server.try_wait(), Ok(Some(_))) {
            server = start_server(path, &log).map_err(start_error)?;
        }
        thread::sleep(POLL);
    }
}

fn start_server(path: &Path, log: &Path) -> io::Result<Child> {
    protocol::create_socket_dir(path)?;
    let log = OpenOptions::new().create(true).append(true).open(log)?;
    Command::new(std::env::current_exe()?)
        .arg("server")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
}

fn reap(mut child: Child) {
    thread::spawn(move || child.wait());
}

fn install_panic_hook() {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_input_modes();
        prev_hook(info);
    }));
}

fn restore_input_modes() {
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags, DisableBracketedPaste, DisableMouseCapture);
}

fn attach(stream: UnixStream, terminal: &DefaultTerminal) -> Result<()> {
    let theme = query_host_theme();
    let size = terminal.size()?;
    let mut writer = stream.try_clone()?;
    let hello =
        Hello { version: protocol::VERSION, build: protocol::build_id(), cols: size.width, rows: size.height, theme };
    protocol::send(&mut writer, &ClientMessage::Hello(Box::new(hello)))?;
    spawn_input_thread(writer);
    spawn_signal_thread(stream.try_clone()?)?;
    receive(stream)
}

fn receive(mut stream: UnixStream) -> Result<()> {
    let mut out = stdout();
    loop {
        match protocol::recv::<ServerMessage>(&mut stream) {
            Ok(Some(ServerMessage::Frame(bytes))) => {
                out.write_all(&bytes)?;
                out.flush()?;
            }
            Ok(Some(ServerMessage::Rejected(reason))) => return Err(Error::Rejected(reason)),
            Err(e) if e.kind() == io::ErrorKind::InvalidData => return Err(Error::Rejected(INCOMPATIBLE.into())),
            Ok(Some(ServerMessage::Detached | ServerMessage::Shutdown) | None) | Err(_) => return Ok(()),
        }
    }
}

fn query_host_theme() -> HostTheme {
    let mut out = stdout();
    if out.write_all(HostTheme::query().as_bytes()).and_then(|()| out.flush()).is_err() {
        return HostTheme::default();
    }
    let input = stdin();
    let deadline = Instant::now() + THEME_QUERY_TIMEOUT;
    let mut probe = ThemeProbe::default();
    let mut buf = [0u8; 4096];
    while !probe.is_done() {
        let Ok(left) = Timespec::try_from(deadline.saturating_duration_since(Instant::now())) else { break };
        let mut fds = [PollFd::new(&input, PollFlags::IN)];
        if !matches!(poll(&mut fds, Some(&left)), Ok(1..)) {
            break;
        }
        match rustix::io::read(&input, &mut buf) {
            Ok(n @ 1..) => probe.feed(&buf[..n]),
            _ => break,
        }
    }
    probe.finish()
}

fn spawn_input_thread(mut writer: UnixStream) {
    thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if protocol::send(&mut writer, &ClientMessage::Event(ev)).is_err() {
                return;
            }
        }
    });
}

fn spawn_signal_thread(stream: UnixStream) -> Result<()> {
    let mut signals = Signals::new([SIGTERM, SIGHUP, SIGINT]).map_err(Error::Signals)?;
    thread::spawn(move || {
        if signals.forever().next().is_some() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    });
    Ok(())
}
