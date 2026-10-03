use std::time::{Duration, Instant};

use crate::issues::one_line;

const SHELL_QUIET: Duration = Duration::from_millis(300);
const SHELL_LATEST: Duration = Duration::from_secs(5);
const AGENT_QUIET: Duration = Duration::from_secs(1);
const AGENT_GONE: Duration = Duration::from_secs(3);
const AGENT_LATEST: Duration = Duration::from_secs(30);
const SUBMIT_QUIET: Duration = Duration::from_millis(300);
const MAX_TRUSTS: u8 = 4;
const MARKERS: [&str; 5] = ["❯", "›", ">", "▸", "→"];
const MENU_REACH: usize = 6;
const PASTE_START: &str = "\x1b[200~";
const PASTE_END: &str = "\x1b[201~";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    pub command: String,
    pub prompt: String,
    pub submit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Shell,
    Agent,
    Submit,
}

#[derive(Debug)]
pub struct Launch {
    pub term: u64,
    spec: Spec,
    stage: Stage,
    since: Instant,
    output: Option<Instant>,
    trusts: u8,
    trusted_screen: Option<String>,
}

pub struct Seen<'a> {
    pub shell_in_foreground: bool,
    pub bracketed_paste: bool,
    pub application_cursor: bool,
    pub screen: &'a mut dyn FnMut() -> String,
    pub trust_prompt: &'a dyn Fn(&str) -> bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Wait,
    Write(Vec<u8>),
    Done(Vec<u8>),
    Abandon,
}

impl Launch {
    pub fn new(term: u64, spec: Spec, now: Instant) -> Self {
        Self { term, spec, stage: Stage::Shell, since: now, output: None, trusts: 0, trusted_screen: None }
    }

    pub fn output(&mut self, now: Instant) {
        self.output = Some(now);
    }

    fn quiet(&self, now: Instant) -> Duration {
        now.duration_since(self.output.unwrap_or(self.since))
    }

    fn next(&mut self, stage: Stage, now: Instant) {
        self.stage = stage;
        self.since = now;
        self.output = None;
    }

    pub fn step(&mut self, now: Instant, seen: &mut Seen) -> Step {
        match self.stage {
            Stage::Shell => {
                let ready = self.output.is_some_and(|at| now.duration_since(at) >= SHELL_QUIET)
                    || now.duration_since(self.since) >= SHELL_LATEST;
                if !ready {
                    return Step::Wait;
                }
                self.next(Stage::Agent, now);
                Step::Write(format!("{}\r", self.spec.command).into_bytes())
            }
            Stage::Agent if seen.shell_in_foreground => {
                if now.duration_since(self.since) >= AGENT_GONE && self.quiet(now) >= AGENT_GONE {
                    Step::Abandon
                } else {
                    Step::Wait
                }
            }
            Stage::Agent => {
                let ready = self.quiet(now) >= AGENT_QUIET || now.duration_since(self.since) >= AGENT_LATEST;
                if !ready {
                    return Step::Wait;
                }
                let screen = (seen.screen)();
                if self.trusts < MAX_TRUSTS
                    && self.trusted_screen.as_ref() != Some(&screen)
                    && (seen.trust_prompt)(&screen)
                {
                    self.trusts += 1;
                    let keys = answer_keys(&screen, seen.application_cursor);
                    self.trusted_screen = Some(screen);
                    self.next(Stage::Agent, now);
                    return Step::Write(keys);
                }
                let text = if self.spec.submit && seen.bracketed_paste {
                    self.spec.prompt.clone()
                } else {
                    one_line(&self.spec.prompt)
                };
                let bytes = if seen.bracketed_paste { format!("{PASTE_START}{text}{PASTE_END}") } else { text };
                if self.spec.submit {
                    self.next(Stage::Submit, now);
                    Step::Write(bytes.into_bytes())
                } else {
                    Step::Done(bytes.into_bytes())
                }
            }
            Stage::Submit => {
                if self.quiet(now) >= SUBMIT_QUIET {
                    Step::Done(b"\r".to_vec())
                } else {
                    Step::Wait
                }
            }
        }
    }
}

