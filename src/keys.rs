use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use libghostty_vt::key::{self, Action, Encoder, KittyKeyFlags, Mods};

pub fn encode(event: KeyEvent, encoder: &mut Encoder<'_>, kitty: KittyKeyFlags, app_cursor: bool) -> Vec<u8> {
    if prefers_legacy(&event, kitty) {
        return legacy(event, app_cursor);
    }
    let mut out = Vec::new();
    match ghostty_event(&event) {
        Some(ev) if encoder.encode_to_vec(&ev, &mut out).is_ok() && !out.is_empty() => out,
        _ => legacy(event, app_cursor),
    }
}

fn prefers_legacy(event: &KeyEvent, kitty: KittyKeyFlags) -> bool {
    if !kitty.is_empty() {
        return false;
    }
    match event.code {
        KeyCode::Char(_) => true,
        KeyCode::Enter => !event.modifiers.is_empty(),
        KeyCode::Tab => event.modifiers == KeyModifiers::CONTROL,
        _ => false,
    }
}

fn ghostty_event(event: &KeyEvent) -> Option<key::Event<'static>> {
    let mut ev = key::Event::new().ok()?;
    ev.set_action(match event.kind {
        KeyEventKind::Press => Action::Press,
        KeyEventKind::Repeat => Action::Repeat,
        KeyEventKind::Release => Action::Release,
    });
    let mut mods = ghostty_mods(event.modifiers);
    if event.code == KeyCode::BackTab {
        mods |= Mods::SHIFT;
    }
    ev.set_mods(mods).set_key(ghostty_key(event.code)?);
    if let KeyCode::Char(c) = event.code {
        let base = unshifted(c);
        ev.set_utf8(Some(c.to_string())).set_unshifted_codepoint(base);
        if base != c {
            ev.set_consumed_mods(Mods::SHIFT);
        }
    }
    Some(ev)
}

fn ghostty_mods(mods: KeyModifiers) -> Mods {
    let mut out = Mods::empty();
    out.set(Mods::SHIFT, mods.contains(KeyModifiers::SHIFT));
    out.set(Mods::CTRL, mods.contains(KeyModifiers::CONTROL));
    out.set(Mods::ALT, mods.contains(KeyModifiers::ALT));
    out.set(Mods::SUPER, mods.contains(KeyModifiers::SUPER));
    out
}

fn ghostty_key(code: KeyCode) -> Option<key::Key> {
    use key::Key as G;

    Some(match code {
        KeyCode::Backspace => G::Backspace,
        KeyCode::Enter => G::Enter,
        KeyCode::Left => G::ArrowLeft,
        KeyCode::Right => G::ArrowRight,
        KeyCode::Up => G::ArrowUp,
        KeyCode::Down => G::ArrowDown,
        KeyCode::Home => G::Home,
        KeyCode::End => G::End,
        KeyCode::PageUp => G::PageUp,
        KeyCode::PageDown => G::PageDown,
        KeyCode::Tab | KeyCode::BackTab => G::Tab,
        KeyCode::Delete => G::Delete,
        KeyCode::Insert => G::Insert,
        KeyCode::Esc => G::Escape,
        KeyCode::F(n @ 1..=12) => {
            [G::F1, G::F2, G::F3, G::F4, G::F5, G::F6, G::F7, G::F8, G::F9, G::F10, G::F11, G::F12][usize::from(n - 1)]
        }
        KeyCode::Char(c) => char_key(unshifted(c))?,
        _ => return None,
    })
}

