use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use serde::{Deserialize, Serialize};

use crate::emulator::Snapshot;
use crate::split::{Dir, Node};

pub const SIDEBAR_WIDTH: u16 = 32;
pub const WORKSPACES_WIDTH: u16 = 26;
pub const PANE_PADDING: u16 = 1;
pub const MIN_COLUMN_WIDTH: u16 = 16;
pub const MIN_PANE_WIDTH: u16 = 20;
pub const COMPACT_WIDTH: u16 = 90;
pub const COMPACT_PITCH: u16 = 3;
const COMPACT_BUTTON_WIDTH: u16 = 7;
const BRAND_HEIGHT: u16 = 2;
const HEADER_HEIGHT: u16 = BRAND_HEIGHT + 3;
const GAP: u16 = 1;
const FORM_WIDTH: u16 = 64;
const FORM_HEIGHT: u16 = 10;
const FORM_PADDING: u16 = 1;
const PICKER_WIDTH: u16 = 72;
const PICKER_HEIGHT: u16 = 24;
const UPDATE_MESSAGE_HEIGHT: u16 = 3;
const ISSUES_WIDTH: u16 = 110;
const ISSUES_HEIGHT: u16 = 30;
const INPUT_PROMPT: &str = "› ";
const SEARCH_ICON: &str = " ⌕ ";
const SEARCH_PLACEHOLDER: &str = "search projects, workspaces, tabs";
const MENU_ICON: &str = "≡";
const BACK_LABEL: &str = "‹ projects";
const CRUMB_SEPARATOR: &str = " › ";
const CANCEL_LABEL: &str = "cancel";
const ISSUES_LABEL: &str = "issues";
const BRAND_COLOR: Color = Color::Indexed(99);
const DARK_SURFACE: Color = Color::Indexed(236);
const LIGHT_SURFACE: Color = Color::Indexed(254);
const DARK_HOVER: Color = Color::Indexed(235);
const LIGHT_HOVER: Color = Color::Indexed(255);
const CLOSE_BUTTON_WIDTH: u16 = 3;
const COMPACT_CLOSE_WIDTH: u16 = 5;
const TOAST_ICON: &str = " ✓ ";
const TOAST_MARGIN: u16 = 1;
const NAME_RESERVED_COLS: usize = 6;
const BEHIND_ICON: &str = "↓";
const GROUP_INDENT: &str = "  ";
pub const GROUP_ICONS: [char; 12] = ['●', '◉', '◐', '◆', '■', '▲', '▼', '★', '✦', '♥', '♣', '♠'];
pub const GROUP_COLOURS: [u8; 16] = [1, 9, 208, 214, 3, 11, 2, 10, 6, 14, 4, 12, 99, 5, 13, 205];
const ICONS_PER_ROW: usize = 6;
const COLOURS_PER_ROW: usize = 8;
const ICON_CELL: u16 = 3;
const COLOUR_CELL: u16 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Border {
    Projects,
    Workspaces,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Widths {
    pub projects: u16,
    pub workspaces: u16,
}

impl Default for Widths {
    fn default() -> Self {
        Self { projects: SIDEBAR_WIDTH, workspaces: WORKSPACES_WIDTH }
    }
}

fn columns_room(total: u16) -> u16 {
    total.saturating_sub(PANE_PADDING + MIN_PANE_WIDTH)
}

impl Widths {
    #[must_use]
    pub fn fit(self, total: u16) -> Self {
        let room = columns_room(total);
        let workspaces = self.workspaces.min(room.saturating_sub(self.projects)).max(MIN_COLUMN_WIDTH);
        let projects = self.projects.min(room.saturating_sub(workspaces)).max(MIN_COLUMN_WIDTH);
        Self { projects, workspaces }
    }

    #[must_use]
    pub fn dragged(self, border: Border, x: u16, total: u16) -> Self {
        let fitted = self.fit(total);
        let room = columns_room(total);
        let right = x.saturating_add(1);
        match border {
            Border::Projects => {
                let max = room.saturating_sub(fitted.workspaces).max(MIN_COLUMN_WIDTH);
                Self { projects: right.clamp(MIN_COLUMN_WIDTH, max), ..self }
            }
            Border::Workspaces => {
                let max = room.saturating_sub(fitted.projects).max(MIN_COLUMN_WIDTH);
                Self { workspaces: right.saturating_sub(fitted.projects).clamp(MIN_COLUMN_WIDTH, max), ..self }
            }
        }
    }

    #[must_use]
    pub fn reset(self, border: Border) -> Self {
        match border {
            Border::Projects => Self { projects: SIDEBAR_WIDTH, ..self },
            Border::Workspaces => Self { workspaces: WORKSPACES_WIDTH, ..self },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Projects,
    Workspaces,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Areas {
    pub pitch: u16,
    pub bar: Rect,
    pub brand: Rect,
    pub search: Rect,
    pub search_button: Rect,
    pub back: Rect,
    pub sidebar: Rect,
    pub title: Rect,
    pub list: Rect,
    pub separator: Rect,
    pub settings: Rect,
    pub quit: Rect,
    pub workspaces: Rect,
    pub workspaces_title: Rect,
    pub workspaces_list: Rect,
    pub workspaces_separator: Rect,
    pub issues: Rect,
    pub results: Rect,
    pub pane: Rect,
    pub projects_border: Rect,
    pub workspaces_border: Rect,
}

impl Areas {
    pub fn border(&self, border: Border) -> Rect {
        match border {
            Border::Projects => self.projects_border,
            Border::Workspaces => self.workspaces_border,
        }
    }

    pub fn border_hit(&self, pos: Position) -> Option<Border> {
        [Border::Projects, Border::Workspaces].into_iter().find(|&b| self.border(b).contains(pos))
    }

    pub fn compact(&self) -> bool {
        !self.bar.is_empty()
    }

    #[must_use]
    pub fn shown(self, nav: Option<Nav>) -> Self {
        if !self.compact() {
            return self;
        }
        let hidden = Rect::default();
        let projects = if nav == Some(Nav::Projects) {
            self
        } else {
            Self {
                sidebar: hidden,
                title: hidden,
                list: hidden,
                separator: hidden,
                settings: hidden,
                quit: hidden,
                ..self
            }
        };
        if nav == Some(Nav::Workspaces) {
            projects
        } else {
            Self {
                workspaces: hidden,
                workspaces_title: hidden,
                workspaces_list: hidden,
                workspaces_separator: hidden,
                issues: hidden,
                back: hidden,
                ..projects
            }
        }
    }
}

fn right_edge(r: Rect) -> Rect {
    Rect::new(r.right().saturating_sub(1), r.y, r.width.min(1), r.height)
}

fn sidebar_block() -> Block<'static> {
    Block::default().borders(Borders::RIGHT).border_style(Style::default().fg(Color::DarkGray))
}

fn projects_column(r: Rect) -> [Rect; 5] {
    let [title, _, list, separator, settings, quit] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(GAP),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(r);
    [title, list, separator, settings, quit]
}

fn workspaces_column(r: Rect) -> [Rect; 4] {
    let [title, _, list, separator, issues, _] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(GAP),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(r);
    [title, list, separator, issues]
}

fn below_header(r: Rect) -> Rect {
    Layout::vertical([Constraint::Length(HEADER_HEIGHT), Constraint::Min(0)]).areas::<2>(r)[1]
}

pub fn layout(area: Rect, widths: Widths) -> Areas {
    if area.width < COMPACT_WIDTH { compact_layout(area) } else { wide_layout(area, widths) }
}

fn wide_layout(area: Rect, widths: Widths) -> Areas {
    let Widths { projects, workspaces } = widths.fit(area.width);
    let [columns, _, pane] = Layout::horizontal([
        Constraint::Length(projects + workspaces),
        Constraint::Length(PANE_PADDING),
        Constraint::Min(1),
    ])
    .areas(area);
    let [left, workspaces] =
        Layout::horizontal([Constraint::Length(projects), Constraint::Length(workspaces)]).areas(columns);
    let [header, results] = Layout::vertical([Constraint::Length(HEADER_HEIGHT), Constraint::Min(0)])
        .areas(Rect { width: columns.width.saturating_sub(1), ..columns });
    let [brand, _, search, _] = Layout::vertical([
        Constraint::Length(BRAND_HEIGHT),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(header);
    let sidebar = below_header(left);
    let [title, list, separator, settings, quit] = projects_column(sidebar_block().inner(sidebar));
    let [workspaces_title, workspaces_list, workspaces_separator, issues] =
        workspaces_column(below_header(sidebar_block().inner(workspaces)));
    let search = search.inner(Margin::new(1, 0));
    Areas {
        pitch: 1,
        bar: Rect::default(),
        brand,
        search,
        search_button: search,
        back: Rect::default(),
        sidebar,
        title,
        list,
        separator,
        settings,
        quit,
        workspaces,
        workspaces_title,
        workspaces_list,
        workspaces_separator,
        issues,
        results,
        pane,
        projects_border: right_edge(sidebar),
        workspaces_border: right_edge(workspaces),
    }
}

fn compact_layout(area: Rect) -> Areas {
    let pitch = COMPACT_PITCH;
    let [bar, below] = Layout::vertical([Constraint::Length(pitch), Constraint::Min(0)]).areas(area);
    let search_width = COMPACT_BUTTON_WIDTH.min(bar.width);
    let search_button = Rect { x: bar.right() - search_width, width: search_width, ..bar };
    let [_, menu] = Layout::vertical([Constraint::Length(GAP), Constraint::Min(0)]).areas(below);
    let column = |footer: u16| {
        Layout::vertical([
            Constraint::Length(pitch),
            Constraint::Length(GAP),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(footer),
        ])
        .areas::<5>(menu)
    };
    let [title, _, list, separator, footer] = column(pitch);
    let [settings, quit] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(footer);
    let [workspaces_title, _, workspaces_list, workspaces_separator, issues] = column(pitch);
    let back = Rect { width: button_width(BACK_LABEL) + 2, ..workspaces_title }.intersection(workspaces_title);
    Areas {
        pitch,
        bar,
        brand: Rect::default(),
        search: bar,
        search_button,
        back,
        sidebar: below,
        title,
        list,
        separator,
        settings,
        quit,
        workspaces: below,
        workspaces_title,
        workspaces_list,
        workspaces_separator,
        issues,
        results: below,
        pane: below,
        projects_border: Rect::default(),
        workspaces_border: Rect::default(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rows {
    list: Rect,
    heights: Vec<u16>,
    button: u16,
    scroll: usize,
}

impl Rows {
    fn room(&self) -> u16 {
        self.list.height.saturating_sub(GAP + self.button)
    }

    fn height(&self, range: Range<usize>) -> u32 {
        self.heights[range].iter().map(|&h| u32::from(h)).sum()
    }

    fn fitting_before(&self, end: usize) -> usize {
        let room = u32::from(self.room());
        let mut start = end;
        while start > 0 && self.height(start - 1..end) <= room {
            start -= 1;
        }
        start
    }

    fn first(&self) -> usize {
        self.scroll.min(self.fitting_before(self.heights.len()))
    }

    fn end(&self) -> usize {
        let (first, room) = (self.first(), u32::from(self.room()));
        (first..self.heights.len()).take_while(|&i| self.height(first..i + 1) <= room).last().map_or(first, |i| i + 1)
    }

    pub fn item(&self, i: usize) -> Rect {
        let first = self.first();
        if !(first..self.end()).contains(&i) {
            return Rect::default();
        }
        let y = u32::from(self.list.y) + self.height(first..i);
        Rect::new(self.list.x, u16::try_from(y).unwrap_or(u16::MAX), self.list.width, self.heights[i])
    }

    pub fn at(&self, pos: Position) -> Option<usize> {
        (self.first()..self.end()).find(|&i| self.item(i).contains(pos))
    }

    pub fn hidden(&self) -> (Range<usize>, Range<usize>) {
        (0..self.first(), self.end()..self.heights.len())
    }

    pub fn scrolled(&self, delta: isize) -> usize {
        let max = self.fitting_before(self.heights.len());
        self.scroll.min(max).saturating_add_signed(delta).min(max)
    }

    pub fn reveal(&self, i: usize) -> usize {
        let first = self.first();
        if i >= self.heights.len() || (first..self.end()).contains(&i) {
            first
        } else if i < first {
            i
        } else {
            self.fitting_before(i + 1).min(i)
        }
    }

    pub fn button(&self) -> Rect {
        let gap = if self.heights.is_empty() { 0 } else { u32::from(GAP) };
        let below = u32::from(self.list.y) + self.height(0..self.heights.len()) + gap;
        let last = self.list.bottom().saturating_sub(self.button);
        let y = u16::try_from(below).unwrap_or(u16::MAX).min(last);
        Rect::new(self.list.x, y, self.list.width, self.button)
    }

    pub fn more_below(&self) -> Rect {
        Rect::new(self.list.x, self.list.y.saturating_add(self.room()), self.list.width, 1).intersection(self.list)
    }
}

pub fn more_above(list: Rect) -> Rect {
    if list.y < GAP { Rect::default() } else { Rect::new(list.x, list.y - GAP, list.width, 1) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarRow {
    Gap,
    Group(usize),
    Project(usize),
}

pub fn sidebar_rows(groups: &[Option<usize>], collapsed: &[bool]) -> Vec<SidebarRow> {
    let in_group = |g: Option<usize>| {
        groups.iter().enumerate().filter(move |(_, group)| **group == g).map(|(p, _)| SidebarRow::Project(p))
    };
    let mut rows: Vec<SidebarRow> = in_group(None).collect();
    for (g, &folded) in collapsed.iter().enumerate() {
        if !rows.is_empty() {
            rows.push(SidebarRow::Gap);
        }
        rows.push(SidebarRow::Group(g));
        if !folded {
            rows.extend(in_group(Some(g)));
        }
    }
    rows
}

pub fn active_row(rows: &[SidebarRow], project: usize, group: Option<usize>) -> Option<usize> {
    let find = |row: SidebarRow| rows.iter().position(|r| *r == row);
    find(SidebarRow::Project(project)).or_else(|| find(SidebarRow::Group(group?)))
}

fn gapped_rows<R: PartialEq>(list: Rect, pitch: u16, rows: &[R], gap: &R, scroll: usize) -> Rows {
    let heights = rows.iter().map(|r| if r == gap { GAP } else { pitch }).collect();
    Rows { list, heights, button: pitch, scroll }
}

fn row_rect<R: PartialEq>(layout: &Rows, rows: &[R], row: &R) -> Rect {
    rows.iter().position(|r| r == row).map_or_else(Rect::default, |i| layout.item(i))
}

pub fn project_rows(list: Rect, pitch: u16, rows: &[SidebarRow], scroll: usize) -> Rows {
    gapped_rows(list, pitch, rows, &SidebarRow::Gap, scroll)
}

pub fn entry_row(list: Rect, pitch: u16, rows: &[SidebarRow], scroll: usize, row: SidebarRow) -> Rect {
    row_rect(&project_rows(list, pitch, rows, scroll), rows, &row)
}

pub fn close_button(list: Rect, pitch: u16, rows: &[SidebarRow], scroll: usize, p: usize) -> Rect {
    row_close_button(entry_row(list, pitch, rows, scroll, SidebarRow::Project(p)))
}

pub fn new_project_button(list: Rect, pitch: u16, rows: &[SidebarRow]) -> Rect {
    project_rows(list, pitch, rows, 0).button()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarHit {
    Select(usize),
    Close(usize),
    Group(usize),
    New,
}

pub fn sidebar_hit(list: Rect, pitch: u16, rows: &[SidebarRow], scroll: usize, pos: Position) -> Option<SidebarHit> {
    if !list.contains(pos) {
        return None;
    }
    let layout = project_rows(list, pitch, rows, scroll);
    if layout.button().contains(pos) {
        return Some(SidebarHit::New);
    }
    let i = layout.at(pos)?;
    match rows[i] {
        SidebarRow::Gap => None,
        SidebarRow::Group(g) => Some(SidebarHit::Group(g)),
        SidebarRow::Project(p) if row_close_button(layout.item(i)).contains(pos) => Some(SidebarHit::Close(p)),
        SidebarRow::Project(p) => Some(SidebarHit::Select(p)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceRow {
    Gap,
    Workspace(usize),
    Tab(usize, usize),
    NewTab(usize),
}

pub fn workspace_rows(tabs: &[usize]) -> Vec<WorkspaceRow> {
    tabs.iter()
        .enumerate()
        .flat_map(|(w, &n)| {
            (w > 0)
                .then_some(WorkspaceRow::Gap)
                .into_iter()
                .chain(std::iter::once(WorkspaceRow::Workspace(w)))
                .chain((0..n).map(move |t| WorkspaceRow::Tab(w, t)))
                .chain(std::iter::once(WorkspaceRow::NewTab(w)))
        })
        .collect()
}

pub fn workspace_layout(list: Rect, pitch: u16, tabs: &[usize], scroll: usize) -> Rows {
    gapped_rows(list, pitch, &workspace_rows(tabs), &WorkspaceRow::Gap, scroll)
}

pub fn new_workspace_button(list: Rect, pitch: u16, tabs: &[usize]) -> Rect {
    workspace_layout(list, pitch, tabs, 0).button()
}

pub fn workspace_row(list: Rect, pitch: u16, tabs: &[usize], scroll: usize, row: WorkspaceRow) -> Rect {
    row_rect(&workspace_layout(list, pitch, tabs, scroll), &workspace_rows(tabs), &row)
}

pub fn row_close_button(row: Rect) -> Rect {
    let width = if row.height > 1 { COMPACT_CLOSE_WIDTH } else { CLOSE_BUTTON_WIDTH };
    Rect::new(row.right().saturating_sub(width), row.y, width.min(row.width), row.height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceHit {
    Workspace(usize),
    CloseWorkspace(usize),
    Tab(usize, usize),
    CloseTab(usize, usize),
    NewTab(usize),
    NewWorkspace,
}

pub fn workspace_hit(list: Rect, pitch: u16, tabs: &[usize], scroll: usize, pos: Position) -> Option<WorkspaceHit> {
    if !list.contains(pos) {
        return None;
    }
    let layout = workspace_layout(list, pitch, tabs, scroll);
    if layout.button().contains(pos) {
        return Some(WorkspaceHit::NewWorkspace);
    }
    let i = layout.at(pos)?;
    let on_close = row_close_button(layout.item(i)).contains(pos);
    Some(match workspace_rows(tabs)[i] {
        WorkspaceRow::Gap => return None,
        WorkspaceRow::Workspace(w) if on_close => WorkspaceHit::CloseWorkspace(w),
        WorkspaceRow::Workspace(w) => WorkspaceHit::Workspace(w),
        WorkspaceRow::Tab(w, t) if on_close => WorkspaceHit::CloseTab(w, t),
        WorkspaceRow::Tab(w, t) => WorkspaceHit::Tab(w, t),
        WorkspaceRow::NewTab(w) => WorkspaceHit::NewTab(w),
    })
}

pub fn menu_area(area: Rect, at: Position, items: &[impl AsRef<str>]) -> Rect {
    let longest = items.iter().map(|item| item.as_ref().chars().count()).max().unwrap_or(0);
    let width = u16::try_from(longest).unwrap_or(u16::MAX).saturating_add(4).min(area.width);
    let height = u16::try_from(items.len()).unwrap_or(u16::MAX).saturating_add(2).min(area.height);
    let x = at.x.min(area.right().saturating_sub(width));
    let y = at.y.saturating_add(1).min(area.bottom().saturating_sub(height));
    Rect::new(x, y, width, height)
}

pub fn menu_item(menu: Rect, i: usize) -> Rect {
    let y = menu.y.saturating_add(1).saturating_add(u16::try_from(i).unwrap_or(u16::MAX));
    Rect::new(menu.x + 1, y, menu.width.saturating_sub(2), 1).intersection(menu)
}

pub fn menu_hit(menu: Rect, items: usize, pos: Position) -> Option<usize> {
    (0..items).find(|&i| menu_item(menu, i).contains(pos))
}

pub fn form_area(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(FORM_WIDTH);
    let height = FORM_HEIGHT.min(area.height);
    Rect::new(area.x + (area.width - width) / 2, area.y + (area.height - height) / 2, width, height)
}

fn form_inner(form: Rect) -> Rect {
    Block::bordered().inner(form).inner(Margin::new(FORM_PADDING, 0))
}

fn form_rows(form: Rect) -> [Rect; 6] {
    let [label, input, hint, toggle, note, _, buttons] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(form_inner(form));
    [label, input, hint, toggle, note, buttons]
}

pub fn form_toggle(form: Rect) -> Rect {
    form_rows(form)[3]
}

fn button_width(label: &str) -> u16 {
    u16::try_from(label.chars().count()).unwrap_or(u16::MAX).saturating_add(2)
}

pub fn update_button(settings: Rect, label: &str) -> Rect {
    let width = button_width(label).min(settings.width);
    Rect { x: settings.right() - width, width, ..settings }
}

pub fn form_buttons(form: Rect, submit: &str) -> [Rect; 2] {
    buttons_in(form_rows(form)[5], submit)
}

fn buttons_in(row: Rect, submit: &str) -> [Rect; 2] {
    let cancel_width = button_width(CANCEL_LABEL);
    let submit_width = button_width(submit);
    let cancel = Rect::new(row.right().saturating_sub(cancel_width), row.y, cancel_width, 1).intersection(row);
    let submit = Rect::new(cancel.x.saturating_sub(submit_width + 1), row.y, submit_width, 1).intersection(row);
    [submit, cancel]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormHit {
    Submit,
    Cancel,
    Toggle,
}

pub fn form_hit(area: Rect, submit: &str, pos: Position) -> Option<FormHit> {
    let form = form_area(area);
    let [s, c] = form_buttons(form, submit);
    if s.contains(pos) {
        Some(FormHit::Submit)
    } else if c.contains(pos) {
        Some(FormHit::Cancel)
    } else if form_toggle(form).contains(pos) {
        Some(FormHit::Toggle)
    } else {
        None
    }
}

fn style_rows(area: Rect) -> [Rect; 5] {
    let [icon_label, icons, colour_label, colours, _, last] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(form_inner(form_area(area)));
    [icon_label, icons, colour_label, colours, last]
}

fn grid_cell(grid: Rect, per_row: usize, width: u16, i: usize) -> Rect {
    let (row, col) = (u16::try_from(i / per_row).unwrap_or(u16::MAX), u16::try_from(i % per_row).unwrap_or(u16::MAX));
    Rect::new(grid.x.saturating_add(col.saturating_mul(width)), grid.y.saturating_add(row), width, 1).intersection(grid)
}

pub fn style_icon(area: Rect, i: usize) -> Rect {
    grid_cell(style_rows(area)[1], ICONS_PER_ROW, ICON_CELL, i)
}

pub fn style_colour(area: Rect, i: usize) -> Rect {
    grid_cell(style_rows(area)[3], COLOURS_PER_ROW, COLOUR_CELL, i)
}

pub fn style_done(area: Rect) -> Rect {
    update_button(style_rows(area)[4], crate::settings::DONE)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleHit {
    Icon(usize),
    Colour(usize),
    Done,
}

pub fn style_hit(area: Rect, pos: Position) -> Option<StyleHit> {
    let [_, icons, _, colours, last] = style_rows(area);
    if update_button(last, crate::settings::DONE).contains(pos) {
        return Some(StyleHit::Done);
    }
    let cell = |grid, per_row, width, count| (0..count).find(|&i| grid_cell(grid, per_row, width, i).contains(pos));
    cell(icons, ICONS_PER_ROW, ICON_CELL, GROUP_ICONS.len())
        .map(StyleHit::Icon)
        .or_else(|| cell(colours, COLOURS_PER_ROW, COLOUR_CELL, GROUP_COLOURS.len()).map(StyleHit::Colour))
}

pub fn picker_area(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(PICKER_WIDTH);
    let height = area.height.saturating_sub(2).min(PICKER_HEIGHT);
    Rect::new(area.x + (area.width - width) / 2, area.y + (area.height - height) / 2, width, height)
}

fn picker_rows(picker: Rect) -> [Rect; 4] {
    let [input, _, list, note, buttons] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(form_inner(picker));
    [input, list, note, buttons]
}

fn update_rows(update: Rect) -> [Rect; 4] {
    let [message, _, notes, note, buttons] = Layout::vertical([
        Constraint::Length(UPDATE_MESSAGE_HEIGHT),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(form_inner(update));
    [message, notes, note, buttons]
}

pub fn update_notes(area: Rect) -> Rect {
    update_rows(picker_area(area))[1]
}

pub fn update_buttons(area: Rect, submit: &str) -> [Rect; 2] {
    buttons_in(update_rows(picker_area(area))[3], submit)
}

pub fn update_scroll(area: Rect, lines: usize, scroll: usize) -> usize {
    scroll.min(lines.saturating_sub(usize::from(update_notes(area).height)))
}

pub fn picker_list(picker: Rect) -> Rect {
    picker_rows(picker)[1]
}

pub fn picker_buttons(picker: Rect, submit: &str) -> [Rect; 2] {
    buttons_in(picker_rows(picker)[3], submit)
}

fn first_visible(list: Rect, items: usize, scroll: usize) -> usize {
    scroll.min(items.saturating_sub(usize::from(list.height)))
}

pub fn picker_item(picker: Rect, items: usize, scroll: usize, i: usize) -> Rect {
    list_item(picker_list(picker), items, scroll, i)
}

pub fn list_item_at(list_box: Rect, items: usize, scroll: usize, i: usize) -> Position {
    list_item(picker_list(list_box), items, scroll, i).as_position()
}

fn list_item(list: Rect, items: usize, scroll: usize, i: usize) -> Rect {
    let Some(row) = i.checked_sub(first_visible(list, items, scroll)).filter(|_| i < items) else {
        return Rect::default();
    };
    let y = list.y.saturating_add(u16::try_from(row).unwrap_or(u16::MAX));
    Rect::new(list.x, y, list.width, 1).intersection(list)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerHit {
    Item(usize),
    Submit,
    Cancel,
}

pub fn picker_hit(area: Rect, submit: &str, items: usize, scroll: usize, pos: Position) -> Option<PickerHit> {
    list_box_hit(picker_area(area), submit, items, scroll, pos)
}

pub fn issues_area(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(ISSUES_WIDTH);
    let height = area.height.saturating_sub(2).min(ISSUES_HEIGHT);
    Rect::new(area.x + (area.width - width) / 2, area.y + (area.height - height) / 2, width, height)
}

pub fn settings_area(area: Rect) -> Rect {
    issues_area(area)
}

fn settings_rows(settings: Rect) -> [Rect; 5] {
    let [tabs, _, body, edit, note, buttons] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(GAP),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(form_inner(settings));
    [tabs, body, edit, note, buttons]
}

pub fn settings_tabs(settings: Rect, names: &[&str]) -> Vec<Rect> {
    tabs_in(settings_rows(settings)[0], names)
}

pub fn settings_body(settings: Rect) -> Rect {
    settings_rows(settings)[1]
}

pub fn settings_edit(settings: Rect) -> Rect {
    settings_rows(settings)[2]
}

pub fn settings_done(settings: Rect) -> Rect {
    issue_buttons_in(settings_rows(settings)[4], &[crate::settings::DONE])[0]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsLine {
    Blank,
    Header(usize),
    Row(usize),
}

pub fn settings_lines(sections: &[&str]) -> Vec<SettingsLine> {
    let mut lines = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        if !section.is_empty() && (i == 0 || sections[i - 1] != *section) {
            if i > 0 {
                lines.push(SettingsLine::Blank);
            }
            lines.push(SettingsLine::Header(i));
        }
        lines.push(SettingsLine::Row(i));
    }
    lines
}

pub fn settings_scroll(sections: &[&str], cursor: usize, height: usize) -> usize {
    let lines = settings_lines(sections);
    let at = lines.iter().position(|l| *l == SettingsLine::Row(cursor)).unwrap_or(0);
    let header = at.saturating_sub(1);
    if at < height { 0 } else { (at + 1).saturating_sub(height).min(header) }
}

pub fn settings_row(area: Rect, sections: &[&str], cursor: usize, row: usize) -> Rect {
    let body = settings_body(settings_area(area));
    let scroll = settings_scroll(sections, cursor, usize::from(body.height));
    let Some(line) = settings_lines(sections).iter().position(|l| *l == SettingsLine::Row(row)) else {
        return Rect::default();
    };
    let Some(visible) = line.checked_sub(scroll) else { return Rect::default() };
    let y = body.y.saturating_add(u16::try_from(visible).unwrap_or(u16::MAX));
    Rect::new(body.x, y, body.width, 1).intersection(body)
}

pub fn settings_remove(row: Rect) -> Rect {
    row_close_button(row)
}

pub fn settings_moves(row: Rect) -> [Rect; 2] {
    let down = Rect::new(row.right().saturating_sub(3), row.y, 3, 1).intersection(row);
    let up = Rect::new(row.right().saturating_sub(6), row.y, 3, 1).intersection(row);
    [up, down]
}

pub fn settings_pick_list(settings: Rect) -> Rect {
    let body = settings_body(settings);
    Rect::new(body.x, body.y.saturating_add(2), body.width, body.height.saturating_sub(2))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsHit {
    Tab(usize),
    Row(usize),
    Remove(usize),
    MoveUp(usize),
    MoveDown(usize),
    Pick(usize),
    Done,
}

pub struct SettingsLayout<'a> {
    pub tabs: &'a [&'a str],
    pub sections: &'a [&'a str],
    pub removable: &'a [bool],
    pub movable: &'a [bool],
    pub cursor: usize,
    pub pick: Option<(usize, usize)>,
}

pub fn settings_hit(area: Rect, layout: &SettingsLayout, pos: Position) -> Option<SettingsHit> {
    let settings = settings_area(area);
    if settings_done(settings).contains(pos) {
        return Some(SettingsHit::Done);
    }
    if let Some(i) = settings_tabs(settings, layout.tabs).iter().position(|r| r.contains(pos)) {
        return Some(SettingsHit::Tab(i));
    }
    if let Some((items, scroll)) = layout.pick {
        let list = settings_pick_list(settings);
        if !list.contains(pos) {
            return None;
        }
        let i = first_visible(list, items, scroll) + usize::from(pos.y - list.y);
        return (i < items).then_some(SettingsHit::Pick(i));
    }
    let i =
        (0..layout.sections.len()).find(|&i| settings_row(area, layout.sections, layout.cursor, i).contains(pos))?;
    let row = settings_row(area, layout.sections, layout.cursor, i);
    if layout.removable.get(i).copied().unwrap_or(false) && settings_remove(row).contains(pos) {
        return Some(SettingsHit::Remove(i));
    }
    if layout.movable.get(i).copied().unwrap_or(false) {
        let [up, down] = settings_moves(row);
        if up.contains(pos) {
            return Some(SettingsHit::MoveUp(i));
        }
        if down.contains(pos) {
            return Some(SettingsHit::MoveDown(i));
        }
    }
    Some(SettingsHit::Row(i))
}

fn issues_rows(issues: Rect) -> [Rect; 5] {
    let [tabs, _, input, _, list, note, buttons] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(form_inner(issues));
    [tabs, input, list, note, buttons]
}

pub fn issues_list(issues: Rect) -> Rect {
    issues_rows(issues)[2]
}

pub fn issue_detail(issues: Rect) -> Rect {
    let [tabs, _, _, note, _] = issues_rows(issues);
    Rect::new(tabs.x, tabs.y, tabs.width, note.y.saturating_sub(tabs.y))
}

fn row_of(row: Rect, x: u16, width: u16) -> Rect {
    Rect::new(x, row.y, width, 1).intersection(row)
}

pub fn issue_tabs(issues: Rect, names: &[&str]) -> Vec<Rect> {
    tabs_in(issues_rows(issues)[0], names)
}

fn tabs_in(row: Rect, names: &[&str]) -> Vec<Rect> {
    let mut x = row.x;
    names
        .iter()
        .map(|name| {
            let r = row_of(row, x, button_width(name));
            x = x.saturating_add(button_width(name) + 1);
            r
        })
        .collect()
}

fn toggle_width(label: &str) -> u16 {
    button_width(label).saturating_add(4)
}

pub fn issue_toggles(issues: Rect, labels: &[&str]) -> Vec<Rect> {
    right_aligned(issues_rows(issues)[0], labels, toggle_width, 2)
}

pub fn issue_buttons(issues: Rect, labels: &[&str]) -> Vec<Rect> {
    issue_buttons_in(issues_rows(issues)[4], labels)
}

fn issue_buttons_in(row: Rect, labels: &[&str]) -> Vec<Rect> {
    right_aligned(row, labels, button_width, 1)
}

fn right_aligned(row: Rect, labels: &[&str], width: fn(&str) -> u16, gap: u16) -> Vec<Rect> {
    let mut x = row.right();
    let mut rects: Vec<Rect> = labels
        .iter()
        .rev()
        .map(|label| {
            x = x.saturating_sub(width(label));
            let r = row_of(row, x, width(label));
            x = x.saturating_sub(gap);
            r
        })
        .collect();
    rects.reverse();
    rects
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssuesHit {
    Tab(usize),
    Toggle(usize),
    Item(usize),
    Button(usize),
}

pub fn issue_button_hit(area: Rect, buttons: &[&str], pos: Position) -> Option<usize> {
    issue_buttons(issues_area(area), buttons).iter().position(|r| r.contains(pos))
}

pub fn issues_hit(
    area: Rect,
    tabs: &[&str],
    toggles: &[&str],
    buttons: &[&str],
    items: usize,
    scroll: usize,
    pos: Position,
) -> Option<IssuesHit> {
    let issues = issues_area(area);
    if let Some(i) = issue_tabs(issues, tabs).iter().position(|r| r.contains(pos)) {
        return Some(IssuesHit::Tab(i));
    }
    if let Some(i) = issue_toggles(issues, toggles).iter().position(|r| r.contains(pos)) {
        return Some(IssuesHit::Toggle(i));
    }
    if let Some(i) = issue_button_hit(area, buttons, pos) {
        return Some(IssuesHit::Button(i));
    }
    let list = issues_list(issues);
    if !list.contains(pos) {
        return None;
    }
    let i = first_visible(list, items, scroll) + usize::from(pos.y - list.y);
    (i < items).then_some(IssuesHit::Item(i))
}

fn list_box_hit(picker: Rect, submit: &str, items: usize, scroll: usize, pos: Position) -> Option<PickerHit> {
    let [s, c] = picker_buttons(picker, submit);
    if s.contains(pos) {
        return Some(PickerHit::Submit);
    }
    if c.contains(pos) {
        return Some(PickerHit::Cancel);
    }
    let list = picker_list(picker);
    if !list.contains(pos) {
        return None;
    }
    let i = first_visible(list, items, scroll) + usize::from(pos.y - list.y);
    (i < items).then_some(PickerHit::Item(i))
}

fn results_rows(results: Rect) -> [Rect; 2] {
    let [list, _, hint] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(GAP), Constraint::Length(1)]).areas(results);
    [list, hint]
}

pub fn results_list(results: Rect) -> Rect {
    results_rows(results)[0]
}

pub fn result_item(results: Rect, items: usize, scroll: usize, i: usize) -> Rect {
    list_item(results_list(results), items, scroll, i)
}

pub fn result_hit(results: Rect, items: usize, scroll: usize, pos: Position) -> Option<usize> {
    let list = results_list(results);
    if !list.contains(pos) {
        return None;
    }
    let i = first_visible(list, items, scroll) + usize::from(pos.y - list.y);
    (i < items).then_some(i)
}

pub enum Note {
    Error(String),
    Busy(&'static str),
}

pub struct Toggle {
    pub label: &'static str,
    pub on: bool,
}

pub struct Form {
    pub title: &'static str,
    pub label: &'static str,
    pub value: String,
    pub hint: String,
    pub toggle: Option<Toggle>,
    pub note: Option<Note>,
    pub submit: &'static str,
}

pub struct Confirm {
    pub title: &'static str,
    pub message: String,
    pub note: Option<Note>,
    pub submit: &'static str,
}

pub struct Update {
    pub message: String,
    pub notes: Vec<Line<'static>>,
    pub scroll: usize,
    pub note: Option<Note>,
    pub submit: &'static str,
}

pub struct Picker {
    pub title: &'static str,
    pub path: String,
    pub filter: String,
    pub items: Vec<Entry>,
    pub selected: Option<usize>,
    pub scroll: usize,
    pub hint: String,
    pub error: Option<String>,
    pub submit: &'static str,
}

pub struct SettingsRow {
    pub section: &'static str,
    pub label: String,
    pub value: String,
    pub note: String,
    pub dangerous: bool,
    pub removable: bool,
    pub movable: bool,
}

pub struct SettingsEdit {
    pub label: String,
    pub value: String,
}

pub struct SettingsPick {
    pub title: String,
    pub filter: String,
    pub items: Vec<(String, String, bool)>,
    pub selected: Option<usize>,
    pub scroll: usize,
}

pub struct Settings {
    pub tabs: Vec<&'static str>,
    pub tab: usize,
    pub rows: Vec<SettingsRow>,
    pub cursor: usize,
    pub edit: Option<SettingsEdit>,
    pub pick: Option<SettingsPick>,
    pub note: Option<Note>,
    pub hint: String,
    pub submit: &'static str,
}

impl Settings {
    pub fn sections(&self) -> Vec<&'static str> {
        self.rows.iter().map(|r| r.section).collect()
    }
}

pub struct IssueRow {
    pub key: String,
    pub title: String,
    pub meta: String,
}

pub enum IssuesBody {
    List { filter: String, items: Vec<IssueRow>, selected: Option<usize>, scroll: usize, empty: String },
    Token { label: &'static str, input: String, help: Vec<String> },
    Detail { lines: Vec<Line<'static>>, scroll: usize },
}

pub struct Issues {
    pub title: String,
    pub tabs: Vec<&'static str>,
    pub tab: usize,
    pub toggles: Vec<String>,
    pub on: Vec<bool>,
    pub body: IssuesBody,
    pub note: Option<Note>,
    pub hint: String,
    pub buttons: Vec<&'static str>,
}

pub struct ResultRow {
    pub name: String,
    pub context: String,
}

pub struct Search {
    pub query: String,
    pub results: Vec<ResultRow>,
    pub selected: usize,
    pub scroll: usize,
    pub hint: String,
}

impl Search {
    fn showing_results(&self) -> bool {
        !self.query.trim().is_empty()
    }
}

pub enum Overlay {
    Menu { at: Position, items: Vec<String> },
    Form(Form),
    GroupStyle(GroupEntry),
    Confirm(Confirm),
    Update(Update),
    Picker(Picker),
    Issues(Issues),
    Settings(Settings),
    Search(Search),
}

pub struct Entry {
    pub name: String,
    pub branch: Option<String>,
}

pub struct ProjectEntry {
    pub name: String,
    pub workspaces: usize,
    pub group: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupEntry {
    pub name: String,
    pub icon: char,
    pub colour: u8,
    #[serde(default)]
    pub collapsed: bool,
}

impl GroupEntry {
    pub fn label(&self) -> String {
        format!("{} {}", self.icon, self.name)
    }
}

pub struct WorkspaceEntry {
    pub name: String,
    pub tabs: Vec<String>,
    pub behind: u32,
}

pub struct TabView {
    pub layout: Node<usize>,
    pub screens: Vec<Snapshot>,
    pub active: usize,
    pub dim_inactive: bool,
    pub dragging: Option<Vec<bool>>,
}

pub struct View<'a> {
    pub groups: Vec<GroupEntry>,
    pub projects: Vec<ProjectEntry>,
    pub active: usize,
    pub projects_scroll: usize,
    pub has_project: bool,
    pub workspaces: Vec<WorkspaceEntry>,
    pub active_workspace: usize,
    pub active_tab: Option<usize>,
    pub workspaces_scroll: usize,
    pub issues: bool,
    pub hover: Option<Position>,
    pub widths: Widths,
    pub resizing: Option<Border>,
    pub light: bool,
    pub tab: Option<TabView>,
    pub overlay: Option<Overlay>,
    pub toast: Option<&'a str>,
    pub nav: Option<Nav>,
    pub update: Option<String>,
}

impl View<'_> {
    pub fn sidebar_rows(&self) -> Vec<SidebarRow> {
        let groups: Vec<Option<usize>> = self.projects.iter().map(|p| p.group).collect();
        let collapsed: Vec<bool> = self.groups.iter().map(|g| g.collapsed).collect();
        sidebar_rows(&groups, &collapsed)
    }

    pub fn tab_counts(&self) -> Vec<usize> {
        self.workspaces.iter().map(|w| w.tabs.len()).collect()
    }

    fn surface(&self) -> Color {
        if self.light { LIGHT_SURFACE } else { DARK_SURFACE }
    }

    fn row_background(&self, r: Rect, active: bool) -> Style {
        if active {
            Style::default().bg(self.surface())
        } else if self.row_hovered(r) {
            Style::default().bg(if self.light { LIGHT_HOVER } else { DARK_HOVER })
        } else {
            Style::default()
        }
    }

    fn row_hovered(&self, r: Rect) -> bool {
        self.resizing.is_none() && self.tab.as_ref().is_none_or(|t| t.dragging.is_none()) && sidebar_hovered(self, r)
    }

    fn search(&self) -> Option<&Search> {
        match &self.overlay {
            Some(Overlay::Search(search)) => Some(search),
            _ => None,
        }
    }
}

pub fn draw(f: &mut Frame, view: &View) {
    let areas = layout(f.area(), view.widths).shown(view.nav);
    match &view.tab {
        Some(tab) => draw_tab(f, view, tab, areas.pane),
        None if view.has_project => f.render_widget(
            Paragraph::new(Span::styled(" no tab open", Style::default().fg(Color::DarkGray))),
            areas.pane,
        ),
        None => {}
    }
    if areas.compact() {
        draw_bar(f, view, &areas);
        if view.nav.is_some() {
            f.render_widget(Clear, areas.pane);
        }
    } else {
        f.render_widget(sidebar_block(), areas.sidebar);
        f.render_widget(sidebar_block(), areas.workspaces);
        draw_borders(f, view, &areas);
        draw_brand(f, areas.brand);
        draw_search_bar(f, view, areas.search);
    }
    if !areas.sidebar.is_empty() {
        draw_sidebar(f, view, &areas);
    }
    if !areas.workspaces.is_empty() {
        draw_workspaces(f, view, &areas);
    }
    match &view.overlay {
        Some(Overlay::Menu { at, items }) => draw_menu(f, view, *at, items),
        Some(Overlay::Form(form)) => draw_form(f, view, form),
        Some(Overlay::GroupStyle(group)) => draw_group_style(f, view, group),
        Some(Overlay::Confirm(confirm)) => draw_confirm(f, view, confirm),
        Some(Overlay::Update(update)) => draw_update(f, view, update),
        Some(Overlay::Picker(picker)) => draw_picker(f, view, picker),
        Some(Overlay::Issues(issues)) => draw_issues(f, view, issues),
        Some(Overlay::Settings(settings)) => draw_settings(f, view, settings),
        Some(Overlay::Search(search)) if search.showing_results() => draw_results(f, view, search, areas.results),
        Some(Overlay::Search(_)) | None => {}
    }
    if let Some(message) = view.toast {
        draw_toast(f, message);
    }
}

pub fn toast_area(area: Rect, message: &str) -> Rect {
    let text = TOAST_ICON.chars().count() + message.chars().count() + 1;
    let width = u16::try_from(text).unwrap_or(u16::MAX).saturating_add(2).min(area.width);
    let height = 3.min(area.height);
    let x = area.right().saturating_sub(width + TOAST_MARGIN).max(area.x);
    let y = area.bottom().saturating_sub(height + TOAST_MARGIN).max(area.y);
    Rect::new(x, y, width, height)
}

fn draw_toast(f: &mut Frame, message: &str) {
    let r = toast_area(f.area(), message);
    f.render_widget(Clear, r);
    f.render_widget(Block::bordered().border_style(Style::default().fg(Color::Green)), r);
    let line = Line::from(vec![Span::styled(TOAST_ICON, Style::default().fg(Color::Green)), Span::raw(message)]);
    f.render_widget(Paragraph::new(line), r.inner(Margin::new(1, 1)));
}

fn draw_tab(f: &mut Frame, view: &View, tab: &TabView, area: Rect) {
    let panes = tab.layout.panes(area);
    let split = panes.len() > 1;
    for (i, pane) in panes {
        let Some(screen) = tab.screens.get(i) else { continue };
        let active = i == tab.active;
        draw_screen(f, screen, pane, active && view.overlay.is_none(), split && !active && tab.dim_inactive);
    }
    draw_dividers(f, view, tab, area);
}

fn draw_screen(f: &mut Frame, screen: &Snapshot, pane: Rect, show_cursor: bool, dim: bool) {
    let buf = f.buffer_mut();
    for (y, row) in (pane.y..pane.bottom()).zip(&screen.rows) {
        for (x, cell) in (pane.x..pane.right()).zip(row) {
            let style = if dim { cell.style.add_modifier(Modifier::DIM) } else { cell.style };
            buf[(x, y)].set_symbol(&cell.symbol).set_style(style);
        }
    }
    if let Some(c) = screen.cursor.filter(|_| show_cursor)
        && c.x < pane.width
        && c.y < pane.height
    {
        f.set_cursor_position(Position::new(pane.x + c.x, pane.y + c.y));
    }
}

const UP: u8 = 1;
const DOWN: u8 = 2;
const LEFT: u8 = 4;
const RIGHT: u8 = 8;

fn divider_symbol(links: u8) -> &'static str {
    match links {
        l if l == UP | DOWN | LEFT | RIGHT => "┼",
        l if l == UP | DOWN | RIGHT => "├",
        l if l == UP | DOWN | LEFT => "┤",
        l if l == LEFT | RIGHT | DOWN => "┬",
        l if l == LEFT | RIGHT | UP => "┴",
        l if l & (UP | DOWN) != 0 => "│",
        _ => "─",
    }
}

fn draw_dividers(f: &mut Frame, view: &View, tab: &TabView, area: Rect) {
    let dividers = tab.layout.dividers(area);
    let mut cells: HashMap<(u16, u16), (u8, bool)> = HashMap::new();
    for d in &dividers {
        let lit = tab.dragging.as_ref() == Some(&d.path) || (tab.dragging.is_none() && sidebar_hovered(view, d.line));
        let links = match d.dir {
            Dir::Right => UP | DOWN,
            Dir::Down => LEFT | RIGHT,
        };
        for y in d.line.top()..d.line.bottom() {
            for x in d.line.left()..d.line.right() {
                let cell = cells.entry((x, y)).or_default();
                cell.0 |= links;
                cell.1 |= lit;
            }
        }
    }
    let bridges: Vec<(u16, u16)> = dividers
        .iter()
        .filter(|d| d.dir == Dir::Down)
        .filter_map(|d| {
            let pad = d.line.x.checked_sub(1)?;
            let bar = pad.checked_sub(1)?;
            let joins =
                !cells.contains_key(&(pad, d.line.y)) && cells.get(&(bar, d.line.y)).is_some_and(|c| c.0 & UP != 0);
            joins.then_some((pad, d.line.y))
        })
        .collect();
    for (x, y) in bridges {
        cells.insert((x, y), (LEFT | RIGHT, false));
        if let Some(bar) = cells.get_mut(&(x - 1, y)) {
            bar.0 |= RIGHT;
        }
    }
    for d in &dividers {
        let Rect { x, y, .. } = d.line;
        let ends = match d.dir {
            Dir::Right => [(Some(x), y.checked_sub(1), DOWN), (Some(x), Some(d.line.bottom()), UP)],
            Dir::Down => [(x.checked_sub(1), Some(y), RIGHT), (Some(d.line.right()), Some(y), LEFT)],
        };
        for (x, y, link) in ends {
            if let (Some(x), Some(y)) = (x, y)
                && let Some(cell) = cells.get_mut(&(x, y))
            {
                cell.0 |= link;
            }
        }
    }
    let buf = f.buffer_mut();
    for ((x, y), (links, lit)) in cells {
        let color = if lit { Color::Cyan } else { Color::DarkGray };
        buf[(x, y)].set_symbol(divider_symbol(links)).set_style(Style::default().fg(color));
    }
}

fn hovered(view: &View, r: Rect) -> bool {
    view.hover.is_some_and(|p| r.contains(p))
}

fn sidebar_hovered(view: &View, r: Rect) -> bool {
    view.overlay.is_none() && hovered(view, r)
}

fn overlay_block(title: &str) -> Block<'_> {
    let block = Block::bordered().border_style(Style::default().fg(Color::DarkGray));
    if title.is_empty() {
        block
    } else {
        block.title(Span::styled(format!(" {title} "), Style::default().add_modifier(Modifier::BOLD)))
    }
}

fn draw_menu(f: &mut Frame, view: &View, at: Position, items: &[String]) {
    let menu = menu_area(f.area(), at, items);
    f.render_widget(Clear, menu);
    f.render_widget(overlay_block(""), menu);
    for (i, item) in items.iter().enumerate() {
        let r = menu_item(menu, i);
        let style = if hovered(view, r) {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        f.render_widget(Paragraph::new(format!(" {item} ")).style(style), r);
    }
}

fn draw_form(f: &mut Frame, view: &View, form: &Form) {
    let r = form_area(f.area());
    f.render_widget(Clear, r);
    f.render_widget(overlay_block(form.title), r);
    let [label, input, hint, toggle, note, _] = form_rows(r);
    let dim = Style::default().fg(Color::DarkGray);

    f.render_widget(Paragraph::new(Span::styled(form.label, dim)), label);
    if let Some(t) = &form.toggle {
        let mark = if t.on { "[x] " } else { "[ ] " };
        let style = if hovered(view, toggle) { Style::default().fg(Color::Cyan) } else { Style::default() };
        f.render_widget(Paragraph::new(Span::styled(format!("{mark}{}", t.label), style)), toggle);
    }
    let max = usize::from(input.width).saturating_sub(INPUT_PROMPT.chars().count() + 1);
    let value = truncate_left(&form.value, max);
    let cursor_x = input.x + u16::try_from(INPUT_PROMPT.chars().count() + value.chars().count()).unwrap_or(0);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(INPUT_PROMPT, Style::default().fg(Color::Cyan)),
            Span::styled(value, Style::default().add_modifier(Modifier::BOLD)),
        ])),
        input,
    );
    f.render_widget(Paragraph::new(Span::styled(truncate_left(&form.hint, usize::from(hint.width)), dim)), hint);

    draw_note(f, form.note.as_ref(), note);

    draw_dialog_buttons(f, view, form_buttons(r, form.submit), form.submit);

    if !matches!(form.note, Some(Note::Busy(_))) && cursor_x < input.right() {
        f.set_cursor_position(Position::new(cursor_x, input.y));
    }
}

fn group_header(group: &GroupEntry, max: usize) -> Span<'static> {
    let style = Style::default().fg(Color::Indexed(group.colour)).add_modifier(Modifier::BOLD);
    Span::styled(format!("{} {}", group.icon, truncate_right(&group.name, max)), style)
}

fn draw_group_style(f: &mut Frame, view: &View, group: &GroupEntry) {
    let area = f.area();
    let r = form_area(area);
    f.render_widget(Clear, r);
    f.render_widget(overlay_block(&group.name), r);
    let [icon_label, icons, colour_label, colours, last] = style_rows(area);
    let dim = Style::default().fg(Color::DarkGray);
    f.render_widget(Paragraph::new(Span::styled("icon", dim)), icon_label);
    f.render_widget(Paragraph::new(Span::styled("colour", dim)), colour_label);
    let selected = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
    for (i, &icon) in GROUP_ICONS.iter().enumerate() {
        let cell = grid_cell(icons, ICONS_PER_ROW, ICON_CELL, i);
        let style = if icon == group.icon {
            selected
        } else if hovered(view, cell) {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Indexed(group.colour))
        };
        f.render_widget(Paragraph::new(Span::styled(format!(" {icon} "), style)), cell);
    }
    for (i, &colour) in GROUP_COLOURS.iter().enumerate() {
        let cell = grid_cell(colours, COLOURS_PER_ROW, COLOUR_CELL, i);
        let (open, close, edge) = if colour == group.colour {
            ("[", "]", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
        } else if hovered(view, cell) {
            ("[", "]", dim)
        } else {
            (" ", " ", dim)
        };
        let line = Line::from(vec![
            Span::styled(open, edge),
            Span::styled("██", Style::default().fg(Color::Indexed(colour))),
            Span::styled(close, edge),
        ]);
        f.render_widget(Paragraph::new(line), cell);
    }
    let done = update_button(last, crate::settings::DONE);
    let max = usize::from(last.width.saturating_sub(done.width)).saturating_sub(5);
    let preview = Line::from(vec![Span::styled("▾ ", dim), group_header(group, max)]);
    f.render_widget(Paragraph::new(preview), last);
    draw_submit(f, view, done, crate::settings::DONE);
}

fn draw_note(f: &mut Frame, note: Option<&Note>, r: Rect) {
    match note {
        Some(Note::Error(text)) => f.render_widget(
            Paragraph::new(text.as_str()).style(Style::default().fg(Color::Red)).wrap(Wrap { trim: true }),
            r,
        ),
        Some(Note::Busy(text)) => {
            f.render_widget(Paragraph::new(Span::styled(*text, Style::default().fg(Color::DarkGray))), r);
        }
        None => {}
    }
}

fn draw_confirm(f: &mut Frame, view: &View, confirm: &Confirm) {
    let r = form_area(f.area());
    f.render_widget(Clear, r);
    f.render_widget(overlay_block(confirm.title), r);
    let [label, _, _, toggle, note, _] = form_rows(r);
    let message = Rect::new(label.x, label.y, label.width, toggle.bottom().saturating_sub(label.y));
    f.render_widget(Paragraph::new(confirm.message.as_str()).wrap(Wrap { trim: true }), message);
    draw_note(f, confirm.note.as_ref(), note);
    draw_dialog_buttons(f, view, form_buttons(r, confirm.submit), confirm.submit);
}

fn draw_update(f: &mut Frame, view: &View, update: &Update) {
    let r = picker_area(f.area());
    f.render_widget(Clear, r);
    f.render_widget(overlay_block("update"), r);
    let [message, notes, note, _] = update_rows(r);
    f.render_widget(Paragraph::new(update.message.as_str()).wrap(Wrap { trim: true }), message);
    let scroll = update_scroll(f.area(), update.notes.len(), update.scroll);
    let visible: Vec<Line> = update.notes.iter().skip(scroll).take(usize::from(notes.height)).cloned().collect();
    f.render_widget(Paragraph::new(visible), notes);
    draw_note(f, update.note.as_ref(), note);
    draw_dialog_buttons(f, view, update_buttons(f.area(), update.submit), update.submit);
}

fn draw_submit(f: &mut Frame, view: &View, r: Rect, label: &str) {
    let style = if hovered(view, r) {
        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    };
    f.render_widget(Paragraph::new(Span::styled(format!(" {label} "), style)), r);
}

fn draw_dialog_buttons(f: &mut Frame, view: &View, [submit, cancel]: [Rect; 2], label: &str) {
    let dim = Style::default().fg(Color::DarkGray);
    let cancel_style = if hovered(view, cancel) { Style::default().fg(Color::Black).bg(Color::Gray) } else { dim };
    draw_submit(f, view, submit, label);
    f.render_widget(Paragraph::new(Span::styled(format!(" {CANCEL_LABEL} "), cancel_style)), cancel);
}

fn draw_picker(f: &mut Frame, view: &View, picker: &Picker) {
    let r = picker_area(f.area());
    f.render_widget(Clear, r);
    f.render_widget(overlay_block(picker.title), r);
    let [input, list, note, _] = picker_rows(r);
    let dim = Style::default().fg(Color::DarkGray);

    let max = usize::from(input.width).saturating_sub(INPUT_PROMPT.chars().count() + 1);
    let filter = truncate_left(&picker.filter, max);
    let path = truncate_left(&picker.path, max.saturating_sub(filter.chars().count()));
    let typed = path.chars().count() + filter.chars().count();
    let cursor_x = input.x + u16::try_from(INPUT_PROMPT.chars().count() + typed).unwrap_or(0);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(INPUT_PROMPT, Style::default().fg(Color::Cyan)),
            Span::raw(path),
            Span::styled(filter, Style::default().add_modifier(Modifier::BOLD)),
        ])),
        input,
    );

    if picker.items.is_empty() {
        let text = if picker.filter.is_empty() { "no folders here" } else { "no matches" };
        f.render_widget(Paragraph::new(Span::styled(format!(" {text}"), dim)), list);
    }
    for (i, item) in picker.items.iter().enumerate() {
        let row = picker_item(r, picker.items.len(), picker.scroll, i);
        if row.is_empty() {
            continue;
        }
        let highlighted = picker.selected == Some(i) || hovered(view, row);
        let (style, branch_style) = if highlighted {
            let style = Style::default().fg(Color::Black).bg(Color::Cyan);
            (style.add_modifier(Modifier::BOLD), style)
        } else {
            (Style::default(), Style::default().fg(Color::DarkGray))
        };
        let branch = item.branch.as_deref().unwrap_or_default();
        let branch_width = branch.chars().count();
        let max = usize::from(row.width).saturating_sub(branch_width + 3);
        let name = truncate_left(&item.name, max);
        let gap = usize::from(row.width).saturating_sub(name.chars().count() + branch_width + 2);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {name}{}", " ".repeat(gap)), style),
                Span::styled(format!("{branch} "), branch_style),
            ]))
            .style(style),
            row,
        );
    }

    match &picker.error {
        Some(error) => f.render_widget(
            Paragraph::new(Span::styled(
                truncate_left(error, usize::from(note.width)),
                Style::default().fg(Color::Red),
            )),
            note,
        ),
        None => f.render_widget(
            Paragraph::new(Span::styled(truncate_left(&picker.hint, usize::from(note.width)), dim)),
            note,
        ),
    }

    draw_dialog_buttons(f, view, picker_buttons(r, picker.submit), picker.submit);

    if cursor_x < input.right() {
        f.set_cursor_position(Position::new(cursor_x, input.y));
    }
}

fn draw_settings(f: &mut Frame, view: &View, settings: &Settings) {
    let area = f.area();
    let r = settings_area(area);
    f.render_widget(Clear, r);
    f.render_widget(overlay_block("settings"), r);
    let [_, body, edit_row, note, _] = settings_rows(r);
    let dim = Style::default().fg(Color::DarkGray);
    let mut cursor = None;
    draw_tabs(f, view, &settings_tabs(r, &settings.tabs), &settings.tabs, settings.tab);

    if let Some(pick) = &settings.pick {
        cursor = draw_input(f, Rect { height: 1, ..body }, &pick.title, &pick.filter);
        let list = settings_pick_list(r);
        if pick.items.is_empty() {
            f.render_widget(Paragraph::new(Span::styled(" nothing matches", dim)), list);
        }
        for (i, (value, item_note, dangerous)) in pick.items.iter().enumerate() {
            let row = list_item(list, pick.items.len(), pick.scroll, i);
            if row.is_empty() {
                continue;
            }
            let highlighted = pick.selected == Some(i) || hovered(view, row);
            let base = if highlighted { Style::default().fg(Color::Black).bg(Color::Cyan) } else { Style::default() };
            let value_style = if *dangerous && !highlighted { base.fg(Color::Red) } else { base };
            let note_style = if highlighted { base } else { dim };
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!(" {value:<32} "), value_style.add_modifier(Modifier::BOLD)),
                    Span::styled(item_note.clone(), note_style),
                ]))
                .style(base),
                row,
            );
        }
    } else {
        let sections = settings.sections();
        let scroll = settings_scroll(&sections, settings.cursor, usize::from(body.height));
        let lines = settings_lines(&sections);
        for (n, line) in lines.iter().skip(scroll).take(usize::from(body.height)).enumerate() {
            let y = body.y + u16::try_from(n).unwrap_or(0);
            let rect = Rect::new(body.x, y, body.width, 1);
            match line {
                SettingsLine::Blank => {}
                SettingsLine::Header(i) => f.render_widget(
                    Paragraph::new(Span::styled(settings.rows[*i].section, dim.add_modifier(Modifier::BOLD))),
                    rect,
                ),
                SettingsLine::Row(i) => draw_settings_row(f, view, &settings.rows[*i], *i == settings.cursor, rect),
            }
        }
    }

    if let Some(edit) = &settings.edit {
        cursor = draw_input(f, edit_row, &format!("{}:", edit.label), &edit.value);
    }
    let (text, style) = match &settings.note {
        Some(Note::Error(text)) => (text.as_str(), Style::default().fg(Color::Red)),
        Some(Note::Busy(text)) => (*text, dim),
        None => (settings.hint.as_str(), dim),
    };
    f.render_widget(Paragraph::new(Span::styled(truncate_right(text, usize::from(note.width)), style)), note);
    draw_submit(f, view, settings_done(r), settings.submit);
    if let Some(pos) = cursor.filter(|_| !matches!(settings.note, Some(Note::Busy(_)))) {
        f.set_cursor_position(pos);
    }
}

