use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};

use crate::changes::diff::{Diff, File, Kind, Line, Segments, Status};
use crate::changes::{GapLine, Mode, Tints};

pub const DEFAULT_WIDTH: u16 = 64;
pub const MIN_WIDTH: u16 = 36;
const HEADER_ROWS: u16 = 3;
const FOOTER_ROWS: u16 = 2;
const BAR_CELLS: usize = 8;
const VIEWED_WIDTH: u16 = 3;
const LOADING: &str = "reading changes…";
const FOLD_ALL: &str = "fold all";
const UNFOLD_ALL: &str = "unfold all";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Open,
    Ask,
    Copy,
}

impl Action {
    pub const ALL: [Self; 3] = [Self::Open, Self::Ask, Self::Copy];

    fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Ask => "ask agent",
            Self::Copy => "copy",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    Loading,
    Failed(String),
    Ready(Arc<Diff>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub mode: Mode,
    pub base: Option<String>,
    pub body: Body,
    pub folded: Vec<bool>,
    pub viewed: Vec<bool>,
    pub gaps: HashMap<(usize, usize), Arc<Vec<GapLine>>>,
    pub scroll: usize,
    pub live: bool,
    pub light: bool,
    pub tints: Tints,
}

impl View {
    fn diff(&self) -> Option<&Diff> {
        match &self.body {
            Body::Ready(diff) => Some(diff),
            _ => None,
        }
    }

    fn foldable(&self) -> bool {
        self.diff().is_some_and(|d| d.files.iter().any(|f| f.fold.shows_lines()))
    }

