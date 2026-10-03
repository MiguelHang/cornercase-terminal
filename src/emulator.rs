use std::cell::RefCell;
use std::rc::Rc;

use crossterm::event::KeyEvent;
use libghostty_vt::Terminal;
use libghostty_vt::fmt::Format;
use libghostty_vt::key::{Encoder, KittyKeyFlags};
use libghostty_vt::render::{CellIterator, RenderState, RowIterator};
use libghostty_vt::screen::TrackedGridRef;
use libghostty_vt::selection::{FormatOptions, Selection};
use libghostty_vt::style::{Palette, StyleColor, Underline};
use libghostty_vt::terminal::{ClipboardLocation, ClipboardWrite, ColorScheme, Mode, Options, Point, PointCoordinate};
use ratatui::layout::Position;
use ratatui::style::{Color, Modifier, Style};

use crate::host_theme::HostTheme;
use crate::keys;
use crate::mouse::{MouseEncoding, MouseMode};

pub type Result<T> = libghostty_vt::error::Result<T>;
pub type ReplyFn = Box<dyn FnMut(&[u8])>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub symbol: String,
    pub style: Style,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub rows: Vec<Vec<Cell>>,
    pub cursor: Option<Position>,
}

impl Snapshot {
    pub fn contents(&self) -> String {
        self.rows
            .iter()
            .map(|row| row.iter().map(|c| c.symbol.as_str()).collect::<String>().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub struct Emulator {
    vt: Terminal<'static, 'static>,
    render: RenderState<'static>,
    rows: RowIterator<'static>,
    cells: CellIterator<'static>,
    keys: Encoder<'static>,
    anchor: Option<TrackedGridRef>,
    copied: Rc<RefCell<Vec<String>>>,
}

impl Emulator {
    pub fn new(rows: u16, cols: u16, scrollback: usize, theme: &HostTheme, mut reply: ReplyFn) -> Result<Self> {
        let mut vt = Terminal::new(Options { cols, rows, max_scrollback: scrollback })?;
        vt.on_pty_write(move |_, bytes| reply(bytes))?;
        let copied = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&copied);
        vt.on_clipboard_write(move |_, write| {
            if let Some(text) = clipboard_text(&write) {
                sink.borrow_mut().push(text);
            }
            Ok(())
        })?;
        apply_theme(&mut vt, theme)?;
        Ok(Self {
            vt,
            render: RenderState::new()?,
            rows: RowIterator::new()?,
            cells: CellIterator::new()?,
            keys: Encoder::new()?,
            anchor: None,
            copied,
        })
    }

    pub fn take_copied(&mut self) -> Vec<String> {
        std::mem::take(&mut self.copied.borrow_mut())
    }

    pub fn encode_key(&mut self, key: KeyEvent) -> Vec<u8> {
        self.keys.set_options_from_terminal(&self.vt);
        let kitty = self.vt.kitty_keyboard_flags().unwrap_or(KittyKeyFlags::DISABLED);
        let app_cursor = self.application_cursor();
        keys::encode(key, &mut self.keys, kitty, app_cursor)
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.vt.vt_write(bytes);
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.vt.resize(cols, rows, 0, 0)
    }

    pub fn size(&self) -> Result<(u16, u16)> {
        Ok((self.vt.rows()?, self.vt.cols()?))
    }

    fn mode(&self, mode: Mode) -> bool {
        self.vt.mode(mode).unwrap_or(false)
    }

    pub fn application_cursor(&self) -> bool {
        self.mode(Mode::DECCKM)
    }

    pub fn bracketed_paste(&self) -> bool {
        self.mode(Mode::BRACKETED_PASTE)
    }

    pub fn mouse_mode(&self) -> MouseMode {
        [
            (Mode::ANY_MOUSE, MouseMode::AnyMotion),
            (Mode::BUTTON_MOUSE, MouseMode::ButtonMotion),
            (Mode::NORMAL_MOUSE, MouseMode::PressRelease),
            (Mode::X10_MOUSE, MouseMode::Press),
        ]
        .into_iter()
        .find_map(|(mode, ours)| self.mode(mode).then_some(ours))
        .unwrap_or(MouseMode::None)
    }

    pub fn mouse_encoding(&self) -> MouseEncoding {
        if self.mode(Mode::SGR_MOUSE) {
            MouseEncoding::Sgr
        } else if self.mode(Mode::UTF8_MOUSE) {
            MouseEncoding::Utf8
        } else {
            MouseEncoding::Default
        }
    }

    pub fn start_selection(&mut self, at: Position) -> Result<()> {
        self.vt.set_selection(None)?;
        self.anchor = Some(self.vt.track_grid_ref(viewport(at))?);
        Ok(())
    }

    pub fn extend_selection(&mut self, to: Position) -> Result<()> {
        let Some(anchor) = &self.anchor else { return Ok(()) };
        let Some(start) = anchor.snapshot(&self.vt)? else { return Ok(()) };
        let end = self.vt.grid_ref(viewport(to))?;
        self.vt.set_selection(Some(&Selection::new(start, end, false)))?;
        Ok(())
    }

    pub fn finish_selection(&mut self) -> Result<Option<String>> {
        self.anchor = None;
        let options = FormatOptions::new().with_emit_format(Format::Plain).with_unwrap(true).with_trim(true);
        let text = self.vt.format_selection_alloc(None, options)?.map(|b| String::from_utf8_lossy(&b).into_owned());
        self.vt.set_selection(None)?;
        Ok(text.filter(|t| !t.is_empty()))
    }

    pub fn snapshot(&mut self) -> Result<Snapshot> {
        let snap = self.render.update(&self.vt)?;
        let cursor =
            if snap.cursor_visible()? { snap.cursor_viewport()?.map(|vp| Position::new(vp.x, vp.y)) } else { None };
        let mut rows = Vec::new();
        let mut text = String::new();
        let mut row_it = self.rows.update(&snap)?;
        while let Some(row) = row_it.next() {
            let mut cells = Vec::new();
            let mut cell_it = self.cells.update(row)?;
            while let Some(cell) = cell_it.next() {
                text.clear();
                if cell.graphemes_len()? > 0 {
                    cell.graphemes_utf8(&mut text)?;
                }
                let symbol = if text.is_empty() { " ".to_string() } else { text.clone() };
                let style = if cell.has_styling()? { to_style(cell.style()?) } else { Style::default() };
                let style = if cell.is_selected()? { selected(style) } else { style };
                cells.push(Cell { symbol, style });
            }
            rows.push(cells);
        }
        Ok(Snapshot { rows, cursor })
    }
}

fn clipboard_text(write: &ClipboardWrite<'_>) -> Option<String> {
    if write.location() != ClipboardLocation::Standard {
        return None;
    }
    let mut contents = write.contents();
    let text = contents.clone().find(|c| c.mime.starts_with("text/plain")).or_else(|| contents.next())?;
    (!text.data.is_empty()).then(|| text.data.to_string())
}

fn viewport(at: Position) -> Point {
    Point::Viewport(PointCoordinate { x: at.x, y: u32::from(at.y) })
}

fn selected(style: Style) -> Style {
    if style.add_modifier.contains(Modifier::REVERSED) {
        style.remove_modifier(Modifier::REVERSED)
    } else {
        style.add_modifier(Modifier::REVERSED)
    }
}

fn apply_theme(vt: &mut Terminal<'static, 'static>, theme: &HostTheme) -> Result<()> {
    if theme.foreground.is_some() {
        vt.set_default_fg_color(theme.foreground)?;
    }
    if theme.background.is_some() {
        vt.set_default_bg_color(theme.background)?;
    }
    if theme.palette.iter().any(Option::is_some) {
        let Palette(mut colors) = vt.default_color_palette()?;
        for (slot, host) in colors.iter_mut().zip(&theme.palette) {
            if let Some(host) = host {
                *slot = *host;
            }
        }
        vt.set_default_color_palette(Some(Palette(colors)))?;
    }
    if let Some(light) = theme.is_light() {
        let scheme = if light { ColorScheme::Light } else { ColorScheme::Dark };
        vt.on_color_scheme(move |_| Some(scheme))?;
    }
    Ok(())
}

fn to_color(color: StyleColor) -> Color {
    match color {
        StyleColor::None => Color::Reset,
        StyleColor::Palette(i) => Color::Indexed(i.0),
        StyleColor::Rgb(rgb) => Color::Rgb(rgb.r, rgb.g, rgb.b),
    }
}

fn to_style(s: libghostty_vt::style::Style) -> Style {
    let mut m = Modifier::empty();
    m.set(Modifier::BOLD, s.bold);
    m.set(Modifier::ITALIC, s.italic);
    m.set(Modifier::DIM, s.faint);
    m.set(Modifier::SLOW_BLINK, s.blink);
    m.set(Modifier::REVERSED, s.inverse);
    m.set(Modifier::HIDDEN, s.invisible);
    m.set(Modifier::CROSSED_OUT, s.strikethrough);
    m.set(Modifier::UNDERLINED, s.underline != Underline::None);
    Style::default().fg(to_color(s.fg_color)).bg(to_color(s.bg_color)).add_modifier(m)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use rstest::rstest;

    use super::*;

    fn emulator() -> (Emulator, Rc<RefCell<Vec<u8>>>) {
        themed_emulator(&HostTheme::default())
    }

    fn themed_emulator(theme: &HostTheme) -> (Emulator, Rc<RefCell<Vec<u8>>>) {
        let replies = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&replies);
        let reply = Box::new(move |bytes: &[u8]| sink.borrow_mut().extend_from_slice(bytes));
        (Emulator::new(24, 80, 0, theme, reply).expect("emulator"), replies)
    }

    fn snapshot_of(output: &[u8]) -> Snapshot {
        let (mut emu, _) = emulator();
        emu.feed(output);
        emu.snapshot().expect("snapshot")
    }

    fn replies_to(output: &[u8]) -> String {
        let (mut emu, replies) = emulator();
        emu.feed(output);
        String::from_utf8_lossy(&replies.borrow()).into_owned()
    }

    fn style_of(output: &[u8]) -> Style {
        snapshot_of(output).rows[0][0].style
    }

    mod snapshot {
        use super::*;

        #[test]
        fn has_the_text() {
            assert!(snapshot_of(b"hello\r\nworld").contents().starts_with("hello\nworld"));
        }

        #[test]
        fn has_the_cursor() {
            assert_eq!(snapshot_of(b"hello\r\nworld").cursor, Some(Position::new(5, 1)));
        }

        #[test]
        fn hides_the_cursor_when_asked() {
            assert_eq!(snapshot_of(b"\x1b[?25l").cursor, None);
        }

        #[test]
        fn has_one_row_per_line_of_the_terminal() {
            assert_eq!(snapshot_of(b"").rows.len(), 24);
        }

        #[test]
        fn rewraps_lines_on_resize() {
            let (mut emu, _) = emulator();
            emu.feed(&[b'x'; 30]);
            emu.resize(24, 20).expect("resize");
            assert!(emu.snapshot().expect("snapshot").contents().starts_with(&format!(
                "{}\n{}",
                "x".repeat(20),
                "x".repeat(10)
            )));
        }
    }

    mod clipboard {
        use super::*;

        fn copied_by(output: &[u8]) -> Vec<String> {
            let (mut emu, _) = emulator();
            emu.feed(output);
            emu.take_copied()
        }

        #[test]
        fn an_osc52_write_is_handed_over() {
            assert_eq!(copied_by(b"\x1b]52;c;aGVsbG8=\x07"), ["hello"]);
        }

        #[rstest]
        #[case::a_read_request(b"\x1b]52;c;?\x07")]
        #[case::the_primary_selection(b"\x1b]52;p;aGVsbG8=\x07")]
        #[case::an_empty_write(b"\x1b]52;c;\x07")]
        #[case::plain_output(b"hello")]
        fn hands_over_nothing_for(#[case] output: &[u8]) {
            assert_eq!(copied_by(output), Vec::<String>::new());
        }

        #[test]
        fn taking_empties_the_queue() {
            let (mut emu, _) = emulator();
            emu.feed(b"\x1b]52;c;aGVsbG8=\x07");
            emu.take_copied();
            assert_eq!(emu.take_copied(), Vec::<String>::new());
        }
    }

    mod selection {
        use super::*;

        fn small(rows: u16, cols: u16) -> Emulator {
            Emulator::new(rows, cols, 100, &HostTheme::default(), Box::new(|_| {})).expect("emulator")
        }

        fn select(emu: &mut Emulator, from: (u16, u16), to: (u16, u16)) {
            emu.start_selection(Position::new(from.0, from.1)).expect("start");
            emu.extend_selection(Position::new(to.0, to.1)).expect("extend");
        }

        fn copied(output: &[u8], from: (u16, u16), to: (u16, u16)) -> Option<String> {
            let (mut emu, _) = emulator();
            emu.feed(output);
            select(&mut emu, from, to);
            emu.finish_selection().expect("finish")
        }

        #[rstest]
        #[case::forwards(b"hello world", (0, 0), (4, 0), "hello")]
        #[case::backwards(b"hello world", (10, 0), (6, 0), "world")]
        #[case::across_lines(b"ab\r\ncd", (0, 0), (1, 1), "ab\ncd")]
        #[case::without_trailing_spaces(b"ab   \r\ncd", (0, 0), (9, 0), "ab")]
        fn copies_the_text_between_both_points(
            #[case] output: &[u8],
            #[case] from: (u16, u16),
            #[case] to: (u16, u16),
            #[case] expected: &str,
        ) {
            assert_eq!(copied(output, from, to).as_deref(), Some(expected));
        }

        #[test]
        fn joins_a_line_the_terminal_wrapped() {
            let mut emu = small(4, 10);
            emu.feed(b"0123456789abcde");
            select(&mut emu, (0, 0), (4, 1));
            assert_eq!(emu.finish_selection().expect("finish").as_deref(), Some("0123456789abcde"));
        }

        #[test]
        fn keeps_its_start_on_the_text_when_the_screen_scrolls() {
            let mut emu = small(3, 10);
            emu.feed(b"a\r\nb\r\nc");
            emu.start_selection(Position::new(0, 2)).expect("start");
            emu.feed(b"\r\nd");
            emu.extend_selection(Position::new(0, 2)).expect("extend");
            assert_eq!(emu.finish_selection().expect("finish").as_deref(), Some("c\nd"));
        }

        #[test]
        fn a_click_without_dragging_copies_nothing() {
            let (mut emu, _) = emulator();
            emu.feed(b"hello");
            emu.start_selection(Position::new(1, 0)).expect("start");
            assert_eq!(emu.finish_selection().expect("finish"), None);
        }

        #[test]
        fn marks_the_selected_cells_reversed() {
            let (mut emu, _) = emulator();
            emu.feed(b"hello world");
            select(&mut emu, (0, 0), (4, 0));
            let row = &emu.snapshot().expect("snapshot").rows[0];
            assert!(row[4].style.add_modifier.contains(Modifier::REVERSED));
            assert!(!row[5].style.add_modifier.contains(Modifier::REVERSED));
        }

        #[test]
        fn reversed_text_turns_back_to_normal_when_selected() {
            let (mut emu, _) = emulator();
            emu.feed(b"\x1b[7mhi");
            select(&mut emu, (0, 0), (1, 0));
            let style = emu.snapshot().expect("snapshot").rows[0][0].style;
            assert!(!style.add_modifier.contains(Modifier::REVERSED));
        }

        #[test]
        fn finishing_clears_the_highlight() {
            let (mut emu, _) = emulator();
            emu.feed(b"hello");
            select(&mut emu, (0, 0), (4, 0));
            emu.finish_selection().expect("finish");
            let row = &emu.snapshot().expect("snapshot").rows[0];
            assert!(row.iter().all(|c| !c.style.add_modifier.contains(Modifier::REVERSED)));
        }
    }

    mod style {
        use super::*;

        #[rstest]
        #[case::palette_stays_indexed(b"\x1b[31mx", Style::default().fg(Color::Indexed(1)).bg(Color::Reset))]
        #[case::extended_palette(b"\x1b[38;5;208mx", Style::default().fg(Color::Indexed(208)).bg(Color::Reset))]
        #[case::rgb(b"\x1b[38;2;10;200;30mx", Style::default().fg(Color::Rgb(10, 200, 30)).bg(Color::Reset))]
        #[case::background(b"\x1b[44mx", Style::default().fg(Color::Reset).bg(Color::Indexed(4)))]
        #[case::bold(b"\x1b[1mx", Style::default().fg(Color::Reset).bg(Color::Reset).add_modifier(Modifier::BOLD))]
        #[case::plain(b"x", Style::default())]
        fn maps_to_ratatui(#[case] output: &[u8], #[case] expected: Style) {
            assert_eq!(style_of(output), expected);
        }
    }

    mod replies {
        use super::*;

        #[rstest]
        #[case::cursor_position(b"\x1b[3;5H\x1b[6n", "\x1b[3;5R")]
        #[case::status(b"\x1b[5n", "\x1b[0n")]
        #[case::plain_output(b"hello", "")]
        fn answers_terminal_queries(#[case] output: &[u8], #[case] expected: &str) {
            assert_eq!(replies_to(output), expected);
        }

        #[test]
        fn answers_device_attributes() {
            assert!(replies_to(b"\x1b[c").starts_with("\x1b[?"));
        }
    }

    mod host_theme {
        use libghostty_vt::style::RgbColor;

        use super::*;

        const DARK: RgbColor = RgbColor { r: 0x12, g: 0x34, b: 0x56 };
        const LIGHT: RgbColor = RgbColor { r: 0xfd, g: 0xf6, b: 0xe3 };
        const RED: RgbColor = RgbColor { r: 0xcc, g: 0x11, b: 0x22 };

        fn themed_replies_to(theme: &HostTheme, output: &[u8]) -> String {
            let (mut emu, replies) = themed_emulator(theme);
            emu.feed(output);
            String::from_utf8_lossy(&replies.borrow()).into_owned()
        }

        fn with_background(background: RgbColor) -> HostTheme {
            HostTheme { background: Some(background), ..HostTheme::default() }
        }

        #[test]
        fn background_query_gets_the_host_background() {
            assert!(themed_replies_to(&with_background(DARK), b"\x1b]11;?\x07").contains("rgb:1212/3434/5656"));
        }

        #[test]
        fn foreground_query_gets_the_host_foreground() {
            let theme = HostTheme { foreground: Some(LIGHT), ..HostTheme::default() };
            assert!(themed_replies_to(&theme, b"\x1b]10;?\x07").contains("rgb:fdfd/f6f6/e3e3"));
        }

        #[test]
        fn palette_query_gets_the_host_palette() {
            let mut theme = HostTheme::default();
            theme.palette[1] = Some(RED);
            assert!(themed_replies_to(&theme, b"\x1b]4;1;?\x07").contains("rgb:cccc/1111/2222"));
        }

        #[test]
        fn background_query_is_unanswered_without_host_colors() {
            assert_eq!(replies_to(b"\x1b]11;?\x07"), "");
        }

        #[rstest]
        #[case::dark(DARK, "\x1b[?997;1n")]
        #[case::light(LIGHT, "\x1b[?997;2n")]
        fn color_scheme_query_follows_the_host_background(#[case] background: RgbColor, #[case] expected: &str) {
            assert_eq!(themed_replies_to(&with_background(background), b"\x1b[?996n"), expected);
        }

        #[test]
        fn palette_colors_still_render_as_indices() {
            let mut theme = HostTheme::default();
            theme.palette[1] = Some(RED);
            let (mut emu, _) = themed_emulator(&theme);
            emu.feed(b"\x1b[31mx");
            assert_eq!(emu.snapshot().expect("snapshot").rows[0][0].style.fg, Some(Color::Indexed(1)));
        }
    }

    mod modes {
        use super::*;

        fn after(output: &[u8]) -> Emulator {
            let (mut emu, _) = emulator();
            emu.feed(output);
            emu
        }

        #[rstest]
        #[case::none(b"", MouseMode::None)]
        #[case::x10(b"\x1b[?9h", MouseMode::Press)]
        #[case::normal(b"\x1b[?1000h", MouseMode::PressRelease)]
        #[case::button(b"\x1b[?1002h", MouseMode::ButtonMotion)]
        #[case::any(b"\x1b[?1003h", MouseMode::AnyMotion)]
        fn mouse_mode_follows_the_program(#[case] output: &[u8], #[case] expected: MouseMode) {
            assert_eq!(after(output).mouse_mode(), expected);
        }

        #[rstest]
        #[case::default(b"", MouseEncoding::Default)]
        #[case::utf8(b"\x1b[?1005h", MouseEncoding::Utf8)]
        #[case::sgr(b"\x1b[?1006h", MouseEncoding::Sgr)]
        fn mouse_encoding_follows_the_program(#[case] output: &[u8], #[case] expected: MouseEncoding) {
            assert_eq!(after(output).mouse_encoding(), expected);
        }

        #[rstest]
        #[case::off(b"", false)]
        #[case::on(b"\x1b[?1h", true)]
        fn application_cursor_follows_the_program(#[case] output: &[u8], #[case] expected: bool) {
            assert_eq!(after(output).application_cursor(), expected);
        }

        #[rstest]
        #[case::off(b"", false)]
        #[case::on(b"\x1b[?2004h", true)]
        fn bracketed_paste_follows_the_program(#[case] output: &[u8], #[case] expected: bool) {
            assert_eq!(after(output).bracketed_paste(), expected);
        }
    }
}