fn draw_settings_row(f: &mut Frame, view: &View, row: &SettingsRow, selected: bool, r: Rect) {
    let dim = Style::default().fg(Color::DarkGray);
    let (marker, bg) = if selected { ("› ", Style::default().bg(view.surface())) } else { ("  ", Style::default()) };
    let label_style = if selected { bg.add_modifier(Modifier::BOLD) } else { bg };
    let value_style = if row.dangerous { bg.fg(Color::Red) } else { bg };
    let width = usize::from(r.width);
    let label = truncate_right(&row.label, 24);
    let value = truncate_right(&row.value, width.saturating_sub(30) / 2 + 10);
    let spans = vec![
        Span::styled(marker, bg.fg(Color::Cyan)),
        Span::styled(format!("{label:<26}"), label_style),
        Span::styled(value.clone(), value_style),
        Span::styled(if value.is_empty() || row.note.is_empty() { String::new() } else { "  ".into() }, bg),
        Span::styled(row.note.clone(), bg.fg(Color::DarkGray)),
    ];
    f.render_widget(Paragraph::new(Line::from(spans)).style(bg), r);
    if !hovered(view, r) {
        return;
    }
    if row.removable {
        let close = settings_remove(r);
        let style = if hovered(view, close) { bg.fg(Color::Red).add_modifier(Modifier::BOLD) } else { bg.patch(dim) };
        f.render_widget(Paragraph::new(Span::styled(" × ", style)), close);
    }
    if row.movable {
        for (rect, arrow) in settings_moves(r).into_iter().zip([" ↑ ", " ↓ "]) {
            let style =
                if hovered(view, rect) { bg.fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { bg.patch(dim) };
            f.render_widget(Paragraph::new(Span::styled(arrow, style)), rect);
        }
    }
}

fn draw_issues(f: &mut Frame, view: &View, issues: &Issues) {
    let r = issues_area(f.area());
    f.render_widget(Clear, r);
    f.render_widget(overlay_block(&issues.title), r);
    let [_, input, list, note, _] = issues_rows(r);
    let dim = Style::default().fg(Color::DarkGray);

    let cursor = match &issues.body {
        IssuesBody::Detail { lines, scroll } => {
            let area = issue_detail(r);
            let visible: Vec<Line> = lines.iter().skip(*scroll).take(usize::from(area.height)).cloned().collect();
            f.render_widget(Paragraph::new(visible), area);
            None
        }
        IssuesBody::List { filter, items, selected, scroll, empty } => {
            draw_issue_tabs(f, view, issues, r);
            let cursor = draw_input(f, input, "", filter);
            if items.is_empty() {
                f.render_widget(Paragraph::new(Span::styled(format!(" {empty}"), dim)), list);
            }
            draw_issue_rows(f, view, list, items, *selected, *scroll);
            cursor
        }
        IssuesBody::Token { label, input: typed, help } => {
            draw_issue_tabs(f, view, issues, r);
            let cursor = draw_input(f, input, label, typed);
            let text: Vec<Line> = help.iter().map(|h| Line::from(h.as_str())).collect();
            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), list.inner(Margin::new(1, 0)));
            cursor
        }
    };

    let (text, style) = match &issues.note {
        Some(Note::Error(text)) => (text.as_str(), Style::default().fg(Color::Red)),
        Some(Note::Busy(text)) => (*text, dim),
        None => (issues.hint.as_str(), dim),
    };
    f.render_widget(Paragraph::new(Span::styled(truncate_right(text, usize::from(note.width)), style)), note);

    let rects = issue_buttons(r, &issues.buttons);
    for (i, (rect, label)) in rects.iter().zip(&issues.buttons).enumerate() {
        let style = match (i == 0, hovered(view, *rect)) {
            (true, true) => Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
            (true, false) => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            (false, true) => Style::default().fg(Color::Black).bg(Color::Gray),
            (false, false) => dim,
        };
        f.render_widget(Paragraph::new(Span::styled(format!(" {label} "), style)), *rect);
    }

    if let Some(pos) = cursor.filter(|_| !matches!(issues.note, Some(Note::Busy(_)))) {
        f.set_cursor_position(pos);
    }
}