    fn all_folded(&self) -> bool {
        self.diff()
            .is_some_and(|d| d.files.iter().zip(&self.folded).all(|(f, folded)| *folded || !f.fold.shows_lines()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    File(usize),
    Gap(usize, usize),
    GapLine(usize, usize, usize),
    Hunk(usize, usize),
    Line(usize, usize, usize),
    Spacer,
}

fn gap_size(file: &File, hunk: usize) -> u32 {
    let Some(before) = hunk.checked_sub(1).and_then(|h| file.hunks.get(h)) else { return 0 };
    file.hunks.get(hunk).map_or(0, |after| after.new_start.saturating_sub(before.new_end()))
}

pub fn rows(view: &View) -> Vec<Row> {
    let Some(diff) = view.diff() else { return Vec::new() };
    let mut rows = Vec::new();
    for (i, file) in diff.files.iter().enumerate() {
        rows.push(Row::File(i));
        if view.folded.get(i).copied().unwrap_or(true) {
            continue;
        }
        for (h, hunk) in file.hunks.iter().enumerate() {
            match view.gaps.get(&(i, h)) {
                Some(lines) => rows.extend((0..lines.len()).map(|k| Row::GapLine(i, h, k))),
                None if gap_size(file, h) > 0 => rows.push(Row::Gap(i, h)),
                None => {}
            }
            rows.push(Row::Hunk(i, h));
            rows.extend((0..hunk.lines.len()).map(|l| Row::Line(i, h, l)));
        }
        rows.push(Row::Spacer);
    }
    rows
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parts {
    pub tabs: Rect,
    pub summary: Rect,
    pub body: Rect,
    pub separator: Rect,
    pub footer: Rect,
}

pub fn parts(area: Rect) -> Parts {
    let inner = Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area };
    let row = |y: u16| Rect::new(inner.x, y, inner.width, 1).intersection(area);
    let top = area.y.saturating_add(HEADER_ROWS);
    let separator = area.bottom().saturating_sub(FOOTER_ROWS).max(top);
    Parts {
        tabs: row(area.y),
        summary: row(area.y.saturating_add(1)),
        body: Rect::new(area.x, top, area.width, separator.saturating_sub(top)).intersection(area),
        separator: row(separator),
        footer: row(separator.saturating_add(1)),
    }
}

fn width(text: &str) -> u16 {
    u16::try_from(text.chars().count()).unwrap_or(u16::MAX)
}

pub fn tabs(area: Rect) -> Vec<(Mode, Rect)> {
    let row = parts(area).tabs;
    let mut x = row.x;
    Mode::ALL
        .into_iter()
        .map(|mode| {
            let w = width(mode.label()) + 2;
            let r = Rect::new(x, row.y, w, 1).intersection(row);
            x = x.saturating_add(w + 1);
            (mode, r)
        })
        .collect()
}

pub fn close(area: Rect) -> Rect {
    let row = parts(area).tabs;
    Rect::new(row.right().saturating_sub(3), row.y, 3, 1).intersection(row)
}

fn base_label(view: &View) -> String {
    format!("vs {} ▾", view.base.as_deref().unwrap_or("…"))
}

pub fn base(area: Rect, view: &View) -> Rect {
    if view.mode == Mode::Uncommitted {
        return Rect::default();
    }
    let row = parts(area).summary;
    let w = width(&base_label(view)) + 2;
    Rect::new(row.right().saturating_sub(w), row.y, w, 1).intersection(row)
}

fn fold_label(view: &View) -> &'static str {
    if view.all_folded() { UNFOLD_ALL } else { FOLD_ALL }
}

pub fn fold_all(area: Rect, view: &View) -> Rect {
    if !view.foldable() {
        return Rect::default();
    }
    let row = parts(area).footer;
    Rect::new(row.x, row.y, width(fold_label(view)) + 2, 1).intersection(row)
}

pub fn max_scroll(area: Rect, view: &View) -> usize {
    rows(view).len().saturating_sub(usize::from(parts(area).body.height))
}

fn visible(area: Rect, view: &View) -> Vec<(Row, Rect)> {
    let body = parts(area).body;
    let rows = rows(view);
    let first = view.scroll.min(rows.len().saturating_sub(usize::from(body.height)));
    rows.into_iter()
        .skip(first)
        .zip(body.y..body.bottom())
        .map(|(row, y)| (row, Rect::new(body.x, y, body.width, 1)))
        .collect()
}

fn actions(row: Rect) -> Vec<(Action, Rect)> {
    let mut right = row.right().saturating_sub(1);
    let mut out: Vec<(Action, Rect)> = Action::ALL
        .into_iter()
        .rev()
        .map(|action| {
            let w = width(action.label()) + 2;
            let r = Rect::new(right.saturating_sub(w), row.y, w, 1).intersection(row);
            right = right.saturating_sub(w + 1);
            (action, r)
        })
        .collect();
    out.reverse();
    out
}

fn viewed_cell(row: Rect) -> Rect {
    Rect::new(row.right().saturating_sub(VIEWED_WIDTH), row.y, VIEWED_WIDTH.min(row.width), 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Mode(Mode),
    Close,
    Base,
    FoldAll,
    File(usize),
    Viewed(usize),
    Gap(usize, usize),
    Action(usize, usize, Action),
}

pub fn hit(area: Rect, view: &View, pos: Position) -> Option<Hit> {
    if close(area).contains(pos) {
        return Some(Hit::Close);
    }
    if let Some((mode, _)) = tabs(area).into_iter().find(|(_, r)| r.contains(pos)) {
        return Some(Hit::Mode(mode));
    }
    if base(area, view).contains(pos) {
        return Some(Hit::Base);
    }
    if fold_all(area, view).contains(pos) {
        return Some(Hit::FoldAll);
    }
    let (row, r) = visible(area, view).into_iter().find(|(_, r)| r.contains(pos))?;
    match row {
        Row::File(i) if viewed_cell(r).contains(pos) => Some(Hit::Viewed(i)),
        Row::File(i) => Some(Hit::File(i)),
        Row::Gap(i, h) => Some(Hit::Gap(i, h)),
        Row::Hunk(i, h) => actions(r).into_iter().find(|(_, a)| a.contains(pos)).map(|(a, _)| Hit::Action(i, h, a)),
        _ => None,
    }
}

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn hovered(hover: Option<Position>, r: Rect) -> bool {
    hover.is_some_and(|p| r.contains(p))
}

fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style, end: u16) -> u16 {
    if x >= end {
        return x;
    }
    buf.set_stringn(x, y, text, usize::from(end - x), style).0
}

fn fill(buf: &mut Buffer, r: Rect, bg: Color) {
    buf.set_style(r, Style::default().bg(bg));
}

fn cut(text: &str, room: usize) -> String {
    crate::ui::truncate_right(text, room)
}

pub fn draw(f: &mut Frame, area: Rect, view: &View, hover: Option<Position>) {
    let buf = f.buffer_mut();
    let p = parts(area);
    draw_tabs(buf, area, view, hover);
    draw_summary(buf, area, view, hover);
    let hovered_hunk =
        visible(area, view).into_iter().find(|(_, r)| hovered(hover, *r)).and_then(|(row, _)| match row {
            Row::Hunk(i, h) | Row::Line(i, h, _) => Some((i, h)),
            _ => None,
        });
    if let Some(diff) = view.diff() {
        if diff.files.is_empty() {
            let text = match (view.mode, view.base.as_deref()) {
                (Mode::Uncommitted, _) | (_, None) => "no changes".to_string(),
                (Mode::Commits, Some(base)) => format!("no commits since {base}"),
                (Mode::All, Some(base)) => format!("nothing changed since {base}"),
            };
            put(buf, p.body.x + 1, p.body.y, &text, dim(), p.body.right());
        }
        for (row, r) in visible(area, view) {
            draw_row(buf, view, diff, row, r, hover, hovered_hunk);
        }
    }
    let line = "─".repeat(usize::from(p.separator.width));
    put(buf, p.separator.x, p.separator.y, &line, dim(), p.separator.right());
    let fold = fold_all(area, view);
    if !fold.is_empty() {
        let style = if hovered(hover, fold) { Style::default().fg(Color::Black).bg(Color::Cyan) } else { dim() };
        put(buf, fold.x, fold.y, &format!(" {} ", fold_label(view)), style, fold.right());
    }
    if let Some(diff) = view.diff().filter(|d| !d.files.is_empty()) {
        let viewed = view.viewed.iter().filter(|v| **v).count();
        let text = format!("{viewed} of {} viewed", diff.files.len());
        let x = p.footer.right().saturating_sub(width(&text));
        put(buf, x.max(fold.right() + 1), p.footer.y, &text, dim(), p.footer.right());
    }
}

fn draw_tabs(buf: &mut Buffer, area: Rect, view: &View, hover: Option<Position>) {
    for (mode, r) in tabs(area) {
        let style = if mode == view.mode {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else if hovered(hover, r) {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        };
        put(buf, r.x, r.y, &format!(" {} ", mode.label()), style, r.right());
    }
    let r = close(area);
    let style = if hovered(hover, r) { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) } else { dim() };
    put(buf, r.x + 1, r.y, "×", style, r.right());
}

fn draw_summary(buf: &mut Buffer, area: Rect, view: &View, hover: Option<Position>) {
    let row = parts(area).summary;
    let selector = base(area, view);
    let end = if selector.is_empty() { row.right() } else { selector.x.saturating_sub(1) };
    match &view.body {
        Body::Loading => {
            put(buf, row.x, row.y, LOADING, dim(), end);
        }
        Body::Failed(error) => {
            let text = cut(error, usize::from(end.saturating_sub(row.x)));
            put(buf, row.x, row.y, &text, Style::default().fg(Color::Red), end);
        }
        Body::Ready(diff) => {
            let n = diff.files.len();
            let mut x = put(buf, row.x, row.y, &n.to_string(), Style::default().fg(Color::White).bold(), end);
            x = put(buf, x, row.y, if n == 1 { " file" } else { " files" }, Style::default().fg(Color::Gray), end);
            let (added, removed) = (diff.added(), diff.removed());
            if added + removed > 0 {
                x = put(buf, x + 2, row.y, &format!("+{added}"), Style::default().fg(Color::Green).bold(), end);
                x = put(buf, x + 1, row.y, &format!("−{removed}"), Style::default().fg(Color::Red).bold(), end);
                let green = (added * BAR_CELLS).div_ceil(added + removed).min(BAR_CELLS);
                x = put(buf, x + 2, row.y, &"▃".repeat(green), Style::default().fg(Color::Green), end);
                put(buf, x, row.y, &"▃".repeat(BAR_CELLS - green), Style::default().fg(Color::Red), end);
            }
            if selector.is_empty() && view.live {
                let x = row.right().saturating_sub(6);
                let x = put(buf, x, row.y, "●", Style::default().fg(Color::Green), row.right());
                put(buf, x, row.y, " live", dim(), row.right());
            }
        }
    }
    if !selector.is_empty() {
        let style =
            if hovered(hover, selector) { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::Gray) };
        put(buf, selector.x, selector.y, &format!(" {} ", base_label(view)), style, selector.right());
    }
}

fn status_style(status: Status) -> Style {
    let colour = match status {
        Status::Modified => Color::Yellow,
        Status::Added | Status::Untracked => Color::Green,
        Status::Deleted => Color::Red,
        Status::Renamed => Color::Cyan,
    };
    Style::default().fg(colour).add_modifier(Modifier::BOLD)
}

fn number_width(file: &File) -> usize {
    let last = file.hunks.iter().map(|h| h.old_end().max(h.new_end())).max().unwrap_or(0);
    last.to_string().len().max(3)
}

fn draw_row(
    buf: &mut Buffer,
    view: &View,
    diff: &Diff,
    row: Row,
    r: Rect,
    hover: Option<Position>,
    hovered_hunk: Option<(usize, usize)>,
) {
    match row {
        Row::File(i) => draw_file(buf, view, &diff.files[i], i, r, hover),
        Row::Gap(i, h) => {
            let file = &diff.files[i];
            let n = gap_size(file, h);
            let digits = u16::try_from(number_width(file)).unwrap_or(3);
            let style = if hovered(hover, r) { Style::default().fg(Color::Cyan) } else { dim() };
            put(buf, r.x + 3 + digits * 2, r.y, "↕", Style::default().fg(Color::Cyan), r.right());
            let label = if n == 1 { "1 unchanged line".to_string() } else { format!("{n} unchanged lines") };
            put(buf, r.x + 5 + digits * 2, r.y, &label, style, r.right());
        }
        Row::GapLine(i, h, k) => {
            let Some(line) = view.gaps.get(&(i, h)).and_then(|g| g.get(k)) else { return };
            let file = &diff.files[i];
            let code = Code { kind: Kind::Context, old: Some(line.old), new: Some(line.new), text: &line.text };
            draw_code(buf, view, r, number_width(file), code, line.syntax.as_ref(), &[]);
        }
        Row::Hunk(i, h) => {
            let hunk = &diff.files[i].hunks[h];
            let end = if hovered_hunk == Some((i, h)) {
                actions(r).first().map_or(r.right(), |(_, a)| a.x)
            } else {
                r.right()
            };
            let mut x = r.x + 1;
            if !hunk.context.is_empty() {
                x = put(buf, x, r.y, "┄┄ ", dim(), end);
                let room = usize::from(end.saturating_sub(x + 2));
                let context = cut(&hunk.context, room);
                x = put(buf, x, r.y, &context, Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC), end);
                x += 1;
            }
            let rest = usize::from(end.saturating_sub(x + 1));
            put(buf, x, r.y, &"┄".repeat(rest), dim(), end);
            if hovered_hunk == Some((i, h)) {
                for (action, a) in actions(r) {
                    let style = if hovered(hover, a) {
                        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Cyan)
                    };
                    put(buf, a.x, a.y, &format!(" {} ", action.label()), style, a.right());
                }
            }
        }
        Row::Line(i, h, l) => {
            let file = &diff.files[i];
            let line: &Line = &file.hunks[h].lines[l];
            let code = Code { kind: line.kind, old: line.old, new: line.new, text: &line.text };
            draw_code(buf, view, r, number_width(file), code, line.syntax.as_ref(), &line.emphasis);
        }
        Row::Spacer => {}
    }
}

