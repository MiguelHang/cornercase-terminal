use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

pub const EXTENSION: &str = "code-workspace";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub name: String,
    pub folders: Vec<Folder>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub path: PathBuf,
    pub name: Option<String>,
}

#[derive(Deserialize)]
struct File {
    #[serde(default)]
    folders: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    path: Option<String>,
    uri: Option<String>,
    name: Option<String>,
}

pub fn is_workspace(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == EXTENSION)
}

pub fn read(file: &Path) -> Result<Workspace> {
    let text = std::fs::read_to_string(file)?;
    let invalid = |e: serde_json::Error| Error::CodeWorkspace { path: file.to_path_buf(), reason: e.to_string() };
    let parsed: File = serde_json::from_str(&strip_jsonc(&text)).map_err(invalid)?;
    let base = file.parent().unwrap_or(Path::new("/"));
    let name = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let folders = parsed
        .folders
        .into_iter()
        .filter_map(|entry| {
            let path = match (entry.path, entry.uri) {
                (Some(path), _) => base.join(path),
                (None, Some(uri)) => PathBuf::from(file_uri(&uri)?),
                (None, None) => return None,
            };
            Some(Folder { path, name: entry.name.filter(|n| !n.trim().is_empty()) })
        })
        .collect();
    Ok(Workspace { name, folders })
}

fn file_uri(uri: &str) -> Option<String> {
    let path = uri.strip_prefix("file://")?;
    let path = path.strip_prefix("localhost").unwrap_or(path);
    if !path.starts_with('/') {
        return None;
    }
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%').then(|| path.get(i + 1..i + 3)).flatten();
        let (byte, width) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()).map_or((bytes[i], 1), |b| (b, 3));
        out.push(byte);
        i += width;
    }
    String::from_utf8(out).ok()
}

fn strip_jsonc(text: &str) -> String {
    without_trailing_commas(&without_comments(text))
}

fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => while chars.next_if(|&c| c != '\n').is_some() {},
            ('/', Some('*')) => {
                chars.next();
                let mut last = ' ';
                for c in chars.by_ref() {
                    if last == '*' && c == '/' {
                        break;
                    }
                    last = c;
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

fn without_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        if in_string {
            out.push(c);
            match c {
                '\\' if i < chars.len() => {
                    out.push(chars[i]);
                    i += 1;
                }
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        if c == '"' {
            in_string = true;
        }
        let closes = chars[i..].iter().find(|c| !c.is_whitespace()).is_some_and(|&c| c == '}' || c == ']');
        if c != ',' || !closes {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_util::TempDir;

    fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
        let file = dir.join(name);
        std::fs::write(&file, text).expect("write workspace");
        file
    }

    mod reading {
        use super::*;

        #[test]
        fn takes_its_name_from_the_file() {
            let tmp = TempDir::new();
            let file = write(tmp.path(), "work.code-workspace", r#"{"folders": []}"#);

            assert_eq!(read(&file).expect("read").name, "work");
        }

        #[test]
        fn resolves_relative_folders_from_the_file() {
            let tmp = TempDir::new();
            let file = write(tmp.path(), "w.code-workspace", r#"{"folders": [{"path": "api"}, {"path": "../web"}]}"#);

            let paths: Vec<PathBuf> = read(&file).expect("read").folders.into_iter().map(|f| f.path).collect();

            assert_eq!(paths, [tmp.path().join("api"), tmp.path().join("../web")]);
        }

        #[test]
        fn keeps_absolute_folders_and_their_names() {
            let tmp = TempDir::new();
            let file = write(tmp.path(), "w.code-workspace", r#"{"folders": [{"path": "/srv/api", "name": "API"}]}"#);

            let folders = read(&file).expect("read").folders;

            assert_eq!(folders, [Folder { path: PathBuf::from("/srv/api"), name: Some("API".into()) }]);
        }

        #[test]
        fn reads_local_file_uris_and_skips_remote_ones() {
            let tmp = TempDir::new();
            let text = r#"{"folders": [
                {"uri": "file:///home/me/my%20app"},
                {"uri": "file://localhost/srv/api"},
                {"uri": "file://server/share/web"},
                {"uri": "vscode-remote://ssh-remote+box/srv/api"}
            ]}"#;
            let file = write(tmp.path(), "w.code-workspace", text);

            let paths: Vec<PathBuf> = read(&file).expect("read").folders.into_iter().map(|f| f.path).collect();

            assert_eq!(paths, [PathBuf::from("/home/me/my app"), PathBuf::from("/srv/api")]);
        }

        #[test]
        fn accepts_comments_and_trailing_commas() {
            let tmp = TempDir::new();
            let text = r#"{
                // the repos
                "folders": [
                    { "path": "api", }, /* the backend */
                    { "path": "web" },
                ],
                "settings": { "url": "http://example.com/x", },
            }"#;
            let file = write(tmp.path(), "w.code-workspace", text);

            assert_eq!(read(&file).expect("read").folders.len(), 2);
        }

        #[test]
        fn says_why_a_broken_file_cannot_be_read() {
            let tmp = TempDir::new();
            let file = write(tmp.path(), "w.code-workspace", "{ nope");

            let error = read(&file).expect_err("broken file").to_string();

            assert!(error.contains("w.code-workspace"), "{error}");
        }
    }

    mod jsonc {
        use super::*;

        #[rstest]
        #[case::line_comment("{\"a\": 1 // x\n}", r#"{"a": 1}"#)]
        #[case::block_comment(r#"{/* x */"a": 1}"#, r#"{"a": 1}"#)]
        #[case::slashes_in_strings(r#"{"a": "//not /* a comment */"}"#, r#"{"a": "//not /* a comment */"}"#)]
        #[case::escaped_quotes(r#"{"a": "\"// x"}"#, r#"{"a": "\"// x"}"#)]
        #[case::trailing_commas(r#"{"a": [1, 2, ], }"#, r#"{"a": [1, 2]}"#)]
        #[case::commas_in_strings(r#"{"a": ", }"}"#, r#"{"a": ", }"}"#)]
        fn becomes_plain_json(#[case] text: &str, #[case] json: &str) {
            let got: serde_json::Value = serde_json::from_str(&strip_jsonc(text)).expect("valid json");
            let want: serde_json::Value = serde_json::from_str(json).expect("valid json");
            assert_eq!(got, want);
        }
    }
}