fn draw_input(f: &mut Frame, row: Rect, label: &str, value: &str) -> Option<Position> {
    let label = if label.is_empty() { String::new() } else { format!("{label} ") };
    let max = usize::from(row.width).saturating_sub(INPUT_PROMPT.chars().count() + label.chars().count() + 1);
    let value = truncate_left(value, max);
    let used = INPUT_PROMPT.chars().count() + label.chars().count() + value.chars().count();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(INPUT_PROMPT, Style::default().fg(Color::Cyan)),
            Span::styled(label, Style::default().fg(Color::DarkGray)),
            Span::styled(value, Style::default().add_modifier(Modifier::BOLD)),
        ])),
        row,
    );
    let x = row.x.saturating_add(u16::try_from(used).unwrap_or(u16::MAX));
    (x < row.right()).then(|| Position::new(x, row.y))
}

fn draw_issue_tabs(f: &mut Frame, view: &View, issues: &Issues, r: Rect) {
    draw_tabs(f, view, &issue_tabs(r, &issues.tabs), &issues.tabs, issues.tab);
    let labels: Vec<&str> = issues.toggles.iter().map(String::as_str).collect();
    for (rect, (label, on)) in issue_toggles(r, &labels).iter().zip(issues.toggles.iter().zip(&issues.on)) {
        let mark = if *on { "[x]" } else { "[ ]" };
        let style = if hovered(view, *rect) { Style::default().fg(Color::Cyan) } else { Style::default() };
        f.render_widget(Paragraph::new(Span::styled(format!(" {mark} {label} "), style)), *rect);
    }
}

