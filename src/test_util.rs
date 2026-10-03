use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);

pub fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    while !cond() {
        assert!(Instant::now() < deadline, "timed out waiting for: {what}");
        std::thread::sleep(POLL);
    }
}

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("cornercase-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir.canonicalize().expect("canonicalize temp dir"))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn is_sh(name: &str) -> bool {
    matches!(name, "sh" | "bash" | "dash")
}

pub fn write_executable(path: &Path, contents: &str) {
    let mut child = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg("cat > \"$1\" && chmod 755 \"$1\"")
        .arg("sh")
        .arg(path)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("run sh");
    std::io::Write::write_all(&mut child.stdin.take().expect("stdin"), contents.as_bytes()).expect("write script");
    assert!(child.wait().expect("wait for sh").success(), "failed to write {}", path.display());
}

pub fn fake_gh(dir: &Path, script: &str) -> PathBuf {
    let path = dir.join("gh");
    write_executable(&path, &format!("#!/bin/sh\n{script}\n"));
    path
}

pub fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com", "-c", "init.defaultBranch=main"])
        .args(args)
        .output()
        .expect("run git");
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
}

pub fn git_repo(files: &[(&str, &str)]) -> TempDir {
    let repo = TempDir::new();
    git(repo.path(), &["init", "--quiet"]);
    for (name, contents) in files {
        let path = repo.path().join(name);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("create folder");
        }
        std::fs::write(path, contents).expect("write file");
    }
    git(repo.path(), &["add", "--all"]);
    git(repo.path(), &["commit", "--quiet", "--allow-empty", "-m", "init"]);
    repo
}

pub struct FakeHttp {
    addr: std::net::SocketAddr,
    requests: std::sync::Arc<parking_lot::Mutex<Vec<String>>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl FakeHttp {
    pub fn start<B: Into<String>>(routes: Vec<(&'static str, u16, B)>) -> Self {
        let routes: Vec<(&'static str, u16, String)> = routes.into_iter().map(|(k, s, b)| (k, s, b.into())).collect();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind fake http");
        let addr = listener.local_addr().expect("fake http address");
        let requests = std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (log, stopped) = (std::sync::Arc::clone(&requests), std::sync::Arc::clone(&stop));
        std::thread::spawn(move || {
            let mut used = vec![false; routes.len()];
            for stream in listener.incoming() {
                if stopped.load(Ordering::Relaxed) {
                    return;
                }
                let Ok(mut stream) = stream else { continue };
                let request = read_request(&mut stream);
                let line = request.lines().next().unwrap_or_default().to_string();
                let mut parts = line.split(' ');
                let method = parts.next().unwrap_or_default();
                let path = parts.next().unwrap_or_default().split('?').next().unwrap_or_default();
                let key = format!("{method} {path}");
                let matching: Vec<usize> = (0..routes.len()).filter(|&i| routes[i].0 == key).collect();
                let pick = matching.iter().copied().find(|&i| !used[i]).or_else(|| matching.last().copied());
                let (status, body) = pick.map_or((404, "{}"), |i| {
                    used[i] = true;
                    (routes[i].1, routes[i].2.as_str())
                });
                log.lock().push(request);
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
            }
        });
        Self { addr, requests, stop }
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn request(&self, i: usize) -> String {
        self.requests.lock().get(i).cloned().unwrap_or_default()
    }

    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().clone()
    }
}

impl Drop for FakeHttp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = std::net::TcpStream::connect(self.addr);
    }
}

fn read_request(stream: &mut std::net::TcpStream) -> String {
    use std::io::Read;
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    while let Ok(n @ 1..) = stream.read(&mut buf) {
        data.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&data);
        if let Some(end) = text.find("\r\n\r\n") {
            let length = text[..end]
                .lines()
                .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)))
                .unwrap_or(0usize);
            if data.len() >= end + 4 + length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&data).into_owned()
}