fn char_key(c: char) -> Option<key::Key> {
    use key::Key as G;

    const LETTERS: [key::Key; 26] = [
        G::A,
        G::B,
        G::C,
        G::D,
        G::E,
        G::F,
        G::G,
        G::H,
        G::I,
        G::J,
        G::K,
        G::L,
        G::M,
        G::N,
        G::O,
        G::P,
        G::Q,
        G::R,
        G::S,
        G::T,
        G::U,
        G::V,
        G::W,
        G::X,
        G::Y,
        G::Z,
    ];
    const DIGITS: [key::Key; 10] =
        [G::Digit0, G::Digit1, G::Digit2, G::Digit3, G::Digit4, G::Digit5, G::Digit6, G::Digit7, G::Digit8, G::Digit9];

    Some(match c {
        'a'..='z' => LETTERS[usize::from(c as u8 - b'a')],
        '0'..='9' => DIGITS[usize::from(c as u8 - b'0')],
        '`' => G::Backquote,
        '\\' => G::Backslash,
        '[' => G::BracketLeft,
        ']' => G::BracketRight,
        ',' => G::Comma,
        '=' => G::Equal,
        '-' => G::Minus,
        '.' => G::Period,
        '\'' => G::Quote,
        ';' => G::Semicolon,
        '/' => G::Slash,
        ' ' => G::Space,
        _ => return None,
    })
}

fn unshifted(c: char) -> char {
    match c {
        'A'..='Z' => c.to_ascii_lowercase(),
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        '_' => '-',
        '+' => '=',
        '{' => '[',
        '}' => ']',
        '|' => '\\',
        ':' => ';',
        '"' => '\'',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        '~' => '`',
        _ => c,
    }
}

fn legacy(key: KeyEvent, app_cursor: bool) -> Vec<u8> {
    let mods = key.modifiers;
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let alt = mods.contains(KeyModifiers::ALT);
    let shift = mods.contains(KeyModifiers::SHIFT);
    let modifier_param = 1 + u8::from(shift) + u8::from(alt) * 2 + u8::from(ctrl) * 4;
    let modified = modifier_param > 1;

    let cursor_key = |c: char| -> Vec<u8> {
        if modified {
            format!("\x1b[1;{modifier_param}{c}").into_bytes()
        } else if app_cursor {
            format!("\x1bO{c}").into_bytes()
        } else {
            format!("\x1b[{c}").into_bytes()
        }
    };
    let tilde_key = |n: u8| -> Vec<u8> {
        if modified { format!("\x1b[{n};{modifier_param}~").into_bytes() } else { format!("\x1b[{n}~").into_bytes() }
    };

    let mut out = match key.code {
        KeyCode::Char(c) if ctrl => ctrl_char(c),
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => cursor_key('A'),
        KeyCode::Down => cursor_key('B'),
        KeyCode::Right => cursor_key('C'),
        KeyCode::Left => cursor_key('D'),
        KeyCode::Home => cursor_key('H'),
        KeyCode::End => cursor_key('F'),
        KeyCode::Insert => tilde_key(2),
        KeyCode::Delete => tilde_key(3),
        KeyCode::PageUp => tilde_key(5),
        KeyCode::PageDown => tilde_key(6),
        KeyCode::F(n @ 1..=4) => {
            let c = char::from(b'P' + n - 1);
            if modified { format!("\x1b[1;{modifier_param}{c}").into_bytes() } else { format!("\x1bO{c}").into_bytes() }
        }
        KeyCode::F(5) => tilde_key(15),
        KeyCode::F(n @ 6..=10) => tilde_key(n + 11),
        KeyCode::F(n @ 11..=12) => tilde_key(n + 12),
        _ => vec![],
    };

    if alt && matches!(key.code, KeyCode::Char(_) | KeyCode::Enter | KeyCode::Backspace) {
        out.insert(0, 0x1b);
    }
    out
}