fn draw_tabs(f: &mut Frame, view: &View, rects: &[Rect], names: &[&str], active: usize) {
    for (i, (rect, name)) in rects.iter().zip(names).enumerate() {
        let style = if i == active {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else if hovered(view, *rect) {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        };
        f.render_widget(Paragraph::new(Span::styled(format!(" {name} "), style)), *rect);
    }
}

fn draw_issue_rows(f: &mut Frame, view: &View, list: Rect, items: &[IssueRow], selected: Option<usize>, scroll: usize) {
    let key_width = items.iter().map(|i| i.key.chars().count()).max().unwrap_or(0);
    let dim = Style::default().fg(Color::DarkGray);
    for (i, item) in items.iter().enumerate() {
        let row = list_item(list, items.len(), scroll, i);
        if row.is_empty() {
            continue;
        }
        let highlighted = selected == Some(i) || hovered(view, row);
        let (style, key_style, meta_style) = if highlighted {
            let style = Style::default().fg(Color::Black).bg(Color::Cyan);
            (style.add_modifier(Modifier::BOLD), style.add_modifier(Modifier::BOLD), style)
        } else {
            (Style::default(), Style::default().fg(Color::Cyan), dim)
        };
        let width = usize::from(row.width);
        let meta = truncate_right(&item.meta, width / 3);
        let meta_width = meta.chars().count();
        let title = truncate_right(&item.title, width.saturating_sub(key_width + meta_width + 6));
        let gap = width.saturating_sub(key_width + title.chars().count() + meta_width + 5);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {:<key_width$}  ", item.key), key_style),
                Span::styled(format!("{title}{}", " ".repeat(gap)), style),
                Span::styled(format!("{meta} "), meta_style),
            ]))
            .style(style),
            row,
        );
    }
}

fn draw_sidebar(f: &mut Frame, view: &View, areas: &Areas) {
    draw_title(f, areas.title, "projects");
    let sidebar = view.sidebar_rows();
    let rows = project_rows(areas.list, areas.pitch, &sidebar, view.projects_scroll);
    draw_entries(f, view, &rows, &sidebar);
    let (above, below) = rows.hidden();
    let count = |range: Range<usize>| {
        (!range.is_empty()).then(|| sidebar[range].iter().filter(|r| **r != SidebarRow::Gap).count())
    };
    draw_more(f, [more_above(areas.list), rows.more_below()], count(above), count(below));
    let r = rows.button();
    draw_button(f, r, " ", "+ new project", button_style(view, r, Style::default().fg(Color::Cyan), Color::Cyan));
    draw_separator(f, areas.separator);
    let mut settings = areas.settings;
    if let Some(label) = &view.update {
        let r = update_button(areas.settings, label);
        let idle = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
        draw_button(f, r, "", label, button_style(view, r, idle, Color::Cyan));
        settings.width -= r.width;
    }
    draw_settings_button(f, view, settings);
    draw_quit_button(f, view, areas.quit);
}

fn draw_borders(f: &mut Frame, view: &View, areas: &Areas) {
    for border in [Border::Projects, Border::Workspaces] {
        let r = areas.border(border);
        if view.resizing == Some(border) || sidebar_hovered(view, r) {
            let buf = f.buffer_mut();
            for y in r.top()..r.bottom() {
                buf[(r.x, y)].set_fg(Color::Cyan);
            }
        }
    }
}

fn draw_brand(f: &mut Frame, r: Rect) {
    let mark = Style::default().fg(BRAND_COLOR);
    let lines = vec![
        Line::from(vec![
            Span::styled(" ▄▀▀▀ ", mark),
            Span::styled("c", mark.add_modifier(Modifier::BOLD)),
            Span::styled("ornercase", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(Span::styled(" █", mark)),
    ];
    f.render_widget(Paragraph::new(lines), r);
}

fn draw_search_bar(f: &mut Frame, view: &View, r: Rect) {
    let dim = Style::default().fg(Color::DarkGray);
    let accent = Style::default().fg(Color::Cyan);
    let line = if let Some(search) = view.search() {
        let max = usize::from(r.width).saturating_sub(SEARCH_ICON.chars().count() + 1);
        let query = truncate_left(&search.query, max);
        let cursor = SEARCH_ICON.chars().count() + query.chars().count();
        if let Ok(offset) = u16::try_from(cursor)
            && offset < r.width
        {
            f.set_cursor_position(Position::new(r.x + offset, r.y));
        }
        Line::from(vec![
            Span::styled(SEARCH_ICON, accent),
            Span::styled(query, Style::default().add_modifier(Modifier::BOLD)),
        ])
    } else {
        let icon = if sidebar_hovered(view, r) { accent } else { dim };
        Line::from(vec![Span::styled(SEARCH_ICON, icon), Span::styled(SEARCH_PLACEHOLDER, dim)])
    };
    f.render_widget(Paragraph::new(line).style(Style::default().bg(view.surface())), r);
}

fn draw_results(f: &mut Frame, view: &View, search: &Search, r: Rect) {
    f.render_widget(Clear, r);
    let [list, hint] = results_rows(r);
    let dim = Style::default().fg(Color::DarkGray);
    if search.results.is_empty() {
        f.render_widget(Paragraph::new(Span::styled("  no matches", dim)), list);
    }
    for (i, result) in search.results.iter().enumerate() {
        let row = result_item(r, search.results.len(), search.scroll, i);
        if row.is_empty() {
            continue;
        }
        let highlighted = search.selected == i || hovered(view, row);
        let (base, matched, context_style) = if highlighted {
            let style = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
            (style, style, style.remove_modifier(Modifier::BOLD))
        } else {
            (Style::default(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD), dim)
        };
        let width = usize::from(row.width);
        let context = truncate_left(&result.context, width / 2);
        let context_width = context.chars().count();
        let name = truncate_right(&result.name, width.saturating_sub(context_width + 5));
        let gap = width.saturating_sub(name.chars().count() + context_width + 3);
        let mut spans = vec![Span::styled("  ", base)];
        spans.extend(highlight(&name, &search.query, base, matched));
        spans.push(Span::styled(" ".repeat(gap), base));
        spans.push(Span::styled(format!("{context} "), context_style));
        f.render_widget(Paragraph::new(Line::from(spans)).style(base), row);
    }
    f.render_widget(
        Paragraph::new(Span::styled(
            format!("  {}", truncate_left(&search.hint, usize::from(hint.width).saturating_sub(2))),
            dim,
        )),
        hint,
    );
}

fn highlight(name: &str, query: &str, base: Style, matched: Style) -> Vec<Span<'static>> {
    let Some((start, end)) = crate::search::find(name, query) else {
        return vec![Span::styled(name.to_string(), base)];
    };
    let part = |from: usize, to: usize| name.chars().skip(from).take(to - from).collect::<String>();
    vec![
        Span::styled(part(0, start), base),
        Span::styled(part(start, end), matched),
        Span::styled(part(end, name.chars().count()), base),
    ]
}

fn draw_bar(f: &mut Frame, view: &View, areas: &Areas) {
    let surface = Style::default().bg(view.surface());
    f.render_widget(Block::new().style(surface), areas.bar);
    if view.search().is_some() {
        draw_search_bar(f, view, middle(areas.bar));
        return;
    }
    let pressed = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
    let menu = Rect { width: areas.bar.width.saturating_sub(areas.search_button.width), ..areas.bar };
    let icon = Rect { width: COMPACT_BUTTON_WIDTH.min(menu.width), ..menu };
    let style = if view.nav.is_some() || sidebar_hovered(view, menu) { pressed } else { surface.fg(Color::Cyan) };
    draw_band(f, icon, Span::styled(centered(MENU_ICON, icon.width), style), style);
    let crumbs = Rect { x: icon.right() + 2, width: menu.width.saturating_sub(icon.width + 2), ..middle(menu) };
    f.render_widget(Paragraph::new(Line::from(breadcrumb(view, usize::from(crumbs.width)))), crumbs);
    let r = areas.search_button;
    let style = if sidebar_hovered(view, r) { pressed } else { surface.fg(Color::DarkGray) };
    draw_band(f, r, Span::styled(centered(SEARCH_ICON.trim(), r.width), style), style);
}

fn middle(r: Rect) -> Rect {
    Rect { y: r.y + r.height.saturating_sub(1) / 2, height: r.height.min(1), ..r }
}

fn centered(text: &str, width: u16) -> String {
    format!("{text:^width$}", width = usize::from(width))
}

fn draw_band<'a>(f: &mut Frame, r: Rect, line: impl Into<Line<'a>>, style: Style) {
    f.render_widget(Block::new().style(style), r);
    f.render_widget(Paragraph::new(line.into()).style(style), middle(r));
}

fn breadcrumb(view: &View, room: usize) -> Vec<Span<'static>> {
    let Some(project) = view.projects.get(view.active).filter(|_| view.has_project) else {
        let mark = Style::default().fg(BRAND_COLOR).add_modifier(Modifier::BOLD);
        return vec![Span::styled("c", mark), Span::styled("ornercase", Style::default().add_modifier(Modifier::BOLD))];
    };
    let workspace = view.workspaces.get(view.active_workspace);
    let tab = workspace.zip(view.active_tab).and_then(|(w, t)| w.tabs.get(t));
    let crumbs: Vec<&str> = [Some(project.name.as_str()), workspace.map(|w| w.name.as_str()), tab.map(String::as_str)]
        .into_iter()
        .flatten()
        .collect();
    let text = truncate_right(&crumbs.join(CRUMB_SEPARATOR), room);
    let project_len = project.name.chars().count().min(text.chars().count());
    let bold = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
    let (head, rest): (String, String) =
        (text.chars().take(project_len).collect(), text.chars().skip(project_len).collect());
    vec![Span::styled(head, bold), Span::styled(rest, Style::default().fg(Color::Gray))]
}

fn draw_back(f: &mut Frame, view: &View, areas: &Areas) {
    let style = button_style(view, areas.back, Style::default().fg(Color::Cyan), Color::Cyan);
    draw_button(f, areas.back, "", BACK_LABEL, style);
    let name = view.projects.get(view.active).filter(|_| view.has_project).map(|p| p.name.as_str()).unwrap_or_default();
    let rest = Rect {
        x: areas.back.right(),
        width: areas.workspaces_title.right().saturating_sub(areas.back.right()),
        ..areas.workspaces_title
    };
    let max = usize::from(rest.width).saturating_sub(1);
    let style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD);
    f.render_widget(Paragraph::new(Span::styled(format!(" {}", truncate_right(name, max)), style)), middle(rest));
}

fn draw_title(f: &mut Frame, r: Rect, title: &str) {
    let style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD);
    f.render_widget(Paragraph::new(Span::styled(format!(" {title}"), style)), middle(r));
}

fn draw_separator(f: &mut Frame, r: Rect) {
    let line = "─".repeat(usize::from(r.width.saturating_sub(2)));
    f.render_widget(Paragraph::new(Span::styled(format!(" {line}"), Style::default().fg(Color::DarkGray))), r);
}

fn button_style(view: &View, r: Rect, idle: Style, hover_bg: Color) -> Style {
    if sidebar_hovered(view, r) {
        Style::default().fg(Color::Black).bg(hover_bg).add_modifier(Modifier::BOLD)
    } else {
        idle
    }
}

fn draw_button(f: &mut Frame, r: Rect, indent: &'static str, label: &str, style: Style) {
    if r.height > 1
        && let Some(bg) = style.bg
    {
        f.render_widget(Block::new().style(Style::default().bg(bg)), r);
    }
    let line = Line::from(vec![Span::raw(indent), Span::styled(format!(" {label} "), style)]);
    f.render_widget(Paragraph::new(line), middle(r));
}

fn draw_workspaces(f: &mut Frame, view: &View, areas: &Areas) {
    if areas.back.is_empty() {
        draw_title(f, areas.workspaces_title, "workspaces");
    } else {
        draw_back(f, view, areas);
    }
    if !view.has_project {
        return;
    }

    let list = areas.workspaces_list;
    let tabs = view.tab_counts();
    let dim = Style::default().fg(Color::DarkGray);
    let accent = Style::default().fg(Color::Cyan);
    let rows = workspace_rows(&tabs);
    let layout = workspace_layout(list, areas.pitch, &tabs, view.workspaces_scroll);
    for (i, &row) in rows.iter().enumerate() {
        let r = layout.item(i);
        if r.is_empty() {
            continue;
        }
        let close_width = usize::from(row_close_button(r).width);
        match row {
            WorkspaceRow::Gap => {}
            WorkspaceRow::Workspace(w) => {
                let style = if w == view.active_workspace {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)
                };
                let entry = &view.workspaces[w];
                let room = usize::from(r.width).saturating_sub(2 + close_width + 1);
                let behind = Some(entry.behind)
                    .filter(|n| *n > 0)
                    .map(|n| format!("{BEHIND_ICON}{n}"))
                    .filter(|tag| tag.chars().count() + 1 < room);
                let tag_width = behind.as_ref().map_or(0, |tag| tag.chars().count() + 1);
                let name = truncate_right(&entry.name, room - tag_width);
                let mut line = vec![Span::styled(format!("  {name}"), style)];
                if let Some(tag) = behind {
                    let pad = room.saturating_sub(name.chars().count() + tag.chars().count());
                    line.extend([Span::raw(" ".repeat(pad)), Span::styled(tag, Style::default().fg(Color::Yellow))]);
                }
                let bg = view.row_background(r, false);
                draw_band(f, r, Line::from(line), bg);
                draw_row_close(f, view, r, bg);
            }
            WorkspaceRow::Tab(w, t) => {
                let active = w == view.active_workspace && view.active_tab == Some(t);
                let (marker, style) = if active {
                    ("▌ ", Style::default().fg(Color::White))
                } else {
                    ("  ", Style::default().fg(Color::Gray))
                };
                let bg = view.row_background(r, active);
                let max = usize::from(r.width).saturating_sub(4 + close_width + 1);
                let name = truncate_right(&view.workspaces[w].tabs[t], max);
                let line = Line::from(vec![Span::raw("  "), Span::styled(marker, accent), Span::styled(name, style)]);
                draw_band(f, r, line, bg);
                draw_row_close(f, view, r, bg);
            }
            WorkspaceRow::NewTab(_) => draw_button(f, r, "   ", "+ tab", button_style(view, r, dim, Color::Cyan)),
        }
    }

    let (above, below) = layout.hidden();
    let named = |range: Range<usize>| {
        (!range.is_empty()).then(|| {
            rows[range].iter().filter(|r| matches!(r, WorkspaceRow::Workspace(_) | WorkspaceRow::Tab(..))).count()
        })
    };
    draw_more(f, [more_above(list), layout.more_below()], named(above), named(below));

    let r = layout.button();
    draw_button(f, r, " ", "+ new workspace", button_style(view, r, accent, Color::Cyan));

    if view.issues {
        draw_separator(f, areas.workspaces_separator);
        let style = button_style(view, areas.issues, Style::default().fg(Color::DarkGray), Color::Cyan);
        draw_button(f, areas.issues, " ", ISSUES_LABEL, style);
    }
}

