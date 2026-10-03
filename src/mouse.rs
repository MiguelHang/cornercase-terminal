use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseMode {
    None,
    Press,
    PressRelease,
    ButtonMotion,
    AnyMotion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEncoding {
    Default,
    Utf8,
    Sgr,
}

const LEGACY_RELEASE: u32 = 3;
const MOTION: u32 = 32;
const WHEEL: u32 = 64;
const LEGACY_OFFSET: u32 = 32;

pub fn encode(ev: &MouseEvent, col: u16, row: u16, mode: MouseMode, encoding: MouseEncoding) -> Option<Vec<u8>> {
    if !is_wanted(ev.kind, mode) {
        return None;
    }

    let sgr = encoding == MouseEncoding::Sgr;
    let code = button_code(ev.kind, sgr) + modifier_bits(ev.modifiers);
    let (x, y) = (u32::from(col) + 1, u32::from(row) + 1);

    Some(match encoding {
        MouseEncoding::Sgr => {
            let end = if matches!(ev.kind, MouseEventKind::Up(_)) { 'm' } else { 'M' };
            format!("\x1b[<{code};{x};{y}{end}").into_bytes()
        }
        MouseEncoding::Default => {
            let byte = |v: u32| u8::try_from(LEGACY_OFFSET + v).unwrap_or(u8::MAX);
            vec![0x1b, b'[', b'M', byte(code), byte(x), byte(y)]
        }
        MouseEncoding::Utf8 => {
            let mut out = b"\x1b[M".to_vec();
            for v in [code, x, y] {
                let c = char::from_u32(LEGACY_OFFSET + v).unwrap_or(' ');
                out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
            }
            out
        }
    })
}

fn is_wanted(kind: MouseEventKind, mode: MouseMode) -> bool {
    use MouseEventKind as K;
    use MouseMode as M;

    match kind {
        K::Down(_) | K::ScrollUp | K::ScrollDown | K::ScrollLeft | K::ScrollRight => mode != M::None,
        K::Up(_) => matches!(mode, M::PressRelease | M::ButtonMotion | M::AnyMotion),
        K::Drag(_) => matches!(mode, M::ButtonMotion | M::AnyMotion),
        K::Moved => mode == M::AnyMotion,
    }
}

fn button_code(kind: MouseEventKind, sgr: bool) -> u32 {
    use MouseEventKind as K;

    let button = |b: MouseButton| match b {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    match kind {
        K::Down(b) => button(b),
        K::Up(b) if sgr => button(b),
        K::Up(_) => LEGACY_RELEASE,
        K::Drag(b) => button(b) + MOTION,
        K::Moved => LEGACY_RELEASE + MOTION,
        K::ScrollUp => WHEEL,
        K::ScrollDown => WHEEL + 1,
        K::ScrollLeft => WHEEL + 2,
        K::ScrollRight => WHEEL + 3,
    }
}

fn modifier_bits(mods: KeyModifiers) -> u32 {
    let bit = |m: KeyModifiers, value: u32| if mods.contains(m) { value } else { 0 };
    bit(KeyModifiers::SHIFT, 4) + bit(KeyModifiers::ALT, 8) + bit(KeyModifiers::CONTROL, 16)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use MouseEncoding as E;
    use MouseEventKind as K;
    use MouseMode as M;

    const LEFT: MouseButton = MouseButton::Left;
    const RIGHT: MouseButton = MouseButton::Right;

    fn event(kind: MouseEventKind) -> MouseEvent {
        MouseEvent { kind, column: 0, row: 0, modifiers: KeyModifiers::NONE }
    }

    fn encode_sgr(kind: MouseEventKind, mode: MouseMode) -> Option<String> {
        encode(&event(kind), 4, 9, mode, E::Sgr).map(|b| String::from_utf8(b).expect("sgr is ascii"))
    }

    mod filtering {
        use super::*;

        #[rstest]
        #[case::press(K::Down(LEFT))]
        #[case::wheel(K::ScrollUp)]
        #[case::motion(K::Moved)]
        fn nothing_is_sent_when_program_did_not_ask(#[case] kind: MouseEventKind) {
            assert_eq!(encode_sgr(kind, M::None), None);
        }

        #[rstest]
        #[case::press_in_x10(K::Down(LEFT), M::Press, true)]
        #[case::wheel_in_x10(K::ScrollDown, M::Press, true)]
        #[case::release_in_x10(K::Up(LEFT), M::Press, false)]
        #[case::release_in_vt200(K::Up(LEFT), M::PressRelease, true)]
        #[case::drag_in_vt200(K::Drag(LEFT), M::PressRelease, false)]
        #[case::drag_in_button_motion(K::Drag(LEFT), M::ButtonMotion, true)]
        #[case::move_in_button_motion(K::Moved, M::ButtonMotion, false)]
        #[case::move_in_any_motion(K::Moved, M::AnyMotion, true)]
        fn mode_decides_which_events_are_sent(
            #[case] kind: MouseEventKind,
            #[case] mode: MouseMode,
            #[case] sent: bool,
        ) {
            assert_eq!(encode_sgr(kind, mode).is_some(), sent);
        }
    }

    mod sgr {
        use super::*;

        #[rstest]
        #[case::left_press(K::Down(LEFT), "\x1b[<0;5;10M")]
        #[case::right_press(K::Down(RIGHT), "\x1b[<2;5;10M")]
        #[case::release_is_lowercase(K::Up(LEFT), "\x1b[<0;5;10m")]
        #[case::drag(K::Drag(LEFT), "\x1b[<32;5;10M")]
        #[case::motion(K::Moved, "\x1b[<35;5;10M")]
        #[case::wheel_up(K::ScrollUp, "\x1b[<64;5;10M")]
        #[case::wheel_down(K::ScrollDown, "\x1b[<65;5;10M")]
        fn encodes_button_and_one_based_coordinates(#[case] kind: MouseEventKind, #[case] expected: &str) {
            assert_eq!(encode_sgr(kind, M::AnyMotion).as_deref(), Some(expected));
        }

        #[test]
        fn modifiers_are_added_to_button_code() {
            let mut ev = event(K::Down(LEFT));
            ev.modifiers = KeyModifiers::SHIFT | KeyModifiers::CONTROL;

            let out = encode(&ev, 0, 0, M::Press, E::Sgr);

            assert_eq!(out.as_deref(), Some(&b"\x1b[<20;1;1M"[..]));
        }
    }

    mod legacy {
        use super::*;

        #[test]
        fn default_encoding_offsets_every_value_by_32() {
            let out = encode(&event(K::Down(LEFT)), 4, 9, M::PressRelease, E::Default);

            assert_eq!(out, Some(vec![0x1b, b'[', b'M', 32, 32 + 5, 32 + 10]));
        }

        #[test]
        fn default_encoding_reports_release_without_button() {
            let out = encode(&event(K::Up(RIGHT)), 4, 9, M::PressRelease, E::Default).expect("release is wanted");

            assert_eq!(out[3], 32 + 3);
        }

        #[test]
        fn default_encoding_clamps_coordinates_that_do_not_fit_a_byte() {
            let out = encode(&event(K::Down(LEFT)), 400, 0, M::Press, E::Default).expect("press is wanted");

            assert_eq!(out[4], u8::MAX);
        }

        #[test]
        fn utf8_encoding_represents_big_coordinates() {
            let out = encode(&event(K::Down(LEFT)), 299, 0, M::Press, E::Utf8).expect("press is wanted");

            let chars: Vec<u32> = String::from_utf8(out).expect("valid utf8").chars().map(u32::from).collect();

            assert_eq!(chars, [0x1b, u32::from(b'['), u32::from(b'M'), 32, 32 + 300, 32 + 1]);
        }
    }
}