fn ctrl_char(c: char) -> Vec<u8> {
    match c.to_ascii_lowercase() {
        c @ 'a'..='z' => vec![c as u8 - b'a' + 1],
        ' ' | '@' | '2' => vec![0],
        '[' | '3' => vec![0x1b],
        '\\' | '4' => vec![0x1c],
        ']' | '5' => vec![0x1d],
        '^' | '6' => vec![0x1e],
        '_' | '-' | '7' => vec![0x1f],
        '?' | '8' => vec![0x7f],
        other => other.to_string().into_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const NONE: KeyModifiers = KeyModifiers::NONE;
    const CTRL: KeyModifiers = KeyModifiers::CONTROL;
    const ALT: KeyModifiers = KeyModifiers::ALT;
    const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

    const NO_KITTY: KittyKeyFlags = KittyKeyFlags::DISABLED;
    const DISAMBIGUATE: KittyKeyFlags = KittyKeyFlags::DISAMBIGUATE;

    fn encode_with(code: KeyCode, mods: KeyModifiers, kitty: KittyKeyFlags, app_cursor: bool) -> Vec<u8> {
        let mut encoder = Encoder::new().expect("key encoder");
        encoder.set_kitty_flags(kitty).set_cursor_key_application(app_cursor);
        encode(KeyEvent::new(code, mods), &mut encoder, kitty, app_cursor)
    }

    fn encode_normal(code: KeyCode, mods: KeyModifiers) -> Vec<u8> {
        encode_with(code, mods, NO_KITTY, false)
    }

    fn encode_app_cursor(code: KeyCode, mods: KeyModifiers) -> Vec<u8> {
        encode_with(code, mods, NO_KITTY, true)
    }

    fn encode_kitty(code: KeyCode, mods: KeyModifiers) -> Vec<u8> {
        encode_with(code, mods, DISAMBIGUATE, false)
    }

    #[rstest]
    #[case::lowercase('a', NONE, b"a")]
    #[case::uppercase('A', SHIFT, b"A")]
    #[case::multibyte('ñ', NONE, "ñ".as_bytes())]
    fn plain_chars_are_sent_as_utf8(#[case] c: char, #[case] mods: KeyModifiers, #[case] expected: &[u8]) {
        assert_eq!(encode_normal(KeyCode::Char(c), mods), expected);
    }

    #[rstest]
    #[case::ctrl_a('a', 0x01)]
    #[case::ctrl_c('c', 0x03)]
    #[case::ctrl_shift_c('C', 0x03)]
    #[case::ctrl_z('z', 0x1a)]
    #[case::ctrl_space(' ', 0x00)]
    #[case::ctrl_bracket('[', 0x1b)]
    #[case::ctrl_underscore('_', 0x1f)]
    fn ctrl_chars_become_control_codes(#[case] c: char, #[case] expected: u8) {
        assert_eq!(encode_normal(KeyCode::Char(c), CTRL), [expected]);
    }

    #[rstest]
    #[case::alt_char(KeyCode::Char('x'), ALT, b"\x1bx")]
    #[case::alt_ctrl_char(KeyCode::Char('c'), CTRL | ALT, b"\x1b\x03")]
    #[case::alt_backspace(KeyCode::Backspace, ALT, b"\x1b\x7f")]
    fn alt_prefixes_escape(#[case] code: KeyCode, #[case] mods: KeyModifiers, #[case] expected: &[u8]) {
        assert_eq!(encode_normal(code, mods), expected);
    }

    #[rstest]
    #[case::enter(KeyCode::Enter, b"\r")]
    #[case::tab(KeyCode::Tab, b"\t")]
    #[case::backspace(KeyCode::Backspace, b"\x7f")]
    #[case::escape(KeyCode::Esc, b"\x1b")]
    #[case::insert(KeyCode::Insert, b"\x1b[2~")]
    #[case::delete(KeyCode::Delete, b"\x1b[3~")]
    #[case::page_up(KeyCode::PageUp, b"\x1b[5~")]
    #[case::page_down(KeyCode::PageDown, b"\x1b[6~")]
    fn special_keys_send_their_sequence(#[case] code: KeyCode, #[case] expected: &[u8]) {
        assert_eq!(encode_normal(code, NONE), expected);
    }

    #[test]
    fn shift_tab_sends_back_tab() {
        assert_eq!(encode_normal(KeyCode::BackTab, SHIFT), b"\x1b[Z");
    }

    #[rstest]
    #[case::up(KeyCode::Up, b"\x1b[A")]
    #[case::down(KeyCode::Down, b"\x1b[B")]
    #[case::right(KeyCode::Right, b"\x1b[C")]
    #[case::left(KeyCode::Left, b"\x1b[D")]
    #[case::home(KeyCode::Home, b"\x1b[H")]
    #[case::end(KeyCode::End, b"\x1b[F")]
    fn cursor_keys_use_csi_in_normal_mode(#[case] code: KeyCode, #[case] expected: &[u8]) {
        assert_eq!(encode_normal(code, NONE), expected);
    }

    #[rstest]
    #[case::up(KeyCode::Up, b"\x1bOA")]
    #[case::home(KeyCode::Home, b"\x1bOH")]
    fn cursor_keys_use_ss3_in_application_mode(#[case] code: KeyCode, #[case] expected: &[u8]) {
        assert_eq!(encode_app_cursor(code, NONE), expected);
    }

    #[rstest]
    #[case::shift_up(KeyCode::Up, SHIFT, b"\x1b[1;2A")]
    #[case::alt_right(KeyCode::Right, ALT, b"\x1b[1;3C")]
    #[case::ctrl_left(KeyCode::Left, CTRL, b"\x1b[1;5D")]
    #[case::ctrl_delete(KeyCode::Delete, CTRL, b"\x1b[3;5~")]
    fn modified_keys_carry_xterm_modifier_param(
        #[case] code: KeyCode,
        #[case] mods: KeyModifiers,
        #[case] expected: &[u8],
    ) {
        assert_eq!(encode_normal(code, mods), expected);
    }

    #[test]
    fn modified_cursor_keys_ignore_application_mode() {
        assert_eq!(encode_app_cursor(KeyCode::Up, CTRL), b"\x1b[1;5A");
    }

    #[rstest]
    #[case::f1(1, b"\x1bOP")]
    #[case::f4(4, b"\x1bOS")]
    #[case::f5(5, b"\x1b[15~")]
    #[case::f6(6, b"\x1b[17~")]
    #[case::f10(10, b"\x1b[21~")]
    #[case::f11(11, b"\x1b[23~")]
    #[case::f12(12, b"\x1b[24~")]
    #[case::f13_unsupported(13, b"")]
    fn function_keys_send_vt220_sequences(#[case] n: u8, #[case] expected: &[u8]) {
        assert_eq!(encode_normal(KeyCode::F(n), NONE), expected);
    }

    #[test]
    fn modified_f1_uses_csi_with_modifier() {
        assert_eq!(encode_normal(KeyCode::F(1), SHIFT), b"\x1b[1;2P");
    }

    #[rstest]
    #[case::plain_char(KeyCode::Char('a'), NONE, b"a")]
    #[case::shifted_char(KeyCode::Char('A'), SHIFT, b"A")]
    #[case::shifted_symbol(KeyCode::Char('!'), SHIFT, b"!")]
    #[case::ctrl_shift_char(KeyCode::Char('A'), CTRL | SHIFT, b"\x1b[97;6u")]
    #[case::ctrl_digit(KeyCode::Char('1'), CTRL, b"\x1b[49;5u")]
    #[case::non_ascii_char(KeyCode::Char('ñ'), NONE, "ñ".as_bytes())]
    #[case::ctrl_char(KeyCode::Char('a'), CTRL, b"\x1b[97;5u")]
    #[case::alt_char(KeyCode::Char('x'), ALT, b"\x1b[120;3u")]
    #[case::escape(KeyCode::Esc, NONE, b"\x1b[27u")]
    #[case::enter(KeyCode::Enter, NONE, b"\r")]
    #[case::shift_enter(KeyCode::Enter, SHIFT, b"\x1b[13;2u")]
    #[case::ctrl_tab(KeyCode::Tab, CTRL, b"\x1b[9;5u")]
    #[case::up(KeyCode::Up, NONE, b"\x1b[A")]
    fn kitty_disambiguate_follows_the_spec(#[case] code: KeyCode, #[case] mods: KeyModifiers, #[case] expected: &[u8]) {
        assert_eq!(encode_kitty(code, mods), expected);
    }

    #[rstest]
    #[case::shift_enter(KeyCode::Enter, SHIFT, b"\r")]
    #[case::ctrl_tab(KeyCode::Tab, CTRL, b"\t")]
    fn without_kitty_extended_keys_stay_legacy(
        #[case] code: KeyCode,
        #[case] mods: KeyModifiers,
        #[case] expected: &[u8],
    ) {
        assert_eq!(encode_normal(code, mods), expected);
    }
}
