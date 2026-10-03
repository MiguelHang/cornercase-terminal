use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crossterm::event::Event;
use rustix::fs::{FlockOperation, flock};
use rustix::io::Errno;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::host_theme::HostTheme;

pub const VERSION: u32 = 1;
pub const SOCKET_ENV: &str = "CORNERCASE_SOCKET";
pub const NESTED_ENV: &str = "CORNERCASE";
const MAX_MESSAGE: usize = 64 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    KillServer,
    Hello(Box<Hello>),
    Event(Event),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Hello {
    pub version: u32,
    pub build: String,
    pub cols: u16,
    pub rows: u16,
    pub theme: HostTheme,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Rejected(String),
    Frame(Vec<u8>),
    Detached,
    Shutdown,
    Restart(PathBuf),
}

pub fn send<T: Serialize>(w: &mut impl Write, msg: &T) -> io::Result<()> {
    let body = postcard::to_stdvec(msg).map_err(io::Error::other)?;
    let len = u32::try_from(body.len()).map_err(io::Error::other)?;
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(&body);
    w.write_all(&frame)?;
    w.flush()
}

pub fn recv<T: DeserializeOwned>(r: &mut impl Read) -> io::Result<Option<T>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        result => result?,
    }
    let len = usize::try_from(u32::from_le_bytes(len)).map_err(io::Error::other)?;
    if len > MAX_MESSAGE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("message of {len} bytes is too big")));
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body)?;
    postcard::from_bytes(&body).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os(SOCKET_ENV) {
        return path.into();
    }
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(
        || std::env::temp_dir().join(format!("cornercase-{}", rustix::process::getuid().as_raw())),
        |dir| PathBuf::from(dir).join("cornercase"),
    );
    dir.join("server.sock")
}

pub fn log_path(socket: &Path) -> PathBuf {
    socket.with_extension("log")
}

pub fn lock_path(socket: &Path) -> PathBuf {
    socket.with_extension("lock")
}

pub fn try_lock(socket: &Path) -> io::Result<Option<File>> {
    let file = OpenOptions::new().create(true).write(true).truncate(false).open(lock_path(socket))?;
    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(Some(file)),
        Err(Errno::WOULDBLOCK) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn create_socket_dir(socket: &Path) -> io::Result<()> {
    match socket.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir),
        _ => Ok(()),
    }
}

pub fn build_id() -> String {
    std::env::current_exe()
        .and_then(|exe| exe.metadata())
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or_else(|| "unknown".into(), |since| since.as_nanos().to_string())
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;

    fn round_trip<T: Serialize + DeserializeOwned>(msg: &T) -> T {
        let mut buf = Vec::new();
        send(&mut buf, msg).expect("send");
        recv(&mut buf.as_slice()).expect("recv").expect("a message")
    }

    mod framing {
        use super::*;

        #[test]
        fn round_trips_client_events() {
            let key = Event::Key(KeyEvent::new(KeyCode::Char('ñ'), KeyModifiers::ALT));

            let msg = round_trip(&ClientMessage::Event(key.clone()));

            assert!(matches!(msg, ClientMessage::Event(ev) if ev == key));
        }

        #[test]
        fn round_trips_frames() {
            let msg = round_trip(&ServerMessage::Frame(b"\x1b[2Jhello".to_vec()));

            assert!(matches!(msg, ServerMessage::Frame(bytes) if bytes == b"\x1b[2Jhello"));
        }

        #[test]
        fn reads_none_at_end_of_stream() {
            assert!(recv::<ServerMessage>(&mut [].as_slice()).expect("recv").is_none());
        }

        #[test]
        fn rejects_garbage_as_invalid_data() {
            let mut buf = Vec::new();
            send(&mut buf, &[0xffu8; 8]).expect("send");

            let err = recv::<ClientMessage>(&mut buf.as_slice()).expect_err("garbage must not decode");

            assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        }

        #[test]
        fn rejects_oversized_messages() {
            let len = u32::try_from(MAX_MESSAGE + 1).expect("fits in u32").to_le_bytes();

            let err = recv::<ClientMessage>(&mut len.as_slice()).expect_err("too big");

            assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        }
    }

    mod compatibility {
        use super::*;

        #[test]
        fn kill_server_is_the_first_client_variant() {
            assert_eq!(postcard::to_stdvec(&ClientMessage::KillServer).expect("encode"), [0]);
        }

        #[test]
        fn rejected_is_the_first_server_variant() {
            assert_eq!(postcard::to_stdvec(&ServerMessage::Rejected(String::new())).expect("encode"), [0, 0]);
        }
    }

    mod socket_dir {
        use super::*;
        use crate::test_util::TempDir;

        #[test]
        fn is_private_to_the_user() {
            use std::os::unix::fs::PermissionsExt;
            let tmp = TempDir::new();
            let socket = tmp.path().join("nested").join("server.sock");

            create_socket_dir(&socket).expect("create dir");

            let mode = std::fs::metadata(tmp.path().join("nested")).expect("metadata").permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
    }
}
