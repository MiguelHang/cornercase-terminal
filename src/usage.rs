use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

use crate::activity;
use crate::error::{Error, Result};
use crate::issues;
use crate::ui;

pub const TIMEOUT: Duration = Duration::from_secs(10);
const ARGS: [&str; 9] = [
    "-p",
    "--setting-sources",
    "",
    "--no-session-persistence",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
];
const USAGE_REQUEST: &str = "usage";
const REQUESTS: &str = concat!(
    r#"{"type":"control_request","request_id":"initialize","request":{"subtype":"initialize"}}"#,
    "\n",
    r#"{"type":"control_request","request_id":"usage","request":{"subtype":"get_usage"}}"#,
    "\n",
);
const WARNING_FROM: u16 = 75;
const CRITICAL_FROM: u16 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Normal,
    Warning,
    Critical,
}

impl Severity {
    fn parse(text: Option<&str>, percent: u16) -> Self {
        match text {
            Some("normal") => Self::Normal,
            Some("warning") => Self::Warning,
            _ if percent >= CRITICAL_FROM => Self::Critical,
            _ if percent >= WARNING_FROM => Self::Warning,
            _ => Self::Normal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub label: String,
    pub percent: u16,
    pub severity: Severity,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub plan: Option<String>,
    pub limited: bool,
    pub windows: Vec<Window>,
    pub extra: Option<String>,
}

#[derive(Deserialize)]
struct Body {
    subscription_type: Option<String>,
    #[serde(default)]
    rate_limits_available: bool,
    rate_limits: Option<Limits>,
}

#[derive(Deserialize)]
struct Limits {
    #[serde(default, rename = "limits")]
    list: Vec<Limit>,
    five_hour: Option<Utilization>,
    seven_day: Option<Utilization>,
    spend: Option<Spend>,
}

#[derive(Deserialize)]
struct Limit {
    kind: String,
    percent: Option<f64>,
    severity: Option<String>,
    resets_at: Option<String>,
    scope: Option<Scope>,
}

#[derive(Deserialize)]
struct Scope {
    model: Option<Model>,
}

#[derive(Deserialize)]
struct Model {
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct Utilization {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

#[derive(Deserialize)]
struct Spend {
    #[serde(default)]
    enabled: bool,
    used: Option<Money>,
    limit: Option<Money>,
}

#[derive(Deserialize)]
struct Money {
    amount_minor: i64,
    currency: Option<String>,
    exponent: u32,
}

impl Money {
    fn text(&self) -> String {
        let scale = 10_i64.checked_pow(self.exponent).unwrap_or(1);
        let (units, minor) = (self.amount_minor / scale, (self.amount_minor % scale).abs());
        let amount = if self.exponent == 0 {
            units.to_string()
        } else {
            format!("{units}.{minor:0width$}", width = self.exponent as usize)
        };
        match &self.currency {
            Some(currency) => format!("{amount} {currency}"),
            None => amount,
        }
    }
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0..=999 first")]
fn whole(percent: Option<f64>) -> u16 {
    percent.unwrap_or(0.0).clamp(0.0, 999.0).round() as u16
}

fn window(label: String, percent: Option<f64>, severity: Option<&str>, resets_at: Option<&str>) -> Window {
    let percent = whole(percent);
    Window {
        label,
        percent,
        severity: Severity::parse(severity, percent),
        resets_at: resets_at.and_then(issues::parse_time),
    }
}

fn label(limit: &Limit) -> String {
    let model = limit.scope.as_ref().and_then(|s| s.model.as_ref()).and_then(|m| m.display_name.as_deref());
    match (limit.kind.as_str(), model) {
        ("session", _) => "session (5h)".into(),
        ("weekly_all", _) => "week".into(),
        ("weekly_scoped", Some(model)) => format!("week · {model}"),
        (kind, Some(model)) => format!("{} · {model}", kind.replace('_', " ")),
        (kind, None) => kind.replace('_', " "),
    }
}

fn windows(limits: &Limits) -> Vec<Window> {
    if !limits.list.is_empty() {
        return limits
            .list
            .iter()
            .map(|l| window(label(l), l.percent, l.severity.as_deref(), l.resets_at.as_deref()))
            .collect();
    }
    [("session (5h)", &limits.five_hour), ("week", &limits.seven_day)]
        .into_iter()
        .filter_map(|(name, u)| u.as_ref().map(|u| window(name.into(), u.utilization, None, u.resets_at.as_deref())))
        .collect()
}

fn extra(spend: &Spend) -> Option<String> {
    let used = spend.used.as_ref().filter(|_| spend.enabled)?.text();
    Some(match &spend.limit {
        Some(limit) => format!("extra usage: {used} of {}", limit.text()),
        None => format!("extra usage: {used}"),
    })
}

pub fn parse(body: Value) -> Result<Report> {
    let body: Body = serde_json::from_value(body).map_err(|e| Error::Usage(format!("unexpected answer: {e}")))?;
    let limits = body.rate_limits.filter(|_| body.rate_limits_available);
    Ok(Report {
        plan: body.subscription_type,
        limited: limits.is_some(),
        windows: limits.as_ref().map(windows).unwrap_or_default(),
        extra: limits.as_ref().and_then(|l| l.spend.as_ref()).and_then(extra),
    })
}

pub fn answer(line: &str) -> Option<Result<Report>> {
    let message: Value = serde_json::from_str(line).ok()?;
    let response = message.get("response").filter(|_| message["type"] == "control_response")?;
    if response["request_id"] != USAGE_REQUEST {
        return None;
    }
    if response["subtype"] == "success" {
        return Some(parse(response["response"].clone()));
    }
    let error = response["error"].as_str().unwrap_or("claude refused the request");
    Some(Err(Error::Usage(error.to_string())))
}

fn read_answer(reader: impl BufRead) -> Result<Report> {
    for line in reader.lines() {
        if let Some(result) = answer(&line?) {
            return result;
        }
    }
    Err(Error::Usage("claude exited without answering".into()))
}

fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(ARGS).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    for key in activity::CLAUDE_SESSION_ENV {
        cmd.env_remove(key);
    }
    cmd
}

pub fn probe(program: &str, timeout: Duration) -> Result<Report> {
    let mut child = command(program).spawn().map_err(|e| Error::Usage(format!("could not run {program}: {e}")))?;
    let mut stdin = child.stdin.take();
    let written = stdin.as_mut().map(|s| s.write_all(REQUESTS.as_bytes()).and_then(|()| s.flush()));
    let (tx, rx) = mpsc::channel();
    if let (Some(Ok(())), Some(stdout)) = (written, child.stdout.take()) {
        std::thread::spawn(move || {
            let _ = tx.send(read_answer(BufReader::new(stdout)));
        });
    }
    let answer = rx.recv_timeout(timeout);
    let _ = child.kill();
    let _ = child.wait();
    drop(stdin);
    match answer {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(Error::Usage("claude did not answer in time".into())),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::Usage("claude exited without answering".into())),
    }
}

fn until(secs: i64) -> String {
    let minutes = (secs.max(0) + 59) / 60;
    let (days, hours, minutes) = (minutes / 1_440, minutes / 60 % 24, minutes % 60);
    if days > 0 {
        format!("resets in {days}d {hours}h")
    } else if hours > 0 {
        format!("resets in {hours}h {minutes}m")
    } else {
        format!("resets in {minutes}m")
    }
}

fn updated(secs: u64) -> String {
    if secs < 60 {
        "updated just now".into()
    } else {
        format!("updated {} ago", issues::age(i64::try_from(secs).unwrap_or(i64::MAX)))
    }
}

#[derive(Debug, Default)]
pub struct State {
    last: Option<(Report, Instant)>,
    loading: bool,
    error: Option<String>,
}

impl State {
    pub fn start(&mut self) -> bool {
        if self.loading {
            return false;
        }
        self.loading = true;
        self.error = None;
        true
    }

    pub fn answered(&mut self, result: Result<Report>, now: Instant) {
        self.loading = false;
        match result {
            Ok(report) => self.last = Some((report, now)),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn view(&self, now: Instant, clock: i64) -> ui::Usage {
        let report = self.last.as_ref().map(|(r, _)| r);
        let plan = report.and_then(|r| r.plan.as_deref()).map_or_else(String::new, |p| format!(" · {p} plan"));
        let windows = report.map(|r| r.windows.as_slice()).unwrap_or_default();
        let note = if self.loading {
            Some(ui::Note::Busy("loading…"))
        } else {
            self.error.as_ref().map(|e| ui::Note::Error(format!("usage unavailable: {e}")))
        };
        ui::Usage {
            title: format!("Claude Code{plan}"),
            windows: windows
                .iter()
                .map(|w| ui::UsageWindow {
                    label: w.label.clone(),
                    percent: w.percent,
                    severity: w.severity,
                    resets: w.resets_at.map(|at| until(at - clock)).unwrap_or_default(),
                })
                .collect(),
            empty: report
                .filter(|r| !r.limited)
                .map(|_| "no plan limits for this account (API key, Bedrock or Vertex)"),
            extra: report.and_then(|r| r.extra.clone()),
            age: self.last.as_ref().map(|(_, at)| updated(now.duration_since(*at).as_secs())).unwrap_or_default(),
            note,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde_json::json;

    use super::*;
    use crate::test_util::{TempDir, write_executable};

    const RECORDED: &str = r#"{"type":"control_response","response":{"subtype":"success","request_id":"usage","response":{"session":{"total_cost_usd":0},"subscription_type":"team","rate_limits_available":true,"rate_limits":{"five_hour":{"utilization":7,"resets_at":"2026-10-04T13:59:59.953605+00:00"},"seven_day":{"utilization":74,"resets_at":"2026-10-04T15:59:59.953630+00:00"},"extra_usage":{"is_enabled":false},"limits":[{"kind":"session","group":"session","percent":7,"severity":"normal","resets_at":"2026-10-04T13:59:59.953605+00:00","scope":null,"is_active":false},{"kind":"weekly_all","group":"weekly","percent":74,"severity":"normal","resets_at":"2026-10-04T15:59:59.953630+00:00","scope":null,"is_active":true},{"kind":"weekly_scoped","group":"weekly","percent":0,"severity":"normal","resets_at":"2026-10-04T16:00:00+00:00","scope":{"model":{"id":null,"display_name":"Fable"},"surface":null},"is_active":false}],"spend":{"used":{"amount_minor":0,"currency":"USD","exponent":2},"limit":null,"percent":0,"severity":"normal","enabled":false}},"behaviors":{}}}}"#;
    const CLOCK: i64 = 1_791_158_400;

    fn at(text: &str) -> Option<i64> {
        issues::parse_time(text)
    }

    fn report(line: &str) -> Report {
        answer(line).expect("the usage answer").expect("a report")
    }

    #[test]
    fn a_recorded_answer_gives_one_window_per_limit() {
        let expected = Report {
            plan: Some("team".into()),
            limited: true,
            windows: vec![
                Window {
                    label: "session (5h)".into(),
                    percent: 7,
                    severity: Severity::Normal,
                    resets_at: at("2026-10-04T13:59:59"),
                },
                Window {
                    label: "week".into(),
                    percent: 74,
                    severity: Severity::Normal,
                    resets_at: at("2026-10-04T15:59:59"),
                },
                Window {
                    label: "week · Fable".into(),
                    percent: 0,
                    severity: Severity::Normal,
                    resets_at: at("2026-10-04T16:00:00"),
                },
            ],
            extra: None,
        };
        assert_eq!(report(RECORDED), expected);
    }

    #[test]
    fn an_account_without_rate_limits_has_no_windows() {
        let body = json!({"subscription_type": null, "rate_limits_available": false, "rate_limits": null});
        assert_eq!(parse(body).expect("a report"), Report::default());
    }

    #[test]
    fn unknown_fields_and_kinds_are_tolerated() {
        let body = json!({
            "subscription_type": "max",
            "rate_limits_available": true,
            "something_new": [1, 2],
            "rate_limits": {
                "limits": [{"kind": "daily_burst", "percent": 12.6, "severity": "elevated", "brand_new": true}],
                "spend": {"enabled": true, "used": {"amount_minor": 1234, "currency": "USD", "exponent": 2},
                          "limit": {"amount_minor": 5000, "currency": "USD", "exponent": 2}}
            }
        });
        let expected = Report {
            plan: Some("max".into()),
            limited: true,
            windows: vec![Window {
                label: "daily burst".into(),
                percent: 13,
                severity: Severity::Normal,
                resets_at: None,
            }],
            extra: Some("extra usage: 12.34 USD of 50.00 USD".into()),
        };
        assert_eq!(parse(body).expect("a report"), expected);
    }

    #[test]
    fn without_a_limits_list_the_five_hour_and_weekly_fields_are_used() {
        let body = json!({"rate_limits_available": true, "rate_limits": {
            "five_hour": {"utilization": 95.0, "resets_at": null},
            "seven_day": {"utilization": 80.0, "resets_at": "2026-10-04T16:00:00Z"}
        }});
        let found: Vec<(String, Severity)> =
            parse(body).expect("a report").windows.into_iter().map(|w| (w.label, w.severity)).collect();
        assert_eq!(found, [("session (5h)".into(), Severity::Critical), ("week".into(), Severity::Warning)]);
    }

    #[rstest]
    #[case::other_request(
        r#"{"type":"control_response","response":{"subtype":"success","request_id":"initialize","response":{}}}"#
    )]
    #[case::other_message(r#"{"type":"system","subtype":"init"}"#)]
    #[case::not_json("warming up")]
    fn other_lines_are_skipped(#[case] line: &str) {
        assert!(answer(line).is_none());
    }

    #[test]
    fn an_error_response_carries_its_message() {
        let line = r#"{"type":"control_response","response":{"subtype":"error","request_id":"usage","error":"not logged in"}}"#;
        let error = answer(line).expect("the usage answer").expect_err("an error");
        assert_eq!(error.to_string(), "not logged in");
    }

    #[rstest]
    #[case::minutes(13 * 60, "resets in 13m")]
    #[case::rounds_up(30, "resets in 1m")]
    #[case::hours(2 * 3_600 + 13 * 60, "resets in 2h 13m")]
    #[case::days(3 * 86_400 + 4 * 3_600, "resets in 3d 4h")]
    #[case::past(-5, "resets in 0m")]
    fn reset_times_are_relative(#[case] secs: i64, #[case] expected: &str) {
        assert_eq!(until(secs), expected);
    }

    #[test]
    fn the_view_shows_loading_then_the_windows_and_their_age() {
        let mut state = State::default();
        let now = Instant::now();
        assert!(state.start());
        assert!(!state.start());
        let loading = state.view(now, CLOCK);
        assert_eq!((loading.windows.len(), loading.note), (0, Some(ui::Note::Busy("loading…"))));

        state.answered(Ok(report(RECORDED)), now);
        let view = state.view(now + Duration::from_secs(120), at("2026-10-04T11:46:59").expect("a time"));
        let rows: Vec<(String, u16, String)> =
            view.windows.into_iter().map(|w| (w.label, w.percent, w.resets)).collect();
        assert_eq!(
            (view.title.as_str(), rows, view.age.as_str(), view.note),
            (
                "Claude Code · team plan",
                vec![
                    ("session (5h)".into(), 7, "resets in 2h 13m".into()),
                    ("week".into(), 74, "resets in 4h 13m".into()),
                    ("week · Fable".into(), 0, "resets in 4h 14m".into()),
                ],
                "updated 2m ago",
                None,
            )
        );
    }

    #[test]
    fn a_failure_keeps_the_last_answer() {
        let mut state = State::default();
        let now = Instant::now();
        state.start();
        state.answered(Ok(report(RECORDED)), now);
        state.start();
        state.answered(Err(Error::Usage("claude did not answer in time".into())), now);
        let view = state.view(now, CLOCK);
        assert_eq!(
            (view.windows.len(), view.note),
            (3, Some(ui::Note::Error("usage unavailable: claude did not answer in time".into())))
        );
    }

    fn fake_claude(script: &str) -> (TempDir, String) {
        let dir = TempDir::new();
        let path = dir.path().join("claude");
        write_executable(&path, &format!("#!/bin/sh\n{script}\n"));
        let command = path.display().to_string();
        (dir, command)
    }

    #[test]
    fn the_probe_answers_from_the_control_protocol() {
        let script =
            format!("read -r init\nread -r usage\necho '{{\"type\":\"system\"}}'\necho '{RECORDED}'\nexec sleep 30");
        let (_dir, command) = fake_claude(&script);
        assert_eq!(probe(&command, TIMEOUT).expect("a report"), report(RECORDED));
    }

    #[rstest]
    #[case::hangs("exec sleep 30", Duration::from_millis(300), "claude did not answer in time")]
    #[case::exits("exit 1", TIMEOUT, "claude exited without answering")]
    fn a_probe_that_gets_no_answer_fails(#[case] script: &str, #[case] timeout: Duration, #[case] expected: &str) {
        let (_dir, command) = fake_claude(script);
        let error = probe(&command, timeout).expect_err("no report");
        assert_eq!(error.to_string(), expected);
    }

    #[test]
    fn a_missing_claude_fails() {
        let error = probe("/nonexistent/claude", TIMEOUT).expect_err("no report");
        assert!(error.to_string().starts_with("could not run /nonexistent/claude"), "{error}");
    }

    #[test]
    fn the_probe_drops_claude_session_variables() {
        let cmd = command("claude");
        let removed: Vec<_> = cmd.get_envs().filter(|(_, v)| v.is_none()).map(|(k, _)| k.to_owned()).collect();
        let mut expected: Vec<_> = activity::CLAUDE_SESSION_ENV.map(std::ffi::OsString::from).to_vec();
        expected.sort();
        assert_eq!(removed, expected);
    }
}
