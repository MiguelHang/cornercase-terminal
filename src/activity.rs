use std::ffi::OsString;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Deserialize;

pub const CLAUDE_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";
pub const CLAUDE_SESSION_ENV: [&str; 10] = [
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
];
const SPINNER: [char; 4] = ['◐', '◓', '◑', '◒'];
const BRAILLE: RangeInclusive<char> = '\u{2800}'..='\u{28ff}';
const IDLE: char = '✳';
const NOTIFY_AFTER: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Working,
    Waiting,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Idle,
    Working,
    Done,
    Waiting,
}

impl Status {
    pub fn needs_you(self) -> bool {
        matches!(self, Self::Done | Self::Waiting)
    }
}

pub fn attention(statuses: impl IntoIterator<Item = Option<Status>>) -> Option<Status> {
    statuses.into_iter().flatten().filter(|s| s.needs_you()).max()
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pane {
    activity: Option<Activity>,
    unseen: bool,
    since: Option<Instant>,
    notified: bool,
}

impl Pane {
    pub fn update(&mut self, activity: Option<Activity>, seen: bool, now: Instant) -> Option<Status> {
        let before = self.status();
        let finished = matches!(self.activity, Some(Activity::Working | Activity::Waiting));
        self.unseen = activity == Some(Activity::Idle) && !seen && (self.unseen || finished);
        self.activity = activity;
        let status = self.status();
        if status != before {
            self.since = Some(now);
            self.notified = false;
        }
        self.notified |= seen;
        let settled = self.since.is_some_and(|since| now.duration_since(since) >= NOTIFY_AFTER);
        if !settled || self.notified || !status.is_some_and(Status::needs_you) {
            return None;
        }
        self.notified = true;
        status
    }

    pub fn see(&mut self) {
        self.unseen = false;
        self.notified = true;
    }

    pub fn status(self) -> Option<Status> {
        Some(match self.activity? {
            Activity::Working => Status::Working,
            Activity::Waiting => Status::Waiting,
            Activity::Idle if self.unseen => Status::Done,
            Activity::Idle => Status::Idle,
        })
    }
}

#[derive(Deserialize)]
struct Session {
    pid: Option<i64>,
    status: Option<String>,
}

pub fn claude_dir(home: Option<&Path>) -> Option<PathBuf> {
    claude_dir_from(std::env::var_os(CLAUDE_DIR_ENV), home)
}

fn claude_dir_from(env: Option<OsString>, home: Option<&Path>) -> Option<PathBuf> {
    match env {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => home.map(|home| home.join(".claude")),
    }
}

pub fn claude(dir: Option<&Path>, pid: i32, title: &str) -> Activity {
    dir.and_then(|dir| std::fs::read_to_string(dir.join("sessions").join(format!("{pid}.json"))).ok())
        .and_then(|text| session(&text, pid))
        .or_else(|| from_title(title))
        .unwrap_or(Activity::Idle)
}

fn session(text: &str, pid: i32) -> Option<Activity> {
    let session: Session = serde_json::from_str(text).ok()?;
    if session.pid.is_some_and(|p| p != i64::from(pid)) {
        return None;
    }
    match session.status.as_deref()? {
        "busy" => Some(Activity::Working),
        "waiting" => Some(Activity::Waiting),
        "idle" | "shell" => Some(Activity::Idle),
        _ => None,
    }
}

fn from_title(title: &str) -> Option<Activity> {
    let mut chars = title.chars();
    let glyph = chars.next()?;
    if chars.next() != Some(' ') {
        return None;
    }
    if SPINNER.contains(&glyph) || BRAILLE.contains(&glyph) {
        Some(Activity::Working)
    } else if glyph == IDLE {
        Some(Activity::Idle)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::test_util::TempDir;

    mod session_file {
        use super::*;

        #[rstest]
        #[case::busy(r#"{"pid":7,"status":"busy"}"#, Some(Activity::Working))]
        #[case::permission(r#"{"pid":7,"status":"waiting","waitingFor":"permission prompt"}"#, Some(Activity::Waiting))]
        #[case::question(r#"{"pid":7,"status":"waiting","waitingFor":"input needed"}"#, Some(Activity::Waiting))]
        #[case::idle(r#"{"pid":7,"status":"idle"}"#, Some(Activity::Idle))]
        #[case::background_shell(r#"{"pid":7,"status":"shell"}"#, Some(Activity::Idle))]
        #[case::no_status_yet(r#"{"pid":7,"sessionId":"a"}"#, None)]
        #[case::unknown_status(r#"{"pid":7,"status":"dreaming"}"#, None)]
        #[case::another_process(r#"{"pid":8,"status":"busy"}"#, None)]
        #[case::half_written(r#"{"pid":7,"sta"#, None)]
        fn says_what_claude_is_doing(#[case] text: &str, #[case] expected: Option<Activity>) {
            assert_eq!(session(text, 7), expected);
        }

        #[test]
        fn is_read_from_the_sessions_folder() {
            let dir = TempDir::new();
            std::fs::create_dir(dir.path().join("sessions")).expect("create sessions");
            std::fs::write(dir.path().join("sessions/42.json"), r#"{"pid":42,"status":"waiting"}"#).expect("write");

            assert_eq!(claude(Some(dir.path()), 42, "✳ Claude Code"), Activity::Waiting);
        }

        #[test]
        fn falls_back_to_the_title_without_one() {
            let dir = TempDir::new();

            assert_eq!(claude(Some(dir.path()), 42, "◑ fix the login"), Activity::Working);
        }

        #[test]
        fn counts_as_idle_without_a_file_or_a_title() {
            assert_eq!(claude(None, 42, ""), Activity::Idle);
        }
    }

    mod title {
        use super::*;

        #[rstest]
        #[case::half_circle("◐ fix the login", Some(Activity::Working))]
        #[case::other_half("◑ fix the login", Some(Activity::Working))]
        #[case::braille("⠂ fix the login", Some(Activity::Working))]
        #[case::idle("✳ Claude Code", Some(Activity::Idle))]
        #[case::shell_title("zsh", None)]
        #[case::glyph_without_space("◐x", None)]
        #[case::empty("", None)]
        fn shows_whether_claude_works(#[case] title: &str, #[case] expected: Option<Activity>) {
            assert_eq!(from_title(title), expected);
        }
    }

    mod pane {
        use super::*;

        fn after(steps: &[(Option<Activity>, bool)]) -> Option<Status> {
            let mut pane = Pane::default();
            let now = Instant::now();
            for (activity, seen) in steps {
                pane.update(*activity, *seen, now);
            }
            pane.status()
        }

        #[rstest]
        #[case::nothing_runs(&[(None, false)], None)]
        #[case::fresh_claude(&[(Some(Activity::Idle), false)], Some(Status::Idle))]
        #[case::working(&[(Some(Activity::Working), false)], Some(Status::Working))]
        #[case::asking(&[(Some(Activity::Working), false), (Some(Activity::Waiting), false)], Some(Status::Waiting))]
        #[case::finished_out_of_sight(&[(Some(Activity::Working), false), (Some(Activity::Idle), false)], Some(Status::Done))]
        #[case::finished_in_sight(&[(Some(Activity::Working), true), (Some(Activity::Idle), true)], Some(Status::Idle))]
        #[case::stays_done(
            &[(Some(Activity::Working), false), (Some(Activity::Idle), false), (Some(Activity::Idle), false)],
            Some(Status::Done)
        )]
        #[case::seen_later(
            &[(Some(Activity::Working), false), (Some(Activity::Idle), false), (Some(Activity::Idle), true)],
            Some(Status::Idle)
        )]
        #[case::works_again(
            &[(Some(Activity::Working), false), (Some(Activity::Idle), false), (Some(Activity::Working), false)],
            Some(Status::Working)
        )]
        #[case::answered_out_of_sight(&[(Some(Activity::Waiting), false), (Some(Activity::Idle), false)], Some(Status::Done))]
        #[case::exited(&[(Some(Activity::Working), false), (Some(Activity::Idle), false), (None, false)], None)]
        fn follows_the_agent(#[case] steps: &[(Option<Activity>, bool)], #[case] expected: Option<Status>) {
            assert_eq!(after(steps), expected);
        }

        #[test]
        fn seeing_it_clears_done() {
            let mut pane = Pane::default();
            let now = Instant::now();
            pane.update(Some(Activity::Working), false, now);
            pane.update(Some(Activity::Idle), false, now);

            pane.see();

            assert_eq!(pane.status(), Some(Status::Idle));
        }
    }

    mod notice {
        use super::*;

        const WORKING: (Option<Activity>, bool) = (Some(Activity::Working), false);
        const WAITING: (Option<Activity>, bool) = (Some(Activity::Waiting), false);
        const IDLE: (Option<Activity>, bool) = (Some(Activity::Idle), false);
        const WAITING_IN_SIGHT: (Option<Activity>, bool) = (Some(Activity::Waiting), true);
        const TICK: Duration = Duration::from_millis(500);

        fn notices(steps: &[(Option<Activity>, bool)]) -> Vec<(usize, Status)> {
            let mut pane = Pane::default();
            let start = Instant::now();
            let mut now = start;
            let mut notices = Vec::new();
            for (i, (activity, seen)) in steps.iter().enumerate() {
                if let Some(status) = pane.update(*activity, *seen, now) {
                    notices.push((i, status));
                }
                now += TICK;
            }
            notices
        }

        #[rstest]
        #[case::asks_out_of_sight(&[WORKING, WAITING, WAITING, WAITING, WAITING], &[(3, Status::Waiting)])]
        #[case::finishes_out_of_sight(&[WORKING, IDLE, IDLE, IDLE, IDLE], &[(3, Status::Done)])]
        #[case::asks_in_sight(&[WORKING, WAITING_IN_SIGHT, WAITING_IN_SIGHT, WAITING_IN_SIGHT], &[])]
        #[case::flips_straight_back(&[WORKING, WAITING, WORKING, WORKING, WORKING], &[])]
        #[case::seen_before_it_settled(&[WORKING, WAITING_IN_SIGHT, WAITING, WAITING, WAITING], &[])]
        #[case::asks_then_finishes(&[WORKING, WAITING, WAITING, WAITING, IDLE, IDLE, IDLE], &[(3, Status::Waiting), (6, Status::Done)])]
        #[case::finishes_twice(&[WORKING, IDLE, IDLE, IDLE, WORKING, IDLE, IDLE, IDLE], &[(3, Status::Done), (7, Status::Done)])]
        #[case::only_works(&[WORKING, WORKING, WORKING, IDLE], &[])]
        fn comes_once_the_change_settles(
            #[case] steps: &[(Option<Activity>, bool)],
            #[case] expected: &[(usize, Status)],
        ) {
            assert_eq!(notices(steps), expected);
        }

        #[test]
        fn none_comes_once_the_tab_was_seen() {
            let mut pane = Pane::default();
            let now = Instant::now();
            pane.update(Some(Activity::Working), false, now);
            pane.update(Some(Activity::Waiting), false, now);

            pane.see();

            assert_eq!(pane.update(Some(Activity::Waiting), false, now + NOTIFY_AFTER), None);
        }
    }

    mod rollup {
        use super::*;

        #[rstest]
        #[case::nothing(&[], None)]
        #[case::only_work(&[Some(Status::Working), Some(Status::Idle), None], None)]
        #[case::done(&[Some(Status::Working), Some(Status::Done)], Some(Status::Done))]
        #[case::waiting_wins(&[Some(Status::Done), Some(Status::Waiting), Some(Status::Working)], Some(Status::Waiting))]
        fn keeps_what_needs_you(#[case] statuses: &[Option<Status>], #[case] expected: Option<Status>) {
            assert_eq!(attention(statuses.iter().copied()), expected);
        }
    }

    mod config_dir {
        use super::*;

        #[rstest]
        #[case::default(None, Some("/home/a/.claude"))]
        #[case::from_the_environment(Some("/opt/claude"), Some("/opt/claude"))]
        #[case::empty_variable(Some(""), Some("/home/a/.claude"))]
        fn is_where_claude_keeps_its_sessions(#[case] env: Option<&str>, #[case] expected: Option<&str>) {
            let found = claude_dir_from(env.map(OsString::from), Some(Path::new("/home/a")));
            assert_eq!(found, expected.map(PathBuf::from));
        }
    }
}