fn draw_file(buf: &mut Buffer, view: &View, file: &File, i: usize, r: Rect, hover: Option<Position>) {
    let open = !view.folded.get(i).copied().unwrap_or(true);
    let viewed = view.viewed.get(i).copied().unwrap_or(false);
    if open {
        fill(buf, r, if view.light { super::LIGHT_SURFACE } else { super::DARK_SURFACE });
        put(buf, r.x + 1, r.y, "▌", Style::default().fg(Color::Cyan), r.right());
    }
    let muted = viewed || !matches!(file.fold, crate::changes::diff::Fold::Open);
    if file.fold.shows_lines() {
        put(buf, r.x + 2, r.y, if open { "▾" } else { "▸" }, dim(), r.right());
    }
    let status = if muted { dim() } else { status_style(file.status) };
    put(buf, r.x + 4, r.y, file.status.letter(), status, r.right());

    let check = viewed_cell(r);
    if viewed {
        put(buf, check.x + 1, r.y, "✓", Style::default().fg(Color::Green), r.right());
    } else if hovered(hover, r) {
        let style = if hovered(hover, check) { Style::default().fg(Color::Green) } else { dim() };
        put(buf, check.x + 1, r.y, "✓", style, r.right());
    }
    let mut right: Vec<(String, Style)> = Vec::new();
    let tag = file.fold.tag().or((file.status == Status::Untracked).then_some("new"));
    if let Some(tag) = tag {
        right.push((tag.to_string(), dim().add_modifier(Modifier::ITALIC)));
    }
    if file.added > 0 {
        right.push((format!("+{}", file.added), if muted { dim() } else { Style::default().fg(Color::Green) }));
    }
    if file.removed > 0 {
        right.push((format!("−{}", file.removed), if muted { dim() } else { Style::default().fg(Color::Red) }));
    }
    let right_width: u16 = right.iter().map(|(t, _)| width(t) + 1).sum();
    let mut x = check.x.saturating_sub(right_width);
    let path_end = x.saturating_sub(1);
    for (text, style) in &right {
        x = put(buf, x, r.y, text, *style, check.x) + 1;
    }

    let path = match &file.old_path {
        Some(old) => format!("{old} → {}", file.path),
        None => file.path.clone(),
    };
    let start = r.x + 6;
    let room = usize::from(path_end.saturating_sub(start));
    let shown = crate::ui::truncate_left(&path, room);
    let split = shown.rfind('/').map_or(0, |i| i + 1);
    let (folder, name) = shown.split_at(split);
    let x = put(buf, start, r.y, folder, dim(), path_end);
    let name_style = if muted { dim() } else { Style::default().fg(Color::White).add_modifier(Modifier::BOLD) };
    put(buf, x, r.y, name, name_style, path_end);
}