fn draw_more(f: &mut Frame, [top, bottom]: [Rect; 2], above: Option<usize>, below: Option<usize>) {
    let dim = Style::default().fg(Color::DarkGray);
    for (hidden, arrow, r) in [(above, "↑", top), (below, "↓", bottom)] {
        let label = match hidden {
            None => continue,
            Some(0) => format!("  {arrow} more"),
            Some(n) => format!("  {arrow} {n} more"),
        };
        f.render_widget(Paragraph::new(Span::styled(label, dim)), r);
    }
}

fn draw_row_close(f: &mut Frame, view: &View, row: Rect, bg: Style) {
    if !sidebar_hovered(view, row) {
        return;
    }
    let r = row_close_button(row);
    let style = if hovered(view, r) { bg.fg(Color::Red).add_modifier(Modifier::BOLD) } else { bg.fg(Color::DarkGray) };
    draw_band(f, r, Span::styled(centered("×", r.width), style), style);
}

fn draw_settings_button(f: &mut Frame, view: &View, r: Rect) {
    let style = button_style(view, r, Style::default().fg(Color::DarkGray), Color::Cyan);
    draw_button(f, r, " ", "settings", style);
}

fn draw_quit_button(f: &mut Frame, view: &View, r: Rect) {
    let style = button_style(view, r, Style::default().fg(Color::DarkGray), Color::Red);
    draw_button(f, r, " ", "quit", style);
}

fn draw_entries(f: &mut Frame, view: &View, rows: &Rows, sidebar: &[SidebarRow]) {
    let group = view.projects.get(view.active).and_then(|p| p.group);
    let active = active_row(sidebar, view.active, group).filter(|_| view.has_project);
    for (i, &row) in sidebar.iter().enumerate() {
        let r = rows.item(i);
        if r.is_empty() {
            continue;
        }
        match row {
            SidebarRow::Gap => {}
            SidebarRow::Group(g) => draw_group(f, view, g, r, active == Some(i)),
            SidebarRow::Project(p) => draw_project(f, view, p, r),
        }
    }
}

fn draw_group(f: &mut Frame, view: &View, g: usize, r: Rect, holds_active: bool) {
    let group = &view.groups[g];
    let marker = if holds_active { "▌ " } else { "  " };
    let (arrow, count) = if group.collapsed {
        ("▸ ", format!(" ({})", view.projects.iter().filter(|p| p.group == Some(g)).count()))
    } else {
        ("▾ ", String::new())
    };
    let max = usize::from(r.width).saturating_sub(NAME_RESERVED_COLS + 2 + count.chars().count());
    let line = Line::from(vec![
        Span::styled(marker, Style::default().fg(Color::Cyan)),
        Span::styled(arrow, Style::default().fg(Color::DarkGray)),
        group_header(group, max),
        Span::styled(count, Style::default().fg(Color::DarkGray)),
    ]);
    draw_band(f, r, line, view.row_background(r, false));
}