fn option_text(line: &str) -> &str {
    line.trim_start_matches(|c: char| c.is_whitespace() || "│┃║|".contains(c))
}

pub fn answer_keys(screen: &str, application_cursor: bool) -> Vec<u8> {
    let lines: Vec<&str> = screen.lines().map(option_text).collect();
    let Some(marked) = lines.iter().position(|l| MARKERS.iter().any(|m| l.starts_with(m))) else {
        return b"\r".to_vec();
    };
    let near = marked.saturating_sub(MENU_REACH)..(marked + MENU_REACH + 1).min(lines.len());
    let yes = near
        .filter(|&i| lines[i].to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|word| word == "yes"))
        .min_by_key(|&i| i.abs_diff(marked));
    match yes {
        Some(target) if target != marked => {
            let key = match (target > marked, application_cursor) {
                (true, false) => "\x1b[B",
                (true, true) => "\x1bOB",
                (false, false) => "\x1b[A",
                (false, true) => "\x1bOA",
            };
            key.repeat(target.abs_diff(marked)).into_bytes()
        }
        _ => b"\r".to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn spec(submit: bool) -> Spec {
        Spec { command: "claude --permission-mode plan".into(), prompt: "https://x.dev/7\nplease".into(), submit }
    }

    struct World {
        shell: bool,
        bracketed: bool,
        screen: String,
    }

    fn step(launch: &mut Launch, now: Instant, world: &World) -> Step {
        let screen = world.screen.clone();
        let mut read = move || screen.clone();
        let trust = |s: &str| s.contains("trust");
        let mut seen = Seen {
            shell_in_foreground: world.shell,
            bracketed_paste: world.bracketed,
            application_cursor: false,
            screen: &mut read,
            trust_prompt: &trust,
        };
        launch.step(now, &mut seen)
    }

    fn shell() -> World {
        World { shell: true, bracketed: false, screen: "$ ".into() }
    }

    fn agent(screen: &str) -> World {
        World { shell: false, bracketed: true, screen: screen.into() }
    }

    fn started(submit: bool) -> (Launch, Instant) {
        let t0 = Instant::now();
        let mut launch = Launch::new(1, spec(submit), t0);
        launch.output(t0);
        assert_eq!(
            step(&mut launch, t0 + 300 * MS, &shell()),
            Step::Write(b"claude --permission-mode plan\r".to_vec())
        );
        (launch, t0 + 300 * MS)
    }

    #[test]
    fn waits_for_the_shell_prompt_before_typing_the_agent() {
        let t0 = Instant::now();
        let mut launch = Launch::new(1, spec(false), t0);
        launch.output(t0);
        assert_eq!(step(&mut launch, t0 + 100 * MS, &shell()), Step::Wait);
    }

    #[test]
    fn types_the_agent_anyway_after_a_while() {
        let t0 = Instant::now();
        let mut launch = Launch::new(1, spec(false), t0);
        assert!(matches!(step(&mut launch, t0 + SHELL_LATEST, &shell()), Step::Write(_)));
    }

    #[test]
    fn waits_until_the_agent_has_been_quiet_for_a_second() {
        let (mut launch, t) = started(false);
        launch.output(t + 500 * MS);
        assert_eq!(step(&mut launch, t + 1200 * MS, &agent("> ")), Step::Wait);
    }

    #[test]
    fn accepts_the_trust_prompt_with_enter() {
        let (mut launch, t) = started(false);
        assert_eq!(step(&mut launch, t + AGENT_QUIET, &agent("Do you trust the files?")), Step::Write(b"\r".to_vec()));
    }

    #[test]
    fn stops_accepting_after_a_few_tries() {
        let (mut launch, mut t) = started(false);
        for n in 0..MAX_TRUSTS {
            t += AGENT_QUIET;
            step(&mut launch, t, &agent(&format!("trust {n}")));
        }
        assert!(matches!(step(&mut launch, t + AGENT_QUIET, &agent("trust again")), Step::Done(_)));
    }

    const CLAUDE_TRUST: &str = "│ Quick safety check: Is this a project you created or one you trust?\n\
        │ Claude Code'll be able to read, edit, and execute files here.\n│\n\
        │ ❯ No, exit\n│   Yes, I trust this folder\n│\n│ Enter to confirm · Esc to cancel";

    #[test]
    fn a_menu_that_defaults_to_no_is_moved_to_yes_first() {
        assert_eq!(answer_keys(CLAUDE_TRUST, false), b"\x1b[B");
    }

    #[test]
    fn once_yes_is_marked_enter_answers() {
        let marked = CLAUDE_TRUST.replace("❯ No, exit", "  No, exit").replace("  Yes, I trust", "❯ Yes, I trust");
        assert_eq!(answer_keys(&marked, false), b"\r");
    }

    #[test]
    fn arrows_follow_the_application_cursor_mode() {
        let above = "  1. Yes, proceed\n> 2. No, exit";
        assert_eq!(answer_keys(above, true), b"\x1bOA");
    }

    #[test]
    fn without_a_menu_enter_answers() {
        assert_eq!(answer_keys("Do you trust the files in this folder? (press enter)", false), b"\r");
    }

    #[test]
    fn a_menu_is_answered_in_two_steps() {
        let (mut launch, t) = started(false);
        let first = step(&mut launch, t + AGENT_QUIET, &agent(CLAUDE_TRUST));
        let moved = CLAUDE_TRUST.replace("❯ No, exit", "  No, exit").replace("  Yes, I trust", "❯ Yes, I trust");
        let second = step(&mut launch, t + 2 * AGENT_QUIET, &agent(&moved));
        assert_eq!((first, second), (Step::Write(b"\x1b[B".to_vec()), Step::Write(b"\r".to_vec())));
    }

    #[test]
    fn a_screen_already_answered_is_not_answered_again() {
        let (mut launch, t) = started(false);
        step(&mut launch, t + AGENT_QUIET, &agent("Do you trust the files?"));
        assert!(matches!(step(&mut launch, t + 2 * AGENT_QUIET, &agent("Do you trust the files?")), Step::Done(_)));
    }

    #[test]
    fn pastes_the_prompt_on_one_line_and_leaves_it_there() {
        let (mut launch, t) = started(false);
        assert_eq!(
            step(&mut launch, t + AGENT_QUIET, &agent("> ")),
            Step::Done(b"\x1b[200~https://x.dev/7 please\x1b[201~".to_vec())
        );
    }

    #[test]
    fn without_bracketed_paste_the_prompt_is_typed() {
        let (mut launch, t) = started(false);
        let world = World { bracketed: false, ..agent("> ") };
        assert_eq!(step(&mut launch, t + AGENT_QUIET, &world), Step::Done(b"https://x.dev/7 please".to_vec()));
    }

    #[test]
    fn submit_pastes_as_written_then_presses_enter() {
        let (mut launch, t) = started(true);
        let pasted = step(&mut launch, t + AGENT_QUIET, &agent("> "));
        let enter = step(&mut launch, t + AGENT_QUIET + SUBMIT_QUIET, &agent("> "));
        assert_eq!(
            (pasted, enter),
            (Step::Write(b"\x1b[200~https://x.dev/7\nplease\x1b[201~".to_vec()), Step::Done(b"\r".to_vec()))
        );
    }

    #[test]
    fn gives_up_when_the_agent_never_leaves_the_shell() {
        let (mut launch, t) = started(false);
        assert_eq!(step(&mut launch, t + AGENT_GONE, &shell()), Step::Abandon);
    }

    #[test]
    fn a_busy_agent_gets_the_prompt_after_the_longest_wait() {
        let (mut launch, t) = started(false);
        launch.output(t + AGENT_LATEST);
        assert!(matches!(step(&mut launch, t + AGENT_LATEST, &agent("⠋ thinking")), Step::Done(_)));
    }
}