#[derive(Clone, Copy)]
struct Code<'a> {
    kind: Kind,
    old: Option<u32>,
    new: Option<u32>,
    text: &'a str,
}

fn draw_code(
    buf: &mut Buffer,
    view: &View,
    r: Rect,
    digits: usize,
    code: Code,
    syntax: Option<&Segments>,
    emphasis: &[Range<usize>],
) {
    let (tint, word, bar) = match code.kind {
        Kind::Context => (None, None, None),
        Kind::Removed => (view.tints.removed, Some(view.tints.removed_word), Some(Color::Red)),
        Kind::Added => (view.tints.added, Some(view.tints.added_word), Some(Color::Green)),
    };
    if let Some(tint) = tint {
        fill(buf, r, tint);
    }
    let number = |n: Option<u32>| n.map_or_else(|| " ".repeat(digits), |n| format!("{n:>digits$}"));
    let numbers = match (tint, bar) {
        (None, Some(colour)) => Style::default().fg(colour),
        _ => dim(),
    };
    let mut x = put(buf, r.x + 1, r.y, &number(code.old), numbers, r.right());
    x = put(buf, x + 1, r.y, &number(code.new), numbers, r.right());
    if let Some(bar) = bar {
        put(buf, x + 1, r.y, "▎", Style::default().fg(bar), r.right());
    }
    let start = x + 3;
    let end = r.right().saturating_sub(1);
    let room = usize::from(end.saturating_sub(start));
    let plain = [(code.text.to_string(), Style::default())];
    let segments: &[(String, Style)] = syntax.map_or(&plain, Vec::as_slice);
    let total: usize = segments.iter().map(|(t, _)| t.chars().count()).sum();
    let limit = if total > room { room.saturating_sub(1) } else { room };
    let mut x = start;
    let mut index = 0;
    for (text, style) in segments {
        let mut run = String::new();
        let mut run_word = None;
        for c in text.chars() {
            if index >= limit {
                break;
            }
            let strong = emphasis.iter().any(|e| e.contains(&index));
            if run_word.is_some_and(|w| w != strong) {
                x = put(buf, x, r.y, &run, paint(*style, tint, word, run_word == Some(true)), end);
                run.clear();
            }
            run_word = Some(strong);
            run.push(c);
            index += 1;
        }
        if !run.is_empty() {
            x = put(buf, x, r.y, &run, paint(*style, tint, word, run_word == Some(true)), end);
        }
    }
    if total > room {
        put(buf, x, r.y, "…", dim(), end + 1);
    }
}