fn draw_project(f: &mut Frame, view: &View, p: usize, r: Rect) {
    let entry = &view.projects[p];
    let (marker, title_style) = if p == view.active {
        ("▌ ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
    } else {
        ("  ", Style::default().fg(Color::Gray))
    };
    let bg = view.row_background(r, p == view.active);
    let indent = if entry.group.is_some() { GROUP_INDENT } else { "" };
    let count = format!(" ({})", entry.workspaces);
    let reserved = NAME_RESERVED_COLS + indent.len() + usize::from(row_close_button(r).width - CLOSE_BUTTON_WIDTH);
    let max = usize::from(r.width).saturating_sub(reserved + count.chars().count());
    let line = Line::from(vec![
        Span::styled(marker, Style::default().fg(Color::Cyan)),
        Span::raw(indent),
        Span::styled(truncate_right(&entry.name, max), title_style),
        Span::styled(count, Style::default().fg(Color::DarkGray)),
    ]);
    draw_band(f, r, line, bg);
    draw_row_close(f, view, r, bg);
}

pub fn folder_name(path: &Path, home: Option<&Path>) -> String {
    if home == Some(path) {
        return "~".into();
    }
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

pub fn display_path(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

pub fn truncate_right(s: &str, max: usize) -> String {
    if s.chars().count() <= max || max < 2 {
        return s.to_string();
    }
    let head: String = s.chars().take(max - 1).collect();
    format!("{}…", head.trim_end_matches('…'))
}

pub fn truncate_left(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max || max < 2 {
        return s.to_string();
    }
    let tail: String = s.chars().skip(n - (max - 1)).collect();
    format!("…{}", tail.trim_start_matches('…'))
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use rstest::rstest;

    use super::*;
    use crate::emulator::Emulator;
    use crate::host_theme::HostTheme;

    const W: u16 = 100;
    const H: u16 = 18;
    const AREA: Rect = Rect { x: 0, y: 0, width: W, height: H };

    fn view(names: &[&str]) -> View<'static> {
        View {
            groups: Vec::new(),
            projects: names
                .iter()
                .map(|n| ProjectEntry { name: (*n).to_string(), workspaces: 1, group: None })
                .collect(),
            active: 0,
            has_project: false,
            workspaces: Vec::new(),
            active_workspace: 0,
            active_tab: None,
            issues: false,
            hover: None,
            widths: Widths::default(),
            resizing: None,
            light: false,
            projects_scroll: 0,
            workspaces_scroll: 0,
            tab: None,
            overlay: None,
            toast: None,
            nav: None,
            update: None,
        }
    }

    fn plain(projects: usize) -> Vec<SidebarRow> {
        sidebar_rows(&vec![None; projects], &[])
    }

    fn render(view: &View) -> Terminal<TestBackend> {
        render_sized(view, W, H)
    }

    fn render_sized(view: &View, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut t = Terminal::new(TestBackend::new(width, height)).expect("test backend");
        t.draw(|f| draw(f, view)).expect("draw");
        t
    }

    fn areas() -> Areas {
        layout(AREA, Widths::default())
    }

    fn row_text(t: &Terminal<TestBackend>, r: Rect) -> String {
        (r.x..r.right()).map(|x| t.backend().buffer()[(x, r.y)].symbol().to_string()).collect()
    }

    fn list() -> Rect {
        areas().list
    }

    fn screen(bytes: &[u8]) -> Snapshot {
        let mut emu = Emulator::new(
            H,
            W - SIDEBAR_WIDTH - WORKSPACES_WIDTH - PANE_PADDING,
            0,
            &HostTheme::default(),
            Box::new(|_| {}),
        )
        .expect("emulator");
        emu.feed(bytes);
        emu.snapshot().expect("snapshot")
    }

    fn single(screen: Snapshot) -> TabView {
        TabView { layout: Node::Leaf(0), screens: vec![screen], active: 0, dim_inactive: true, dragging: None }
    }

    fn close_x() -> u16 {
        list().right() - 2
    }

    mod layout {
        use super::*;

        #[test]
        fn sidebar_has_fixed_width_on_the_left_below_the_header() {
            assert_eq!(areas().sidebar, Rect::new(0, HEADER_HEIGHT, SIDEBAR_WIDTH, H - HEADER_HEIGHT));
        }

        #[test]
        fn workspaces_column_sits_right_of_the_sidebar() {
            assert_eq!(areas().workspaces, Rect::new(SIDEBAR_WIDTH, 0, WORKSPACES_WIDTH, H));
        }

        #[test]
        fn workspaces_list_starts_level_with_the_projects_list() {
            assert_eq!((areas().workspaces_list.y, areas().workspaces_title.y), (list().y, areas().title.y));
        }

        #[test]
        fn pane_takes_the_rest_after_a_padding_column() {
            let x = SIDEBAR_WIDTH + WORKSPACES_WIDTH + PANE_PADDING;
            assert_eq!(areas().pane, Rect::new(x, 0, W - x, H));
        }

        #[test]
        fn brand_spans_both_columns_at_the_top() {
            assert_eq!(areas().brand, Rect::new(0, 0, SIDEBAR_WIDTH + WORKSPACES_WIDTH - 1, BRAND_HEIGHT));
        }

        #[test]
        fn search_spans_both_columns_a_row_below_the_brand() {
            let search = areas().search;
            assert_eq!(search, Rect::new(1, BRAND_HEIGHT + 1, SIDEBAR_WIDTH + WORKSPACES_WIDTH - 3, 1));
        }

        #[test]
        fn title_leaves_a_blank_row_below_the_search() {
            assert_eq!(areas().title.y, areas().search.bottom() + 1);
        }

        #[test]
        fn list_leaves_a_blank_row_below_the_title() {
            assert_eq!(list().y, areas().title.bottom() + GAP);
        }

        #[test]
        fn list_ends_before_the_border() {
            assert_eq!(list().right(), SIDEBAR_WIDTH - 1);
        }

        #[test]
        fn list_ends_right_above_the_separator() {
            assert_eq!(list().bottom(), areas().separator.y);
        }

        #[test]
        fn settings_button_sits_right_above_the_quit_button() {
            assert_eq!(areas().settings, Rect::new(0, H - 2, SIDEBAR_WIDTH - 1, 1));
        }

        #[test]
        fn quit_button_takes_the_last_row() {
            assert_eq!(areas().quit, Rect::new(0, H - 1, SIDEBAR_WIDTH - 1, 1));
        }

        #[test]
        fn results_cover_both_columns_below_the_header() {
            let results = areas().results;
            assert_eq!(results, Rect::new(0, HEADER_HEIGHT, SIDEBAR_WIDTH + WORKSPACES_WIDTH - 1, H - HEADER_HEIGHT));
        }
    }

    mod widths {
        use super::*;

        const WIDE: Widths = Widths { projects: 40, workspaces: 30 };

        #[test]
        fn fit_keeps_widths_that_leave_room_for_the_pane() {
            assert_eq!(WIDE.fit(W), WIDE);
        }

        #[test]
        fn fit_shrinks_the_workspaces_column_first() {
            let room = 80 - PANE_PADDING - MIN_PANE_WIDTH;
            assert_eq!(WIDE.fit(80), Widths { projects: 40, workspaces: room - 40 });
        }

        #[test]
        fn fit_shrinks_the_projects_column_once_workspaces_is_at_its_minimum() {
            let room = 60 - PANE_PADDING - MIN_PANE_WIDTH;
            assert_eq!(WIDE.fit(60), Widths { projects: room - MIN_COLUMN_WIDTH, workspaces: MIN_COLUMN_WIDTH });
        }

        #[test]
        fn fit_never_goes_below_the_minimum() {
            let min = Widths { projects: MIN_COLUMN_WIDTH, workspaces: MIN_COLUMN_WIDTH };
            assert_eq!(WIDE.fit(20), min);
        }

        #[test]
        fn dragging_the_projects_border_puts_it_under_the_mouse() {
            assert_eq!(Widths::default().dragged(Border::Projects, 39, W).projects, 40);
        }

        #[test]
        fn dragging_the_workspaces_border_puts_it_under_the_mouse() {
            let widths = Widths::default().dragged(Border::Workspaces, SIDEBAR_WIDTH + 29, W);
            assert_eq!(widths, Widths { projects: SIDEBAR_WIDTH, workspaces: 30 });
        }

        #[rstest]
        #[case::projects(Border::Projects)]
        #[case::workspaces(Border::Workspaces)]
        fn dragging_stops_at_the_minimum_width(#[case] border: Border) {
            let widths = Widths::default().dragged(border, 0, W);
            let width = if border == Border::Projects { widths.projects } else { widths.workspaces };
            assert_eq!(width, MIN_COLUMN_WIDTH);
        }

        #[rstest]
        #[case::projects(Border::Projects)]
        #[case::workspaces(Border::Workspaces)]
        fn dragging_leaves_room_for_the_pane(#[case] border: Border) {
            let widths = Widths::default().dragged(border, W - 1, W);
            assert_eq!(layout(AREA, widths).pane.width, MIN_PANE_WIDTH);
        }

        #[test]
        fn reset_brings_back_the_default_width_of_that_column_only() {
            assert_eq!(WIDE.reset(Border::Projects), Widths { projects: SIDEBAR_WIDTH, workspaces: 30 });
        }
    }

    mod borders {
        use super::*;

        #[test]
        fn the_projects_border_is_the_last_column_of_the_sidebar() {
            assert_eq!(areas().projects_border, Rect::new(SIDEBAR_WIDTH - 1, HEADER_HEIGHT, 1, H - HEADER_HEIGHT));
        }

        #[test]
        fn the_workspaces_border_is_the_last_column_of_the_workspaces_column() {
            let x = SIDEBAR_WIDTH + WORKSPACES_WIDTH - 1;
            assert_eq!(areas().workspaces_border, Rect::new(x, 0, 1, H));
        }

        #[test]
        fn they_follow_the_widths() {
            let widths = Widths { projects: 40, workspaces: 20 };
            let areas = layout(AREA, widths);
            assert_eq!((areas.projects_border.x, areas.workspaces_border.x, areas.pane.x), (39, 59, 61));
        }

        #[rstest]
        #[case::projects(Position::new(SIDEBAR_WIDTH - 1, HEADER_HEIGHT + 2), Some(Border::Projects))]
        #[case::workspaces(Position::new(SIDEBAR_WIDTH + WORKSPACES_WIDTH - 1, 0), Some(Border::Workspaces))]
        #[case::header_has_no_projects_border(Position::new(SIDEBAR_WIDTH - 1, 0), None)]
        #[case::inside_the_list(Position::new(3, HEADER_HEIGHT + 2), None)]
        fn hit_finds_the_border_under_the_mouse(#[case] pos: Position, #[case] expected: Option<Border>) {
            assert_eq!(areas().border_hit(pos), expected);
        }

        fn middle(border: Border) -> Position {
            let r = areas().border(border);
            Position::new(r.x, r.y + r.height / 2)
        }

        #[test]
        fn a_border_is_dim_by_default() {
            let pos = middle(Border::Projects);
            assert_eq!(render(&view(&["~"])).backend().buffer()[pos].fg, Color::DarkGray);
        }

        #[test]
        fn a_border_turns_cyan_on_hover() {
            let pos = middle(Border::Workspaces);
            let v = View { hover: Some(pos), ..view(&["~"]) };
            assert_eq!(render(&v).backend().buffer()[pos].fg, Color::Cyan);
        }

        #[test]
        fn a_border_stays_cyan_while_it_is_dragged() {
            let pos = middle(Border::Projects);
            let v = View { resizing: Some(Border::Projects), ..view(&["~"]) };
            assert_eq!(render(&v).backend().buffer()[pos].fg, Color::Cyan);
        }

        #[test]
        fn a_border_ignores_the_hover_while_an_overlay_is_open() {
            let pos = middle(Border::Workspaces);
            let menu = Overlay::Menu { at: Position::new(80, 1), items: vec!["rename tab".into()] };
            let v = View { hover: Some(pos), overlay: Some(menu), ..view(&["~"]) };
            assert_eq!(render(&v).backend().buffer()[pos].fg, Color::DarkGray);
        }
    }

    mod sidebar_hit {
        use super::*;

        #[rstest]
        #[case::first(3, 0, Some(SidebarHit::Select(0)))]
        #[case::second(3, 1, Some(SidebarHit::Select(1)))]
        #[case::gap_before_the_new_button(3, 2, None)]
        #[case::new_button_after_two_entries(3, 3, Some(SidebarHit::New))]
        fn maps_rows_to_entries(#[case] x: u16, #[case] row: u16, #[case] expected: Option<SidebarHit>) {
            assert_eq!(sidebar_hit(list(), 1, &plain(2), 0, Position::new(x, list().y + row)), expected);
        }

        #[rstest]
        #[case::first(0)]
        #[case::second(1)]
        fn close_button_closes_its_entry(#[case] i: u16) {
            let pos = Position::new(close_x(), list().y + i);
            assert_eq!(sidebar_hit(list(), 1, &plain(2), 0, pos), Some(SidebarHit::Close(usize::from(i))));
        }

        #[rstest]
        #[case::brand(areas().brand.y)]
        #[case::title(areas().title.y)]
        fn header_rows_are_ignored(#[case] y: u16) {
            assert_eq!(sidebar_hit(list(), 1, &plain(1), 0, Position::new(3, y)), None);
        }

        #[test]
        fn pane_is_ignored() {
            assert_eq!(sidebar_hit(list(), 1, &plain(1), 0, Position::new(SIDEBAR_WIDTH + 5, list().y)), None);
        }
    }

    mod groups {
        use super::*;

        const TALL: u16 = 22;

        fn group(name: &str, icon: char, colour: u8) -> GroupEntry {
            GroupEntry { name: name.into(), icon, colour, collapsed: false }
        }

        fn grouped(active: usize, collapsed: bool) -> View<'static> {
            let mut v = View { active, has_project: true, ..view(&["tmp", "api", "web", "cornercase"]) };
            v.groups = vec![group("work", '●', 4), group("oss", '★', 99)];
            v.groups[0].collapsed = collapsed;
            for (p, g) in [(1, 0), (2, 0), (3, 1)] {
                v.projects[p].group = Some(g);
            }
            v
        }

        fn tall_list() -> Rect {
            layout(Rect::new(0, 0, W, TALL), Widths::default()).list
        }

        #[rstest]
        #[case::no_groups(&[None, None], &[], &[SidebarRow::Project(0), SidebarRow::Project(1)])]
        #[case::loose_projects_first(&[Some(0), None], &[false], &[
            SidebarRow::Project(1),
            SidebarRow::Gap,
            SidebarRow::Group(0),
            SidebarRow::Project(0),
        ])]
        #[case::no_gap_without_loose_projects(&[Some(0)], &[false], &[SidebarRow::Group(0), SidebarRow::Project(0)])]
        #[case::collapsed_hides_its_projects(&[Some(0), Some(1)], &[true, false], &[
            SidebarRow::Group(0),
            SidebarRow::Gap,
            SidebarRow::Group(1),
            SidebarRow::Project(1),
        ])]
        #[case::an_empty_group_keeps_its_header(&[None], &[false], &[
            SidebarRow::Project(0),
            SidebarRow::Gap,
            SidebarRow::Group(0),
        ])]
        fn rows(#[case] groups: &[Option<usize>], #[case] collapsed: &[bool], #[case] expected: &[SidebarRow]) {
            assert_eq!(sidebar_rows(groups, collapsed), expected);
        }

        #[rstest]
        #[case::loose_project(0, Some(SidebarHit::Select(0)))]
        #[case::gap(1, None)]
        #[case::header(2, Some(SidebarHit::Group(0)))]
        #[case::grouped_project(3, Some(SidebarHit::Select(1)))]
        fn hit_testing_follows_the_rows(#[case] row: u16, #[case] expected: Option<SidebarHit>) {
            let rows = grouped(0, false).sidebar_rows();
            assert_eq!(sidebar_hit(list(), 1, &rows, 0, Position::new(3, list().y + row)), expected);
        }

        #[test]
        fn the_close_button_of_a_grouped_project_closes_it() {
            let rows = grouped(0, false).sidebar_rows();
            let pos = close_button(list(), 1, &rows, 0, 2).as_position();
            assert_eq!(sidebar_hit(list(), 1, &rows, 0, pos), Some(SidebarHit::Close(2)));
        }

        #[test]
        fn renders_expanded_groups_with_the_active_project_inside() {
            insta::assert_snapshot!(render_sized(&grouped(1, false), W, TALL).backend());
        }

        #[test]
        fn a_collapsed_group_holding_the_active_project_marks_its_header() {
            insta::assert_snapshot!(render_sized(&grouped(1, true), W, TALL).backend());
        }

        #[test]
        fn a_collapsed_group_without_the_active_project_is_not_marked() {
            insta::assert_snapshot!(render_sized(&grouped(0, true), W, TALL).backend());
        }

        #[test]
        fn the_header_takes_the_group_colour() {
            let rows = grouped(0, false).sidebar_rows();
            let header = entry_row(tall_list(), 1, &rows, 0, SidebarRow::Group(1));
            let t = render_sized(&grouped(0, false), W, TALL);
            assert_eq!(t.backend().buffer()[(header.x + 4, header.y)].fg, Color::Indexed(99));
        }

        fn styling(icon: char, colour: u8) -> View<'static> {
            let group = GroupEntry { name: "work".into(), icon, colour, collapsed: false };
            View { overlay: Some(Overlay::GroupStyle(group)), ..grouped(1, false) }
        }

        #[test]
        fn renders_the_icon_and_colour_modal() {
            insta::assert_snapshot!(render(&styling('★', 99)).backend());
        }

        #[test]
        fn the_chosen_icon_is_highlighted() {
            let t = render(&styling('★', 99));
            let at = |i: usize| t.backend().buffer()[style_icon(AREA, i).as_position()].bg;
            assert_eq!((at(7), at(0)), (Color::Cyan, Color::Reset));
        }

        #[test]
        fn each_swatch_shows_its_colour() {
            let t = render(&styling('★', 99));
            let fg = |i: usize| {
                let cell = style_colour(AREA, i);
                t.backend().buffer()[(cell.x + 1, cell.y)].fg
            };
            assert_eq!((0..GROUP_COLOURS.len()).map(fg).collect::<Vec<_>>(), GROUP_COLOURS.map(Color::Indexed));
        }

        #[rstest]
        #[case::first_icon(style_icon(AREA, 0), Some(StyleHit::Icon(0)))]
        #[case::icon_on_the_second_row(style_icon(AREA, 11), Some(StyleHit::Icon(11)))]
        #[case::colour(style_colour(AREA, 5), Some(StyleHit::Colour(5)))]
        #[case::colour_on_the_second_row(style_colour(AREA, 15), Some(StyleHit::Colour(15)))]
        #[case::done(style_done(AREA), Some(StyleHit::Done))]
        #[case::title(Rect { y: form_area(AREA).y, ..style_icon(AREA, 0) }, None)]
        fn style_hits(#[case] cell: Rect, #[case] expected: Option<StyleHit>) {
            assert_eq!(style_hit(AREA, Position::new(cell.right() - 1, cell.y)), expected);
        }

        #[test]
        fn the_cells_fit_a_narrow_terminal() {
            let narrow = Rect::new(0, 0, 40, 20);
            let all =
                (0..GROUP_ICONS.len()).map(|i| style_icon(narrow, i)).chain((0..16).map(|i| style_colour(narrow, i)));
            assert!(all.into_iter().all(|r| r.width >= ICON_CELL));
        }

        #[test]
        fn the_more_counts_skip_the_gaps() {
            let t = render(&View { projects_scroll: 2, ..grouped(0, false) });
            let above = row_text(&t, more_above(list()));
            assert!(above.contains("↑ 1 more"), "{above:?}");
        }
    }

    mod compact {
        use super::*;

        const SMALL: Rect = Rect { x: 0, y: 0, width: 80, height: 30 };

        fn small() -> Areas {
            layout(SMALL, Widths::default())
        }

        fn render_small(view: &View) -> Terminal<TestBackend> {
            let mut t = Terminal::new(TestBackend::new(SMALL.width, SMALL.height)).expect("test backend");
            t.draw(|f| draw(f, view)).expect("draw");
            t
        }

        fn in_a_project(nav: Option<Nav>) -> View<'static> {
            View {
                has_project: true,
                workspaces: vec![
                    WorkspaceEntry { name: "feat/login".into(), tabs: vec!["claude".into(), "nvim".into()], behind: 0 },
                    WorkspaceEntry { name: "main".into(), tabs: vec!["zsh".into()], behind: 0 },
                ],
                active_tab: Some(0),
                nav,
                ..view(&["cornercase", "shop"])
            }
        }

        #[rstest]
        #[case::narrow(COMPACT_WIDTH - 1, true)]
        #[case::wide(COMPACT_WIDTH, false)]
        fn narrow_terminals_get_the_menu_bar(#[case] width: u16, #[case] compact: bool) {
            assert_eq!(layout(Rect::new(0, 0, width, 20), Widths::default()).compact(), compact);
        }

        #[test]
        fn the_pane_takes_everything_under_the_bar() {
            assert_eq!(small().pane, Rect::new(0, COMPACT_PITCH, SMALL.width, SMALL.height - COMPACT_PITCH));
        }

        #[test]
        fn the_search_button_is_the_end_of_the_bar() {
            assert_eq!(
                small().search_button,
                Rect::new(SMALL.width - COMPACT_BUTTON_WIDTH, 0, COMPACT_BUTTON_WIDTH, COMPACT_PITCH)
            );
        }

        #[rstest]
        #[case::closed(None, false, false)]
        #[case::projects(Some(Nav::Projects), true, false)]
        #[case::workspaces(Some(Nav::Workspaces), false, true)]
        fn the_menu_shows_one_column_at_a_time(
            #[case] nav: Option<Nav>,
            #[case] projects: bool,
            #[case] workspaces: bool,
        ) {
            let shown = small().shown(nav);
            assert_eq!((!shown.list.is_empty(), !shown.workspaces_list.is_empty()), (projects, workspaces));
            assert_eq!(!shown.back.is_empty(), workspaces);
        }

        #[test]
        fn wide_terminals_show_both_columns_whatever_the_menu() {
            let areas = layout(AREA, Widths::default());
            assert_eq!(areas.shown(None), areas);
        }

        #[test]
        fn renders_the_bar_over_the_pane() {
            insta::assert_snapshot!(render_small(&in_a_project(None)).backend());
        }

        #[test]
        fn renders_the_workspaces_menu() {
            insta::assert_snapshot!(render_small(&in_a_project(Some(Nav::Workspaces))).backend());
        }

        #[test]
        fn renders_commits_to_pull_in_the_workspaces_menu() {
            let mut v = in_a_project(Some(Nav::Workspaces));
            v.workspaces[0].behind = 12;
            insta::assert_snapshot!(render_small(&v).backend());
        }

        #[test]
        fn renders_the_projects_menu() {
            insta::assert_snapshot!(render_small(&in_a_project(Some(Nav::Projects))).backend());
        }

        fn with_groups(view: View<'static>) -> View<'static> {
            let mut v = View { active: 1, ..view };
            v.groups = vec![GroupEntry { name: "work".into(), icon: '●', colour: 4, collapsed: false }];
            v.projects[1].group = Some(0);
            v
        }

        #[test]
        fn renders_groups_in_the_projects_menu() {
            insta::assert_snapshot!(render_small(&with_groups(in_a_project(Some(Nav::Projects)))).backend());
        }

        #[test]
        fn a_header_is_a_whole_band_to_click() {
            let v = with_groups(in_a_project(Some(Nav::Projects)));
            let rows = v.sidebar_rows();
            let header = entry_row(small().list, COMPACT_PITCH, &rows, 0, SidebarRow::Group(0));
            let bottom = Position::new(header.x + 2, header.bottom() - 1);
            assert_eq!(
                (header.height, sidebar_hit(small().list, COMPACT_PITCH, &rows, 0, bottom)),
                (COMPACT_PITCH, Some(SidebarHit::Group(0)))
            );
        }

        #[test]
        fn the_active_entry_fills_its_whole_band() {
            let t = render_small(&in_a_project(Some(Nav::Projects)));
            let r = entry_row(small().list, COMPACT_PITCH, &plain(2), 0, SidebarRow::Project(0));
            let rows: Vec<Color> = (r.top()..r.bottom()).map(|y| t.backend().buffer()[(r.x + 20, y)].bg).collect();
            assert_eq!(rows, vec![DARK_SURFACE; usize::from(COMPACT_PITCH)]);
        }

        #[test]
        fn the_menu_button_is_a_block_while_the_menu_is_open() {
            let t = render_small(&in_a_project(Some(Nav::Workspaces)));
            let rows: Vec<Color> = (0..COMPACT_PITCH).map(|y| t.backend().buffer()[(1, y)].bg).collect();
            assert_eq!(rows, vec![Color::Cyan; usize::from(COMPACT_PITCH)]);
        }

        #[test]
        fn the_bar_shows_the_brand_without_a_project() {
            let t = render_small(&view(&[]));
            let row: String = (0..SMALL.width).map(|x| t.backend().buffer()[(x, 1)].symbol().to_string()).collect();
            assert!(row.contains("cornercase"), "{row:?}");
        }

        #[test]
        fn the_search_field_takes_the_whole_bar() {
            let search =
                Search { query: "feat".into(), results: Vec::new(), selected: 0, scroll: 0, hint: String::new() };
            let v = View { overlay: Some(Overlay::Search(search)), ..in_a_project(None) };
            let t = render_small(&v);
            let row: String = (0..SMALL.width).map(|x| t.backend().buffer()[(x, 1)].symbol().to_string()).collect();
            assert!(row.starts_with(" ⌕ feat") && !row.contains("claude"), "{row:?}");
        }
    }

    mod sidebar_scroll {
        use super::*;

        fn rows(entries: usize, scroll: usize) -> Rows {
            project_rows(list(), 1, &plain(entries), scroll)
        }

        fn many(n: usize, projects_scroll: usize) -> View<'static> {
            let names: Vec<String> = (0..n).map(|i| format!("p{i}")).collect();
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            View { projects_scroll, ..view(&names) }
        }

        #[test]
        fn scrolled_rows_map_to_later_entries() {
            assert_eq!(sidebar_hit(list(), 1, &plain(20), 3, Position::new(3, list().y)), Some(SidebarHit::Select(3)));
        }

        #[test]
        fn the_row_above_the_new_button_is_not_an_entry_when_overflowing() {
            assert_eq!(sidebar_hit(list(), 1, &plain(20), 0, rows(20, 0).more_below().as_position()), None);
        }

        #[rstest]
        #[case::down(0, 3, 3)]
        #[case::stops_at_the_last_entry(0, 100, 14)]
        #[case::up(5, -3, 2)]
        #[case::stops_at_the_first_entry(2, -3, 0)]
        #[case::a_stale_scroll_is_clamped_first(100, -3, 11)]
        fn the_wheel_moves_within_the_entries(#[case] scroll: usize, #[case] delta: isize, #[case] expected: usize) {
            assert_eq!(rows(20, scroll).scrolled(delta), expected);
        }

        #[test]
        fn nothing_scrolls_when_everything_fits() {
            assert_eq!(rows(3, 0).scrolled(3), 0);
        }

        #[rstest]
        #[case::already_visible(0, 2, 0)]
        #[case::below(0, 10, 5)]
        #[case::above(10, 4, 4)]
        fn reveal_scrolls_as_little_as_it_can(#[case] scroll: usize, #[case] i: usize, #[case] expected: usize) {
            assert_eq!(rows(20, scroll).reveal(i), expected);
        }

        #[test]
        fn hidden_entries_are_counted_above_and_below() {
            assert_eq!(rows(20, 3).hidden(), (0..3, 9..20));
        }

        #[test]
        fn shows_how_many_projects_are_hidden() {
            let t = render(&many(20, 3));
            assert_eq!(row_text(&t, more_above(list())).trim_end(), "  ↑ 3 more");
            assert_eq!(row_text(&t, rows(20, 3).more_below()).trim_end(), "  ↓ 11 more");
        }

        #[test]
        fn shows_no_indicator_when_everything_fits() {
            let t = render(&many(3, 0));
            assert_eq!(row_text(&t, more_above(list())).trim(), "");
            assert_eq!(row_text(&t, rows(20, 0).more_below()).trim(), "");
        }

        #[test]
        fn the_new_button_stays_at_the_bottom_while_scrolled() {
            let t = render(&many(20, 3));
            assert!(row_text(&t, new_project_button(list(), 1, &plain(20))).contains("+ new project"));
        }

        #[test]
        fn workspace_rows_follow_the_scroll() {
            let pos = Position::new(areas().workspaces_list.x + 4, areas().workspaces_list.y);
            assert_eq!(
                workspace_hit(areas().workspaces_list, areas().pitch, &[10], 2, pos),
                Some(WorkspaceHit::Tab(0, 1))
            );
        }

        #[test]
        fn hidden_workspace_rows_count_workspaces_and_tabs_only() {
            let v = View {
                has_project: true,
                workspaces: vec![WorkspaceEntry { name: "main".into(), tabs: vec!["zsh".into(); 10], behind: 0 }],
                active_tab: Some(0),
                ..view(&["cornercase"])
            };
            let t = render(&v);
            let below = workspace_layout(areas().workspaces_list, 1, &[10], 0).more_below();
            assert_eq!(row_text(&t, below).trim_end(), "  ↓ 5 more");
        }
    }

    mod tall_rows {
        use super::*;

        const TALL: Rect = Rect { x: 0, y: 4, width: 40, height: 14 };

        #[test]
        fn each_entry_takes_the_whole_pitch() {
            assert_eq!(project_rows(TALL, 3, &plain(2), 0).item(1), Rect::new(0, 7, 40, 3));
        }

        #[rstest]
        #[case::top(4)]
        #[case::middle(5)]
        #[case::bottom(6)]
        fn any_row_of_an_entry_selects_it(#[case] y: u16) {
            assert_eq!(sidebar_hit(TALL, 3, &plain(2), 0, Position::new(10, y)), Some(SidebarHit::Select(0)));
        }

        #[test]
        fn the_new_button_is_as_tall_as_an_entry() {
            assert_eq!(new_project_button(TALL, 3, &plain(1)), Rect::new(0, 8, 40, 3));
        }

        #[test]
        fn the_gap_between_workspaces_stays_one_row() {
            let second = workspace_row(TALL, 3, &[0, 0], 0, WorkspaceRow::Workspace(1));
            assert_eq!(second.y, TALL.y + 3 + 3 + GAP);
        }

        #[test]
        fn the_wheel_moves_by_entries() {
            assert_eq!(project_rows(TALL, 3, &plain(10), 0).scrolled(100), 7);
        }

        #[test]
        fn reveal_counts_rows_not_entries() {
            assert_eq!(project_rows(TALL, 3, &plain(10), 0).reveal(5), 3);
        }

        #[test]
        fn a_tall_row_gets_a_wider_close_button() {
            assert_eq!(row_close_button(Rect::new(0, 0, 40, 3)).width, COMPACT_CLOSE_WIDTH);
        }
    }

    mod buttons {
        use super::*;

        #[test]
        fn new_button_sticks_to_bottom_when_entries_overflow() {
            assert_eq!(new_project_button(list(), 1, &plain(100)).y, list().bottom() - 1);
        }

        #[test]
        fn new_button_wins_over_entries_when_overflowing() {
            let pos = Position::new(3, list().bottom() - 1);
            assert_eq!(sidebar_hit(list(), 1, &plain(100), 0, pos), Some(SidebarHit::New));
        }

        #[test]
        fn close_button_out_of_view_is_empty() {
            assert!(close_button(list(), 1, &plain(50), 0, 49).is_empty());
        }

        fn with_update(hover: Option<Position>) -> View<'static> {
            View { update: Some("↑ 9.0.0".into()), hover, ..view(&["~"]) }
        }

        fn settings_row(v: &View) -> String {
            let t = render(v);
            let r = areas().settings;
            (r.x..r.right()).map(|x| t.backend().buffer()[(x, r.y)].symbol().to_string()).collect()
        }

        #[test]
        fn an_update_sits_at_the_end_of_the_settings_row() {
            let row = settings_row(&with_update(None));
            assert!(row.starts_with("  settings ") && row.ends_with(" ↑ 9.0.0 "), "{row:?}");
        }

        #[test]
        fn hovering_the_update_leaves_settings_alone() {
            let r = update_button(areas().settings, "↑ 9.0.0");
            let t = render(&with_update(Some(r.as_position())));
            let buffer = t.backend().buffer();
            let settings = areas().settings.as_position().offset(ratatui::layout::Offset { x: 2, y: 0 });
            assert_eq!((buffer[r.as_position()].bg, buffer[settings].bg), (Color::Cyan, Color::Reset));
        }
    }

    mod workspaces_column {
        use super::*;

        fn wlist() -> Rect {
            areas().workspaces_list
        }

        fn at(row: u16) -> Position {
            Position::new(wlist().x + 4, wlist().y + row)
        }

        fn close_at(row: u16) -> Position {
            Position::new(wlist().right() - 2, wlist().y + row)
        }

        fn with_workspaces(active_tab: Option<usize>) -> View<'static> {
            View {
                has_project: true,
                workspaces: vec![
                    WorkspaceEntry { name: "login".into(), tabs: vec!["claude".into(), "nvim".into()], behind: 0 },
                    WorkspaceEntry { name: "main".into(), tabs: vec!["zsh".into()], behind: 0 },
                ],
                active_tab,
                ..view(&["cornercase"])
            }
        }

        fn row_text(v: &View, row: WorkspaceRow) -> String {
            let r = workspace_row(areas().workspaces_list, areas().pitch, &v.tab_counts(), v.workspaces_scroll, row);
            let t = render(v);
            (r.x..r.right()).map(|x| t.backend().buffer()[(x, r.y)].symbol().to_string()).collect()
        }

        #[rstest]
        #[case::workspace(WorkspaceRow::Workspace(0))]
        #[case::tab(WorkspaceRow::Tab(0, 0))]
        fn long_names_keep_their_start(#[case] row: WorkspaceRow) {
            let long = "feature-with-a-very-long-name-that-does-not-fit";
            let mut v = with_workspaces(Some(0));
            v.workspaces[0].name = long.into();
            v.workspaces[0].tabs[0] = long.into();
            let text = row_text(&v, row);
            assert!(text.contains("feature-with-") && text.contains('…'), "{text:?}");
        }

        #[test]
        fn rows_are_each_workspace_then_its_tabs_then_plus_tab() {
            use WorkspaceRow::{Gap, NewTab, Tab, Workspace};
            assert_eq!(
                workspace_rows(&[2, 0]),
                [Workspace(0), Tab(0, 0), Tab(0, 1), NewTab(0), Gap, Workspace(1), NewTab(1)]
            );
        }

        #[rstest]
        #[case::workspace(at(0), Some(WorkspaceHit::Workspace(0)))]
        #[case::workspace_close(close_at(0), Some(WorkspaceHit::CloseWorkspace(0)))]
        #[case::tab(at(1), Some(WorkspaceHit::Tab(0, 0)))]
        #[case::tab_close(close_at(2), Some(WorkspaceHit::CloseTab(0, 1)))]
        #[case::plus_tab(at(3), Some(WorkspaceHit::NewTab(0)))]
        #[case::gap_before_new_workspace(at(4), None)]
        #[case::new_workspace(at(5), Some(WorkspaceHit::NewWorkspace))]
        fn maps_clicks(#[case] pos: Position, #[case] expected: Option<WorkspaceHit>) {
            assert_eq!(workspace_hit(wlist(), 1, &[2], 0, pos), expected);
        }

        #[test]
        fn the_gap_between_workspaces_is_not_a_row() {
            assert_eq!(workspace_hit(wlist(), 1, &[0, 0], 0, at(2)), None);
        }

        #[test]
        fn new_workspace_sticks_to_the_bottom_when_rows_overflow() {
            assert_eq!(new_workspace_button(wlist(), 1, &[100]).y, wlist().bottom() - 1);
        }

        #[test]
        fn rows_under_the_new_workspace_button_are_hidden() {
            let last_visible = usize::from(wlist().height) - 1;
            assert!(workspace_row(wlist(), 1, &[100], 0, WorkspaceRow::Tab(0, last_visible)).is_empty());
        }

        #[test]
        fn renders_the_workspaces_of_the_active_project() {
            insta::assert_snapshot!(render(&with_workspaces(Some(0))).backend());
        }

        #[test]
        fn renders_commits_to_pull_before_the_close_button() {
            let mut v = View { hover: Some(at(0)), ..with_workspaces(Some(0)) };
            v.workspaces[0].behind = 3;
            v.workspaces[1].behind = 1;
            insta::assert_snapshot!(render(&v).backend());
        }

        #[rstest]
        #[case::nothing_to_pull("login", 0, "  login")]
        #[case::some("login", 3, "  login            ↓3")]
        #[case::long_name_is_cut_first("feature-with-a-very-long-name", 3, "  feature-with-a-… ↓3")]
        fn commits_to_pull_sit_at_the_end_of_the_row(#[case] name: &str, #[case] behind: u32, #[case] expected: &str) {
            let mut v = with_workspaces(Some(0));
            v.workspaces[0] = WorkspaceEntry { name: name.into(), behind, ..v.workspaces.remove(0) };
            assert_eq!(row_text(&v, WorkspaceRow::Workspace(0)).trim_end(), expected);
        }

        #[test]
        fn marks_the_active_tab() {
            let pos = Position::new(wlist().x + 2, wlist().y + 1);
            assert_eq!(render(&with_workspaces(Some(0))).backend().buffer()[pos].symbol(), "▌");
        }

        #[test]
        fn plus_tab_is_filled_on_hover() {
            let v = View { hover: Some(at(3)), ..with_workspaces(Some(0)) };
            assert_eq!(render(&v).backend().buffer()[at(3)].bg, Color::Cyan);
        }

        #[test]
        fn an_empty_workspace_says_so_in_the_pane() {
            let v = View {
                workspaces: vec![WorkspaceEntry { name: "main".into(), tabs: Vec::new(), behind: 0 }],
                ..with_workspaces(None)
            };
            let text: String =
                render(&v).backend().buffer().content().iter().map(ratatui::buffer::Cell::symbol).collect();
            assert!(text.contains("no tab open"), "{text}");
        }

        #[test]
        fn nothing_shows_without_a_project() {
            let text: String =
                render(&view(&[])).backend().buffer().content().iter().map(ratatui::buffer::Cell::symbol).collect();
            assert!(!text.contains("new workspace"), "{text}");
        }
    }

    mod dialogs {
        use super::*;

        fn new_workspace_form(on: bool) -> Form {
            Form {
                title: "new workspace",
                label: "name",
                value: "feat/login".into(),
                hint: "in ~/.cornercase/worktrees/cornercase/feat-login".into(),
                toggle: Some(Toggle { label: "with its own worktree", on }),
                note: None,
                submit: "create",
            }
        }

        fn with(overlay: Overlay) -> View<'static> {
            View { overlay: Some(overlay), ..view(&["cornercase"]) }
        }

        #[test]
        fn renders_the_worktree_toggle() {
            insta::assert_snapshot!(render(&with(Overlay::Form(new_workspace_form(true)))).backend());
        }

        #[test]
        fn the_toggle_row_is_hit() {
            let pos = form_toggle(form_area(AREA)).as_position();
            assert_eq!(form_hit(AREA, "create", pos), Some(FormHit::Toggle));
        }

        #[test]
        fn renders_a_confirmation() {
            let confirm = Confirm {
                title: "remove workspace",
                message: "Remove the workspace login and delete its worktree folder ~/wt/login? The branch is kept."
                    .into(),
                note: Some(Note::Error("contains modified or untracked files, use --force to delete it".into())),
                submit: "remove anyway",
            };
            insta::assert_snapshot!(render(&with(Overlay::Confirm(confirm))).backend());
        }

        #[test]
        fn renders_an_update_with_its_notes() {
            let notes = (1..=40).map(|i| Line::from(format!("• change {i}"))).collect();
            let update = Update {
                message: "cornercase 9.0.0 is out (you have 0.1.0). Updating replaces ~/.local/bin/cornercase; \
                    your terminals keep running until you restart."
                    .into(),
                notes,
                scroll: 30,
                note: Some(Note::Busy("downloading…")),
                submit: "update",
            };
            insta::assert_snapshot!(render(&with(Overlay::Update(update))).backend());
        }
    }

    mod truncate_left {
        use super::*;

        #[rstest]
        #[case::fits("short", 10, "short")]
        #[case::keeps_the_end("/a/b/c/project", 8, "…project")]
        #[case::counts_chars_not_bytes("ñandú/añil", 5, "…añil")]
        #[case::keeps_a_single_ellipsis("x › …/b/project", 12, "…/b/project")]
        fn shortens_from_the_left(#[case] input: &str, #[case] max: usize, #[case] expected: &str) {
            assert_eq!(truncate_left(input, max), expected);
        }
    }

    mod truncate_right {
        use super::*;

        #[rstest]
        #[case::fits("short", 10, "short")]
        #[case::keeps_the_start("feature/login-page", 8, "feature…")]
        #[case::counts_chars_not_bytes("ñandú/añil", 5, "ñand…")]
        #[case::keeps_a_single_ellipsis("shop › #482 empty addr… › bash", 24, "shop › #482 empty addr…")]
        fn shortens_from_the_right(#[case] input: &str, #[case] max: usize, #[case] expected: &str) {
            assert_eq!(truncate_right(input, max), expected);
        }
    }

    mod folder_name {
        use super::*;

        #[rstest]
        #[case::last_component("/home/ana/projects/cornercase", Some("/home/ana"), "cornercase")]
        #[case::home_itself("/home/ana", Some("/home/ana"), "~")]
        #[case::no_home("/tmp", None, "tmp")]
        #[case::root("/", Some("/home/ana"), "/")]
        fn shows_only_the_current_folder(#[case] path: &str, #[case] home: Option<&str>, #[case] expected: &str) {
            assert_eq!(folder_name(Path::new(path), home.map(Path::new)), expected);
        }
    }

    mod draw {
        use super::*;

        #[rstest]
        #[case::icon(Position::new(1, 0))]
        #[case::icon_stem(Position::new(1, 1))]
        #[case::first_letter(Position::new(6, 0))]
        fn brand_uses_the_brand_color(#[case] pos: Position) {
            assert_eq!(render(&view(&["~"])).backend().buffer()[pos].fg, BRAND_COLOR);
        }

        #[test]
        fn renders_sidebar_with_active_entry() {
            let v = View { active: 1, ..view(&["cornercase", "tmp"]) };
            insta::assert_snapshot!(render(&v).backend());
        }

        #[test]
        fn shows_the_workspace_count_next_to_the_name() {
            let mut v = View { active: 1, ..view(&["cornercase", "api", "tmp"]) };
            v.projects[0].workspaces = 3;
            v.projects[2].workspaces = 0;
            insta::assert_snapshot!(render(&v).backend());
        }

        #[test]
        fn a_long_name_is_cut_before_the_count() {
            let v = View {
                projects: vec![ProjectEntry { name: "a".repeat(40), workspaces: 12, group: None }],
                ..view(&[])
            };
            let row: String =
                (0..list().width).map(|x| render(&v).backend().buffer()[(x, list().y)].symbol().to_string()).collect();
            assert!(row.contains("aaa… (12)"), "{row:?}");
        }

        #[test]
        fn truncates_long_names() {
            let v = view(&["a-folder-with-a-really-long-name-that-does-not-fit"]);
            insta::assert_snapshot!(render(&v).backend());
        }

        #[test]
        fn renders_active_screen_in_pane() {
            let snap = screen(b"$ echo hello\r\nhello\r\n$ ");
            let v = View { tab: Some(single(snap)), ..view(&["~"]) };
            insta::assert_snapshot!(render(&v).backend());
        }

        #[test]
        fn places_cursor_where_the_inner_terminal_has_it() {
            let snap = screen(b"$ echo hello\r\nhello\r\n$ ");
            let v = View { tab: Some(single(snap)), ..view(&["~"]) };
            let mut t = render(&v);
            assert_eq!(t.get_cursor_position().expect("cursor"), Position::new(areas().pane.x + 2, 2));
        }
    }

    mod splits {
        use ratatui::style::Modifier;

        use super::*;
        use crate::split::Dir;

        fn three(dim_inactive: bool) -> TabView {
            let mut layout = Node::Leaf(0);
            layout.split(0, Dir::Right, 1);
            layout.split(1, Dir::Down, 2);
            let screens = vec![screen(b"$ left"), screen(b"$ top"), screen(b"$ bottom")];
            TabView { layout, screens, active: 1, dim_inactive, dragging: None }
        }

        fn style_at(v: &View, at: Position) -> Style {
            render(v).backend().buffer()[(at.x, at.y)].style()
        }

        fn first_cell(v: &View, i: usize) -> Position {
            let tab = v.tab.as_ref().expect("a tab");
            tab.layout.panes(areas().pane)[i].1.as_position()
        }

        #[test]
        fn panes_are_drawn_with_dividers_that_join() {
            let v = View { tab: Some(three(true)), ..view(&["~"]) };
            insta::assert_snapshot!(render(&v).backend());
        }

        #[test]
        fn inactive_panes_are_dimmed() {
            let v = View { tab: Some(three(true)), ..view(&["~"]) };
            let at = first_cell(&v, 0);
            assert!(style_at(&v, at).add_modifier.contains(Modifier::DIM));
        }

        #[test]
        fn the_active_pane_is_not_dimmed() {
            let v = View { tab: Some(three(true)), ..view(&["~"]) };
            let at = first_cell(&v, 1);
            assert!(!style_at(&v, at).add_modifier.contains(Modifier::DIM));
        }

        #[test]
        fn dimming_can_be_turned_off() {
            let v = View { tab: Some(three(false)), ..view(&["~"]) };
            let at = first_cell(&v, 0);
            assert!(!style_at(&v, at).add_modifier.contains(Modifier::DIM));
        }

        #[test]
        fn the_cursor_is_in_the_active_pane() {
            let v = View { tab: Some(three(true)), ..view(&["~"]) };
            let top = first_cell(&v, 1);
            assert_eq!(render(&v).get_cursor_position().expect("cursor"), Position::new(top.x + 5, top.y));
        }

        #[test]
        fn a_hovered_divider_turns_cyan() {
            let tab = three(true);
            let line = tab.layout.dividers(areas().pane)[0].line;
            let at = Position::new(line.x, line.y + 1);
            let v = View { tab: Some(tab), hover: Some(at), ..view(&["~"]) };
            assert_eq!(style_at(&v, at).fg, Some(Color::Cyan));
        }
    }

    mod toast {
        use super::*;

        #[test]
        fn sits_in_the_bottom_right_corner() {
            let r = toast_area(AREA, "copied to clipboard");
            assert_eq!((r.right(), r.bottom(), r.height), (W - 1, H - 1, 3));
        }

        #[test]
        fn fits_a_small_screen() {
            let small = Rect::new(0, 0, 10, 2);
            assert_eq!(toast_area(small, "copied to clipboard"), small);
        }

        #[test]
        fn renders_over_the_pane() {
            let snap = screen(b"$ echo hello\r\nhello\r\n$ ");
            let v = View { tab: Some(single(snap)), toast: Some("copied to clipboard"), ..view(&["~"]) };
            insta::assert_snapshot!(render(&v).backend());
        }
    }

    mod overlay {
        use super::*;

        fn form(note: Option<Note>) -> Form {
            Form {
                title: "new worktree",
                label: "branch",
                value: "feat/login".into(),
                hint: "in ~/.cornercase/worktrees/cornercase/feat-login".into(),
                toggle: None,
                note,
                submit: "create",
            }
        }

        fn with(overlay: Overlay) -> View<'static> {
            View { overlay: Some(overlay), ..view(&["cornercase"]) }
        }

        #[test]
        fn renders_the_menu_where_it_was_opened() {
            let v = with(Overlay::Menu { at: Position::new(4, 4), items: vec!["new worktree".into()] });
            insta::assert_snapshot!(render(&v).backend());
        }

        #[test]
        fn menu_stays_inside_the_screen() {
            let menu = menu_area(AREA, Position::new(W - 1, H - 1), &["new worktree"]);
            assert_eq!((menu.right(), menu.bottom()), (W, H));
        }

        #[test]
        fn menu_item_is_highlighted_on_hover() {
            let at = Position::new(4, 4);
            let item = menu_item(menu_area(AREA, at, &["new worktree"]), 0).as_position();
            let v = View { hover: Some(item), ..with(Overlay::Menu { at, items: vec!["new worktree".into()] }) };
            assert_eq!(render(&v).backend().buffer()[item].bg, Color::Cyan);
        }

        #[test]
        fn buttons_behind_the_menu_are_not_highlighted() {
            let at = Position::new(4, list().y);
            let new = new_project_button(list(), 1, &plain(1));
            let over_new = Position::new(menu_area(AREA, at, &["new worktree"]).x + 2, new.y);
            let v = View { hover: Some(over_new), ..with(Overlay::Menu { at, items: vec!["new worktree".into()] }) };
            assert_eq!(render(&v).backend().buffer()[new.as_position()].bg, Color::Reset);
        }

        #[test]
        fn sidebar_buttons_are_not_highlighted_behind_a_form() {
            let quit = areas().quit.as_position();
            let v = View { hover: Some(quit), ..with(Overlay::Form(form(None))) };
            assert_eq!(render(&v).backend().buffer()[quit].bg, Color::Reset);
        }

        #[test]
        fn renders_a_form() {
            insta::assert_snapshot!(render(&with(Overlay::Form(form(None)))).backend());
        }

        #[test]
        fn renders_a_form_error() {
            let v = with(Overlay::Form(form(Some(Note::Error("a branch named 'feat/login' already exists".into())))));
            insta::assert_snapshot!(render(&v).backend());
        }

        #[test]
        fn puts_the_cursor_after_the_input() {
            let mut t = render(&with(Overlay::Form(form(None))));
            let input = form_rows(form_area(AREA))[1];
            let expected = input.x + u16::try_from(INPUT_PROMPT.chars().count() + "feat/login".len()).expect("width");
            assert_eq!(t.get_cursor_position().expect("cursor"), Position::new(expected, input.y));
        }

        #[test]
        fn hides_the_pane_cursor_behind_an_overlay() {
            let snap = screen(b"$ ");
            let v = View { tab: Some(single(snap)), ..with(Overlay::Form(form(Some(Note::Busy("creating…"))))) };
            assert!(!render(&v).backend().cursor_visible());
        }

        #[rstest]
        #[case::submit(0, Some(FormHit::Submit))]
        #[case::cancel(1, Some(FormHit::Cancel))]
        fn buttons_are_hit(#[case] which: usize, #[case] expected: Option<FormHit>) {
            let pos = form_buttons(form_area(AREA), "create")[which].as_position();
            assert_eq!(form_hit(AREA, "create", pos), expected);
        }

        #[test]
        fn the_rest_of_the_form_is_not_a_button() {
            assert_eq!(form_hit(AREA, "create", form_area(AREA).as_position()), None);
        }
    }

    mod picker {
        use super::*;

        fn entry(name: &str, branch: Option<&str>) -> Entry {
            Entry { name: name.into(), branch: branch.map(str::to_string) }
        }

        fn picker(selected: Option<usize>) -> Picker {
            Picker {
                title: "new workspace",
                path: "~/projects/".into(),
                filter: String::new(),
                items: vec![entry("..", None), entry("cornercase", Some("main")), entry("notes", None)],
                selected,
                scroll: 0,
                hint: "enter opens ~/projects".into(),
                error: None,
                submit: "open",
            }
        }

        fn with(picker: Picker) -> View<'static> {
            View { overlay: Some(Overlay::Picker(picker)), ..view(&["cornercase"]) }
        }

        fn item(i: usize) -> Position {
            picker_item(picker_area(AREA), 3, 0, i).as_position()
        }

        #[test]
        fn renders_the_folders_with_their_branch() {
            insta::assert_snapshot!(render(&with(picker(None))).backend());
        }

        #[test]
        fn says_when_nothing_matches() {
            let p = Picker { filter: "zzz".into(), items: Vec::new(), hint: String::new(), ..picker(None) };
            insta::assert_snapshot!(render(&with(p)).backend());
        }

        #[test]
        fn highlights_the_selected_folder() {
            assert_eq!(render(&with(picker(Some(1)))).backend().buffer()[item(1)].bg, Color::Cyan);
        }

        #[test]
        fn highlights_the_hovered_folder() {
            let v = View { hover: Some(item(2)), ..with(picker(None)) };
            assert_eq!(render(&v).backend().buffer()[item(2)].bg, Color::Cyan);
        }

        #[test]
        fn puts_the_cursor_after_the_path() {
            let mut t = render(&with(picker(None)));
            let input = picker_rows(picker_area(AREA))[0];
            let expected = input.x + u16::try_from(INPUT_PROMPT.chars().count() + "~/projects/".len()).expect("width");
            assert_eq!(t.get_cursor_position().expect("cursor"), Position::new(expected, input.y));
        }

        #[rstest]
        #[case::folder(item(1), Some(PickerHit::Item(1)))]
        #[case::below_the_last_folder(item(2).offset(ratatui::layout::Offset { x: 0, y: 1 }), None)]
        #[case::submit(picker_buttons(picker_area(AREA), "open")[0].as_position(), Some(PickerHit::Submit))]
        #[case::cancel(picker_buttons(picker_area(AREA), "open")[1].as_position(), Some(PickerHit::Cancel))]
        #[case::the_input(picker_rows(picker_area(AREA))[0].as_position(), None)]
        fn maps_clicks(#[case] pos: Position, #[case] expected: Option<PickerHit>) {
            assert_eq!(picker_hit(AREA, "open", 3, 0, pos), expected);
        }

        #[test]
        fn scrolled_rows_map_to_later_folders() {
            let list = picker_list(picker_area(AREA));
            let items = usize::from(list.height) + 5;
            assert_eq!(picker_hit(AREA, "open", items, 2, list.as_position()), Some(PickerHit::Item(2)));
        }

        #[test]
        fn scroll_never_leaves_empty_rows_at_the_bottom() {
            let list = picker_list(picker_area(AREA));
            let items = usize::from(list.height) + 5;
            assert_eq!(picker_hit(AREA, "open", items, 100, list.as_position()), Some(PickerHit::Item(5)));
        }

        #[test]
        fn folders_scrolled_out_have_no_row() {
            assert!(picker_item(picker_area(AREA), 50, 10, 3).is_empty());
        }
    }

    mod settings {
        use super::*;

        fn row(section: &'static str, label: &str, value: &str, note: &str) -> SettingsRow {
            SettingsRow {
                section,
                label: label.into(),
                value: value.into(),
                note: note.into(),
                dangerous: false,
                removable: false,
                movable: false,
            }
        }

        const TABS: [&str; 4] = ["Worktrees", "Agents", "Issues", "TUI"];

        fn source(label: &str, note: &str) -> SettingsRow {
            SettingsRow { movable: true, ..row("Sources shown", label, "", note) }
        }

        fn settings(cursor: usize) -> Settings {
            Settings {
                tabs: TABS.to_vec(),
                tab: 2,
                rows: vec![
                    SettingsRow { removable: true, ..row("Accounts", "Shortcut API token", "@ana in acme", "saved") },
                    row("Accounts", "Linear API key", "not connected", "enter pastes one"),
                    source("[x] All", "every source together"),
                    source("[x] GitHub", ""),
                    source("[ ] Linear", ""),
                ],
                cursor,
                edit: None,
                pick: None,
                note: None,
                hint: "enter changes the selected setting".into(),
                submit: "done",
            }
        }

        fn with(settings: Settings) -> View<'static> {
            View { overlay: Some(Overlay::Settings(settings)), ..view(&["shop"]) }
        }

        fn sections() -> Vec<&'static str> {
            settings(0).sections()
        }

        fn layout<'a>(sections: &'a [&'a str]) -> SettingsLayout<'a> {
            SettingsLayout {
                sections,
                tabs: &TABS,
                removable: &[true, false, false, false, false],
                movable: &[false, false, true, true, true],
                cursor: 0,
                pick: None,
            }
        }

        #[test]
        fn renders_the_sections_and_rows() {
            insta::assert_snapshot!(render(&with(settings(1))).backend());
        }

        #[test]
        fn renders_a_pick() {
            let pick = SettingsPick {
                title: "How should claude start?".into(),
                filter: "pl".into(),
                items: vec![
                    ("plan".into(), "--permission-mode plan".into(), false),
                    ("skip permissions (dangerous)".into(), "--dangerously-skip-permissions".into(), true),
                ],
                selected: Some(0),
                scroll: 0,
            };
            let agents = Settings { tab: 1, rows: vec![row("Agent", "default agent", "claude", "")], ..settings(0) };
            insta::assert_snapshot!(render(&with(Settings { pick: Some(pick), ..agents })).backend());
        }

        #[test]
        fn headers_go_before_the_first_row_of_each_section() {
            assert_eq!(
                settings_lines(&["Folder", "Accounts", "Accounts"]),
                [
                    SettingsLine::Header(0),
                    SettingsLine::Row(0),
                    SettingsLine::Blank,
                    SettingsLine::Header(1),
                    SettingsLine::Row(1),
                    SettingsLine::Row(2)
                ]
            );
        }

        #[test]
        fn a_row_without_a_section_has_no_header() {
            assert_eq!(
                settings_lines(&["", "Agent"]),
                [SettingsLine::Row(0), SettingsLine::Blank, SettingsLine::Header(1), SettingsLine::Row(1)]
            );
        }

        #[test]
        fn the_active_tab_is_filled() {
            let t = render(&with(settings(0)));
            let issues = settings_tabs(settings_area(AREA), &TABS)[2];
            assert_eq!(t.backend().buffer()[issues.as_position()].bg, Color::Cyan);
        }

        #[test]
        fn scrolls_to_keep_the_selected_row_visible() {
            let sections = vec!["A"; 40];
            let at = settings_scroll(&sections, 30, 10);
            let line = settings_lines(&sections).iter().position(|l| *l == SettingsLine::Row(30)).expect("row");
            assert!(line >= at && line < at + 10, "line {line} scroll {at}");
        }

        #[test]
        fn a_dangerous_value_is_red() {
            let claude = SettingsRow {
                dangerous: true,
                ..row("How each agent starts", "claude", "skip permissions (dangerous)", "the default agent")
            };
            let r = settings_row(AREA, &["How each agent starts"], 0, 0);
            let t = render(&with(Settings { tab: 1, rows: vec![claude], ..settings(0) }));
            let cell =
                (r.x..r.right()).map(|x| &t.backend().buffer()[(x, r.y)]).find(|c| c.symbol() == "k").expect("text");
            assert_eq!(cell.fg, Color::Red);
        }

        #[test]
        fn the_edit_line_puts_the_cursor_after_the_value() {
            let edit = SettingsEdit { label: "Linear API key".into(), value: "•••".into() };
            let mut t = render(&with(Settings { edit: Some(edit), ..settings(2) }));
            let row = settings_edit(settings_area(AREA));
            let x = row.x
                + u16::try_from(INPUT_PROMPT.chars().count() + "Linear API key: ".chars().count() + 3).expect("width");
            assert_eq!(t.get_cursor_position().expect("cursor"), Position::new(x, row.y));
        }

        #[rstest]
        #[case::a_tab(settings_tabs(settings_area(AREA), &TABS)[1].as_position(), Some(SettingsHit::Tab(1)))]
        #[case::a_row(settings_row(AREA, &sections(), 0, 1).as_position(), Some(SettingsHit::Row(1)))]
        #[case::remove(settings_remove(settings_row(AREA, &sections(), 0, 0)).as_position(), Some(SettingsHit::Remove(0)))]
        #[case::move_up(settings_moves(settings_row(AREA, &sections(), 0, 3))[0].as_position(), Some(SettingsHit::MoveUp(3)))]
        #[case::move_down(settings_moves(settings_row(AREA, &sections(), 0, 3))[1].as_position(), Some(SettingsHit::MoveDown(3)))]
        #[case::done(settings_done(settings_area(AREA)).as_position(), Some(SettingsHit::Done))]
        #[case::a_header(settings_body(settings_area(AREA)).as_position(), None)]
        fn maps_clicks(#[case] pos: Position, #[case] expected: Option<SettingsHit>) {
            let sections = sections();
            assert_eq!(settings_hit(AREA, &layout(&sections), pos), expected);
        }

        #[test]
        fn a_click_in_a_pick_chooses_that_item() {
            let sections = sections();
            let list = settings_pick_list(settings_area(AREA));
            let pos = Position::new(list.x + 2, list.y + 1);
            let pick = SettingsLayout { pick: Some((3, 0)), ..layout(&sections) };
            assert_eq!(settings_hit(AREA, &pick, pos), Some(SettingsHit::Pick(1)));
        }
    }

    mod issues {
        use super::*;

        const TABS: [&str; 4] = ["All", "GitHub", "Shortcut", "Linear"];
        const TOGGLES: [&str; 2] = ["closed", "mine"];

        fn row(key: &str, title: &str, meta: &str) -> IssueRow {
            IssueRow { key: key.into(), title: title.into(), meta: meta.into() }
        }

        fn issues(body: IssuesBody) -> Issues {
            Issues {
                title: "issues · shop".into(),
                tabs: TABS.to_vec(),
                tab: 0,
                toggles: TOGGLES.map(String::from).to_vec(),
                on: vec![false, true],
                body,
                note: None,
                hint: "enter reads #482 · start works on it in its own worktree, on issue-482-returns".into(),
                buttons: vec!["start", "cancel"],
            }
        }

        fn list(selected: Option<usize>) -> IssuesBody {
            IssuesBody::List {
                filter: String::new(),
                items: vec![
                    row("#482", "Returns page crashes on empty address", "bug · ana · 3d"),
                    row("sc-48", "Dark mode", "In Review · luis · 2mo"),
                ],
                selected,
                scroll: 0,
                empty: "no open issues".into(),
            }
        }

        fn with(issues: Issues) -> View<'static> {
            View { overlay: Some(Overlay::Issues(issues)), ..view(&["shop"]) }
        }

        fn item(i: usize) -> Position {
            list_item(issues_list(issues_area(AREA)), 2, 0, i).as_position()
        }

        fn text(v: &View) -> String {
            render(v).backend().buffer().content().iter().map(ratatui::buffer::Cell::symbol).collect()
        }

        #[test]
        fn renders_the_tabs_toggles_and_issues() {
            insta::assert_snapshot!(render(&with(issues(list(Some(0))))).backend());
        }

        #[test]
        fn renders_the_token_form() {
            let body = IssuesBody::Token {
                label: "API token",
                input: "•••".into(),
                help: vec!["Connect Shortcut.".into(), String::new(), "Paste a token.".into()],
            };
            let i = Issues { tab: 2, buttons: vec!["connect", "cancel"], ..issues(body) };
            insta::assert_snapshot!(render(&with(i)).backend());
        }

        #[test]
        fn renders_an_issue_from_its_scroll() {
            let lines = (0..40).map(|n| Line::from(format!("line {n}"))).collect();
            let i = Issues { buttons: vec!["start", "back"], ..issues(IssuesBody::Detail { lines, scroll: 5 }) };
            let first = issue_detail(issues_area(AREA));
            let t = render(&with(i));
            let row: String =
                (first.x..first.right()).map(|x| t.backend().buffer()[(x, first.y)].symbol().to_string()).collect();
            assert!(row.starts_with("line 5"), "{row:?}");
        }

        #[test]
        fn the_active_tab_is_filled() {
            let i = issues(list(None));
            let tab = issue_tabs(issues_area(AREA), &TABS)[0].as_position();
            assert_eq!(render(&with(i)).backend().buffer()[tab].bg, Color::Cyan);
        }

        #[test]
        fn a_toggle_shows_whether_it_is_on() {
            let t = text(&with(issues(list(None))));
            assert!(t.contains("[ ] closed") && t.contains("[x] mine"), "{t}");
        }

        #[test]
        fn highlights_the_selected_issue() {
            assert_eq!(render(&with(issues(list(Some(1))))).backend().buffer()[item(1)].bg, Color::Cyan);
        }

        #[test]
        fn highlights_the_hovered_issue() {
            let v = View { hover: Some(item(1)), ..with(issues(list(None))) };
            assert_eq!(render(&v).backend().buffer()[item(1)].bg, Color::Cyan);
        }

        #[test]
        fn says_why_the_list_is_empty() {
            let body = IssuesBody::List {
                filter: "zzz".into(),
                items: Vec::new(),
                selected: None,
                scroll: 0,
                empty: "no matches".into(),
            };
            assert!(text(&with(issues(body))).contains("no matches"));
        }

        #[test]
        fn an_error_replaces_the_hint() {
            let i = Issues { note: Some(Note::Error("no git remotes found".into())), ..issues(list(None)) };
            let t = text(&with(i));
            assert!(t.contains("no git remotes found") && !t.contains("enter reads"), "{t}");
        }

        #[rstest]
        #[case::tab(issue_tabs(issues_area(AREA), &TABS)[3].as_position(), Some(IssuesHit::Tab(3)))]
        #[case::toggle(issue_toggles(issues_area(AREA), &TOGGLES)[1].as_position(), Some(IssuesHit::Toggle(1)))]
        #[case::issue(item(1), Some(IssuesHit::Item(1)))]
        #[case::below_the_last(item(1).offset(ratatui::layout::Offset { x: 0, y: 1 }), None)]
        #[case::start(issue_buttons(issues_area(AREA), &["start", "cancel"])[0].as_position(), Some(IssuesHit::Button(0)))]
        #[case::cancel(issue_buttons(issues_area(AREA), &["start", "cancel"])[1].as_position(), Some(IssuesHit::Button(1)))]
        fn maps_clicks(#[case] pos: Position, #[case] expected: Option<IssuesHit>) {
            assert_eq!(issues_hit(AREA, &TABS, &TOGGLES, &["start", "cancel"], 2, 0, pos), expected);
        }

        #[test]
        fn buttons_end_at_the_right_edge_in_order() {
            let b = issue_buttons(issues_area(AREA), &["start", "disconnect", "cancel"]);
            assert!(
                b[0].right() < b[1].x
                    && b[1].right() < b[2].x
                    && b[2].right() == issues_rows(issues_area(AREA))[4].right()
            );
        }

        #[test]
        fn the_button_shows_only_with_a_project() {
            let shown = |v: &View| text(v).contains(ISSUES_LABEL);
            let project = View { has_project: true, issues: true, ..view(&["shop"]) };
            assert_eq!((shown(&view(&[])), shown(&project)), (false, true));
        }

        #[test]
        fn the_button_sits_level_with_settings() {
            assert_eq!(areas().issues.y, areas().settings.y);
        }

        #[test]
        fn the_button_is_filled_on_hover() {
            let pos = areas().issues.as_position().offset(ratatui::layout::Offset { x: 2, y: 0 });
            let v = View { has_project: true, issues: true, hover: Some(pos), ..view(&["shop"]) };
            assert_eq!(render(&v).backend().buffer()[pos].bg, Color::Cyan);
        }
    }

    mod search {
        use super::*;

        fn results() -> Rect {
            areas().results
        }

        fn search(query: &str, results: Vec<ResultRow>) -> Search {
            Search { query: query.into(), results, selected: 0, scroll: 0, hint: "enter goes to feat/search".into() }
        }

        fn row(name: &str, context: &str) -> ResultRow {
            ResultRow { name: name.into(), context: context.into() }
        }

        fn found() -> Vec<ResultRow> {
            vec![row("feat/search", "notes"), row("nvim", "notes › feat/search")]
        }

        fn with(search: Search) -> View<'static> {
            View { overlay: Some(Overlay::Search(search)), ..view(&["cornercase", "notes"]) }
        }

        fn item(i: usize) -> Position {
            result_item(results(), 2, 0, i).as_position().offset(ratatui::layout::Offset { x: 2, y: 0 })
        }

        #[test]
        fn the_bar_shows_a_placeholder_when_closed() {
            let bar = areas().search;
            let text: String = (bar.x..bar.right())
                .map(|x| render(&view(&[])).backend().buffer()[(x, bar.y)].symbol().to_string())
                .collect();
            assert!(text.contains(SEARCH_PLACEHOLDER), "{text:?}");
        }

        #[test]
        fn renders_the_results_over_both_columns() {
            insta::assert_snapshot!(render(&with(search("feat", found()))).backend());
        }

        #[test]
        fn an_empty_query_keeps_the_columns() {
            insta::assert_snapshot!(render(&with(search("", Vec::new()))).backend());
        }

        #[test]
        fn says_when_nothing_matches() {
            let text: String = render(&with(search("zzz", Vec::new())))
                .backend()
                .buffer()
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect();
            assert!(text.contains("no matches"), "{text}");
        }

        #[test]
        fn highlights_the_selected_result() {
            assert_eq!(render(&with(search("feat", found()))).backend().buffer()[item(0)].bg, Color::Cyan);
        }

        #[test]
        fn highlights_the_hovered_result() {
            let v = View { hover: Some(item(1)), ..with(search("feat", found())) };
            assert_eq!(render(&v).backend().buffer()[item(1)].bg, Color::Cyan);
        }

        #[test]
        fn marks_the_matching_letters() {
            let s = Search { selected: 1, ..search("feat", found()) };
            assert_eq!(render(&with(s)).backend().buffer()[item(0)].fg, Color::Cyan);
        }

        #[test]
        fn puts_the_cursor_after_the_query() {
            let mut t = render(&with(search("feat", found())));
            let bar = areas().search;
            let x = bar.x + u16::try_from(SEARCH_ICON.chars().count() + "feat".len()).expect("width");
            assert_eq!(t.get_cursor_position().expect("cursor"), Position::new(x, bar.y));
        }

        #[rstest]
        #[case::first(0, Some(0))]
        #[case::second(1, Some(1))]
        #[case::below_the_last(2, None)]
        fn maps_clicks(#[case] row: u16, #[case] expected: Option<usize>) {
            let list = results_list(results());
            assert_eq!(result_hit(results(), 2, 0, Position::new(list.x + 3, list.y + row)), expected);
        }

        #[test]
        fn scrolled_rows_map_to_later_results() {
            let list = results_list(results());
            let items = usize::from(list.height) + 5;
            assert_eq!(result_hit(results(), items, 3, list.as_position()), Some(3));
        }
    }

    mod display_path {
        use super::*;

        #[rstest]
        #[case::under_home("/home/ana/.cornercase/w", "~/.cornercase/w")]
        #[case::home_itself("/home/ana", "~")]
        #[case::elsewhere("/srv/w", "/srv/w")]
        fn shows_home_as_tilde(#[case] path: &str, #[case] expected: &str) {
            assert_eq!(display_path(Path::new(path), Some(Path::new("/home/ana"))), expected);
        }
    }

    mod hover {
        use super::*;

        fn close_pos() -> Position {
            Position::new(close_x(), list().y)
        }

        fn new_pos() -> Position {
            Position::new(3, new_project_button(list(), 1, &plain(1)).y)
        }

        #[test]
        fn close_button_is_hidden_by_default() {
            assert_eq!(render(&view(&["~"])).backend().buffer()[close_pos()].symbol(), " ");
        }

        #[test]
        fn close_button_shows_dim_while_the_row_is_hovered() {
            let v = View { hover: Some(Position::new(list().x + 3, list().y)), ..view(&["~"]) };
            let cell = render(&v).backend().buffer()[close_pos()].clone();
            assert_eq!((cell.symbol(), cell.fg), ("×", Color::DarkGray));
        }

        #[test]
        fn the_active_entry_has_a_background() {
            let pos = Position::new(list().x + 3, list().y);
            assert_eq!(render(&view(&["~", "tmp"])).backend().buffer()[pos].bg, DARK_SURFACE);
        }

        #[test]
        fn the_surface_is_light_on_a_light_theme() {
            let pos = Position::new(list().x + 3, list().y);
            assert_eq!(render(&View { light: true, ..view(&["~"]) }).backend().buffer()[pos].bg, LIGHT_SURFACE);
        }

        #[test]
        fn close_button_turns_red_on_hover() {
            let v = View { hover: Some(close_pos()), ..view(&["~"]) };
            assert_eq!(render(&v).backend().buffer()[close_pos()].fg, Color::Red);
        }

        #[test]
        fn new_button_has_no_background_by_default() {
            assert_eq!(render(&view(&["~"])).backend().buffer()[new_pos()].bg, Color::Reset);
        }

        fn quit_pos() -> Position {
            areas().quit.as_position().offset(ratatui::layout::Offset { x: 2, y: 0 })
        }

        #[test]
        fn quit_button_is_dim_by_default() {
            assert_eq!(render(&view(&["~"])).backend().buffer()[quit_pos()].bg, Color::Reset);
        }

        fn settings_pos() -> Position {
            areas().settings.as_position().offset(ratatui::layout::Offset { x: 2, y: 0 })
        }

        #[test]
        fn settings_button_is_dim_by_default() {
            assert_eq!(render(&view(&["~"])).backend().buffer()[settings_pos()].bg, Color::Reset);
        }

        #[test]
        fn settings_button_is_filled_on_hover() {
            let v = View { hover: Some(settings_pos()), ..view(&["~"]) };
            assert_eq!(render(&v).backend().buffer()[settings_pos()].bg, Color::Cyan);
        }

        #[test]
        fn quit_button_turns_red_on_hover() {
            let v = View { hover: Some(quit_pos()), ..view(&["~"]) };
            assert_eq!(render(&v).backend().buffer()[quit_pos()].bg, Color::Red);
        }

        #[test]
        fn new_button_is_filled_on_hover() {
            let v = View { hover: Some(new_pos()), ..view(&["~"]) };
            assert_eq!(render(&v).backend().buffer()[new_pos()].bg, Color::Cyan);
        }
    }

    mod row_hover {
        use super::*;

        #[derive(Debug, Clone, Copy)]
        enum Row {
            Project(usize),
            Group,
            Workspace,
            Tab(usize),
        }

        impl Row {
            fn nav(self) -> Nav {
                match self {
                    Row::Project(_) | Row::Group => Nav::Projects,
                    Row::Workspace | Row::Tab(_) => Nav::Workspaces,
                }
            }

            fn active(self) -> bool {
                matches!(self, Row::Project(0) | Row::Tab(0))
            }
        }

        const COMPACT: Rect = Rect { x: 0, y: 0, width: 80, height: 30 };

        fn sample(light: bool) -> View<'static> {
            let mut v = View {
                has_project: true,
                workspaces: vec![WorkspaceEntry {
                    name: "login".into(),
                    tabs: vec!["claude".into(), "nvim".into()],
                    behind: 3,
                }],
                active_tab: Some(0),
                light,
                ..view(&["cornercase", "shop"])
            };
            v.groups = vec![GroupEntry { name: "work".into(), icon: '●', colour: 4, collapsed: false }];
            v.projects[1].group = Some(0);
            v
        }

        fn rect(v: &View, size: Rect, row: Row) -> Rect {
            let a = layout(size, v.widths).shown(v.nav);
            let sidebar = |r| entry_row(a.list, a.pitch, &v.sidebar_rows(), v.projects_scroll, r);
            let workspaces = |r| workspace_row(a.workspaces_list, a.pitch, &v.tab_counts(), v.workspaces_scroll, r);
            match row {
                Row::Project(p) => sidebar(SidebarRow::Project(p)),
                Row::Group => sidebar(SidebarRow::Group(0)),
                Row::Workspace => workspaces(WorkspaceRow::Workspace(0)),
                Row::Tab(t) => workspaces(WorkspaceRow::Tab(0, t)),
            }
        }

        fn backgrounds(v: &View, size: Rect, r: Rect) -> Vec<Color> {
            let t = render_sized(v, size.width, size.height);
            let mut colours: Vec<Color> = r.positions().map(|p| t.backend().buffer()[p].bg).collect::<Vec<_>>();
            colours.dedup();
            colours
        }

        fn hovering(mut v: View<'static>, size: Rect, row: Row) -> (View<'static>, Rect) {
            let r = rect(&v, size, row);
            v.hover = Some(Position::new(r.x + 4, middle(r).y));
            (v, r)
        }

        #[rstest]
        fn a_hovered_row_is_filled_edge_to_edge(
            #[values(Row::Project(0), Row::Project(1), Row::Group, Row::Workspace, Row::Tab(0), Row::Tab(1))] row: Row,
            #[values(false, true)] light: bool,
            #[values(false, true)] compact: bool,
        ) {
            let (size, nav) = if compact { (COMPACT, Some(row.nav())) } else { (AREA, None) };
            let (v, r) = hovering(View { nav, ..sample(light) }, size, row);
            let expected = match (row.active(), light) {
                (true, false) => DARK_SURFACE,
                (true, true) => LIGHT_SURFACE,
                (false, false) => DARK_HOVER,
                (false, true) => LIGHT_HOVER,
            };
            assert_eq!(backgrounds(&v, size, r), vec![expected]);
        }

        #[test]
        fn the_close_button_and_the_behind_tag_sit_on_the_hover_background() {
            let (v, r) = hovering(sample(false), AREA, Row::Workspace);
            let t = render(&v);
            let cells: Vec<(String, Color)> = [r.right() - 2, r.right() - 5]
                .into_iter()
                .map(|x| t.backend().buffer()[(x, r.y)].clone())
                .map(|c| (c.symbol().to_string(), c.bg))
                .collect();
            assert_eq!(cells, vec![("×".into(), DARK_HOVER), ("3".into(), DARK_HOVER)]);
        }

        #[test]
        fn other_rows_stay_plain() {
            let (v, _) = hovering(sample(false), AREA, Row::Tab(1));
            assert_eq!(backgrounds(&v, AREA, rect(&v, AREA, Row::Project(1))), vec![Color::Reset]);
        }

        #[test]
        fn no_row_is_lit_while_an_overlay_is_open() {
            let (mut v, r) = hovering(sample(false), AREA, Row::Project(1));
            v.overlay = Some(Overlay::Menu { at: Position::new(80, 1), items: vec!["rename tab".into()] });
            assert_eq!(backgrounds(&v, AREA, r), vec![Color::Reset]);
        }

        #[test]
        fn no_row_is_lit_while_dragging_a_border() {
            let (v, r) = hovering(sample(false), AREA, Row::Tab(1));
            let v = View { resizing: Some(Border::Workspaces), ..v };
            assert_eq!(backgrounds(&v, AREA, r), vec![Color::Reset]);
        }

        #[test]
        fn no_row_is_lit_while_dragging_a_divider() {
            let (v, r) = hovering(sample(false), AREA, Row::Tab(1));
            let tab = TabView { dragging: Some(Vec::new()), ..single(screen(b"")) };
            let v = View { tab: Some(tab), ..v };
            assert_eq!(backgrounds(&v, AREA, r), vec![Color::Reset]);
        }
    }
}