fn paint(style: Style, tint: Option<Color>, word: Option<Color>, strong: bool) -> Style {
    let bg = if strong { word } else { tint };
    let style = Style { bg: None, ..style };
    bg.map_or(style, |bg| style.bg(bg))
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::changes::diff;

    const PATCH: &str = "diff --git a/src/returns.rs b/src/returns.rs
--- a/src/returns.rs
+++ b/src/returns.rs
@@ -9,4 +9,4 @@ impl Address
 /// The first line, when the address has one.
-pub fn street(parts: &[String]) -> &str {
-    parts.first().unwrap()
+pub fn street(parts: &[String]) -> Option<&str> {
+    parts.first().map(String::as_str)
 }
@@ -44,2 +44,6 @@ mod tests
     }
+
+    #[test]
+    fn empty_address_is_rejected() {}
 }
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -1 +1 @@
-a
+b
";

    const TINTS: Tints = Tints {
        removed: Some(Color::Indexed(52)),
        added: Some(Color::Indexed(22)),
        removed_word: Color::Indexed(88),
        added_word: Color::Indexed(28),
    };

    fn sample() -> View {
        let mut files = diff::parse(PATCH);
        files.iter_mut().for_each(diff::finish);
        let mut untracked = diff::untracked("tests/returns.rs", 4, Some(b"one\n"));
        diff::finish(&mut untracked);
        files.push(untracked);
        let diff = Diff { files: files.into_iter().map(Arc::new).collect() };
        View {
            mode: Mode::Uncommitted,
            base: None,
            folded: vec![false, true, true],
            viewed: vec![false, false, false],
            body: Body::Ready(Arc::new(diff)),
            gaps: HashMap::new(),
            scroll: 0,
            live: true,
            light: false,
            tints: TINTS,
        }
    }

    const AREA: Rect = Rect { x: 0, y: 0, width: 60, height: 24 };

    fn render(view: &View, hover: Option<Position>) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).expect("terminal");
        terminal.draw(|f| draw(f, AREA, view, hover)).expect("draw");
        terminal
    }

    fn row_of(view: &View, wanted: Row) -> Rect {
        visible(AREA, view).into_iter().find(|(row, _)| *row == wanted).map(|(_, r)| r).expect("row is visible")
    }

    #[test]
    fn draws_the_panel() {
        insta::assert_snapshot!(render(&sample(), None).backend());
    }

    #[test]
    fn shows_the_base_in_commits() {
        let view = View { mode: Mode::Commits, base: Some("origin/main".into()), ..sample() };
        insta::assert_snapshot!(render(&view, None).backend());
    }

    #[test]
    fn a_hovered_hunk_shows_its_actions() {
        let view = sample();
        let line = row_of(&view, Row::Line(0, 0, 2));
        let terminal = render(&view, Some(Position::new(10, line.y)));
        let header = row_of(&view, Row::Hunk(0, 0));
        let text: String =
            (0..AREA.width).map(|x| terminal.backend().buffer()[(x, header.y)].symbol().to_string()).collect();
        assert!(text.contains(" open   ask agent   copy "), "{text}");
    }

    #[test]
    fn changed_words_get_a_stronger_tint() {
        let view = sample();
        let terminal = render(&view, None);
        let r = row_of(&view, Row::Line(0, 0, 3));
        let buffer = terminal.backend().buffer();
        let x = (0..AREA.width).find(|&x| buffer[(x, r.y)].symbol() == "O").expect("Option on screen");
        assert_eq!(
            (Some(buffer[(x, r.y)].bg), buffer[(2, r.y)].bg),
            (Some(TINTS.added_word), TINTS.added.expect("tint"))
        );
    }

    #[test]
    fn rows_skip_folded_files() {
        let rows = rows(&sample());
        assert!(rows.contains(&Row::File(1)) && !rows.iter().any(|r| matches!(r, Row::Line(1, ..))));
    }

    #[test]
    fn a_gap_between_hunks_can_be_opened() {
        let view = sample();
        assert!(rows(&view).contains(&Row::Gap(0, 1)));
        let gap = row_of(&view, Row::Gap(0, 1));
        assert_eq!(hit(AREA, &view, Position::new(20, gap.y)), Some(Hit::Gap(0, 1)));
    }

    #[rstest::rstest]
    #[case::uncommitted(4, Hit::Mode(Mode::Uncommitted))]
    #[case::commits(17, Hit::Mode(Mode::Commits))]
    #[case::all(26, Hit::Mode(Mode::All))]
    #[case::close(58, Hit::Close)]
    fn header_clicks(#[case] x: u16, #[case] expected: Hit) {
        assert_eq!(hit(AREA, &sample(), Position::new(x, 0)), Some(expected));
    }

    #[test]
    fn clicks_on_a_file_row() {
        let view = sample();
        let r = row_of(&view, Row::File(1));
        assert_eq!(
            (hit(AREA, &view, Position::new(10, r.y)), hit(AREA, &view, Position::new(r.right() - 2, r.y))),
            (Some(Hit::File(1)), Some(Hit::Viewed(1)))
        );
    }

    #[test]
    fn hunk_actions_are_clickable() {
        let view = sample();
        let r = row_of(&view, Row::Hunk(0, 0));
        let (_, ask) = actions(r)[1];
        assert_eq!(hit(AREA, &view, Position::new(ask.x + 1, r.y)), Some(Hit::Action(0, 0, Action::Ask)));
    }

    #[test]
    fn the_base_selector_is_clickable_in_commits() {
        let view = View { mode: Mode::Commits, base: Some("main".into()), ..sample() };
        let r = base(AREA, &view);
        assert_eq!(hit(AREA, &view, Position::new(r.x + 1, r.y)), Some(Hit::Base));
    }

    #[test]
    fn scrolling_moves_the_rows() {
        let short = Rect { height: 8, ..AREA };
        let first = visible(short, &View { scroll: 2, ..sample() })[0].0;
        assert_eq!(first, rows(&sample())[2]);
    }

    #[test]
    fn shows_an_error() {
        let view = View { body: Body::Failed("cannot compare with nope".into()), ..sample() };
        insta::assert_snapshot!(render(&view, None).backend());
    }
}
