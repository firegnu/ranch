//! Reading the screen, for agents whose hooks leave a state out (R7). The pen keeps a screen only
//! for an adapter that has screen rules, and gives its text with its status; `corral` then checks
//! the rules against it whenever it works out a state.
use std::panic::{AssertUnwindSafe, catch_unwind};

/// The agent's screen, kept by its pen from the agent's output. Reading the screen must never take
/// the pen down: if the terminal parser panics, the screen is dropped and the hooks' state stands.
pub struct Screen {
    parser: Option<vt100::Parser>,
    strings: Strings,
}
impl Screen {
    /// A screen of `size` (rows, columns) that has shown `history`.
    pub fn new(size: (u16, u16), history: &[u8]) -> Self {
        let mut s = Self {
            parser: catch_unwind(|| vt100::Parser::new(size.0, size.1, 0)).ok(),
            strings: Strings::Out,
        };
        s.feed(history);
        s
    }
    fn guard(&mut self, f: impl FnOnce(&mut vt100::Parser)) {
        if let Some(p) = &mut self.parser
            && catch_unwind(AssertUnwindSafe(|| f(p))).is_err()
        {
            self.parser = None;
        }
    }
    pub fn feed(&mut self, data: &[u8]) {
        let data = self.strings.strip(data);
        self.guard(|p| p.process(&data));
    }
    pub fn resize(&mut self, size: (u16, u16)) {
        self.guard(|p| p.screen_mut().set_size(size.0, size.1));
    }
    /// What is on the screen, one line per screen row; none once the screen is dropped.
    pub fn text(&self) -> Option<String> {
        let p = self.parser.as_ref()?;
        catch_unwind(AssertUnwindSafe(|| {
            let cols = p.screen().size().1;
            p.screen().rows(0, cols).collect::<Vec<_>>().join("\n")
        }))
        .ok()
    }
}

/// Where the output is in control strings (OSC, DCS, SOS, PM, APC: `ESC ]`, `ESC P`, `ESC X`,
/// `ESC ^`, `ESC _`). They draw nothing, and the parser would keep an unterminated one in memory
/// without limit, so they are left out before it sees them.
#[derive(Clone, Copy)]
enum Strings {
    Out,
    Escape,
    In,
    InEscape,
}
impl Strings {
    fn strip(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        for &b in data {
            *self = match (*self, b) {
                (Self::Out, 0x1b) => Self::Escape,
                (Self::Out, _) => {
                    out.push(b);
                    Self::Out
                }
                (Self::Escape | Self::InEscape, b']' | b'P' | b'X' | b'^' | b'_') => Self::In,
                // `ESC \` ends a string; elsewhere it is passed on.
                (Self::InEscape, b'\\') => Self::Out,
                (Self::Escape | Self::InEscape, 0x1b) => {
                    if let Self::Escape = self {
                        out.push(0x1b);
                    }
                    Self::Escape
                }
                (Self::Escape | Self::InEscape, _) => {
                    out.extend([0x1b, b]);
                    Self::Out
                }
                // BEL ends a string; CAN and SUB cancel it.
                (Self::In, 0x07 | 0x18 | 0x1a) => Self::Out,
                (Self::In, 0x1b) => Self::InEscape,
                (Self::In, _) => Self::In,
            };
        }
        out
    }
}

/// When the screen shows a state the hooks cannot report: every one of `all` and none of `none`
/// in the last `rows` lines that are not blank, or in the whole screen when `rows` is 0.
pub struct Rule {
    pub rows: usize,
    pub all: &'static [&'static str],
    pub none: &'static [&'static str],
}
impl Rule {
    fn matches(&self, screen: &str) -> bool {
        let lines: Vec<&str> = screen.lines().filter(|l| !l.trim().is_empty()).collect();
        let area = lines[lines.len().saturating_sub(if self.rows == 0 {
            lines.len()
        } else {
            self.rows
        })..]
            .join("\n");
        self.all.iter().all(|s| area.contains(s)) && !self.none.iter().any(|s| area.contains(s))
    }
}
/// Whether any of `rules` matches `screen`.
pub fn matches(rules: &[Rule], screen: &str) -> bool {
    rules.iter().any(|r| r.matches(screen))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screen's rows, without the blank ones at the bottom.
    fn shown(s: &Screen) -> String {
        s.text().unwrap().trim_end_matches('\n').to_owned()
    }

    #[test]
    fn keeps_what_the_screen_shows() {
        let mut s = Screen::new((5, 20), b"");
        s.feed(b"one\r\ntwo\r\nthree");
        assert_eq!(s.text().unwrap(), "one\ntwo\nthree\n\n");
        // Clearing the screen and drawing elsewhere.
        s.feed(b"\x1b[2J\x1b[H\x1b[3;5Hmiddle");
        assert_eq!(shown(&s), "\n\n    middle");
        // A character split across two reads.
        let e = "é".as_bytes();
        s.feed(b"\x1b[2J\x1b[Hcaf");
        s.feed(&e[..1]);
        s.feed(&e[1..]);
        assert_eq!(shown(&s), "café");
        // Scrolling off the top.
        s.feed(b"\x1b[2J\x1b[H1\r\n2\r\n3\r\n4\r\n5\r\n6");
        assert_eq!(shown(&s), "2\n3\n4\n5\n6");
        // A narrower screen: the cursor stops at its last column; a long line wraps onto rows.
        s.resize((5, 3));
        s.feed(b"\x1b[2J\x1b[H\x1b[1;10Hx\r\nabcdef");
        assert_eq!(shown(&s), "  x\nabc\ndef");
    }

    #[test]
    fn a_screen_rebuilt_from_the_history_is_the_same() {
        let history: &[u8] = b"banner\r\n\x1b[2J\x1b[Hprompt> \x1b[1mbold\x1b[0m\r\n\x1b[2;3Hx";
        let mut s = Screen::new((4, 30), b"");
        for b in history {
            s.feed(&[*b]);
        }
        assert_eq!(Screen::new((4, 30), history).text(), s.text());
    }

    #[test]
    fn a_parser_panic_drops_the_screen_and_nothing_else() {
        // vt100 0.16.2 panics when a narrower screen cuts a wide character in two (R7 review).
        let mut s = Screen::new((2, 5), "ab中".as_bytes());
        s.resize((2, 3));
        s.feed(b"x");
        assert_eq!(s.text(), None);
        s.feed(b"more");
        s.resize((40, 120));
        assert_eq!(s.text(), None);
        // A wide character on a one-column screen.
        let mut s = Screen::new((2, 1), b"");
        s.feed("中x".as_bytes());
        s.resize((2, 4));
        s.feed(b"y");
        let _ = s.text();
    }

    #[test]
    fn control_strings_never_reach_the_parser() {
        let mut strings = Strings::Out;
        let mut kept = Vec::new();
        // An unterminated OSC, fed in pieces, keeps nothing however long it gets.
        for chunk in [&b"ok\x1b"[..], b"]52;c;", &[b'a'; 65536], &[b'a'; 65536]] {
            kept.extend(strings.strip(chunk));
        }
        assert_eq!(kept, b"ok");
        // BEL, `ESC \`, CAN and SUB end it; other sequences pass, split or not.
        for (input, out) in [
            (&b"\x07after"[..], &b"after"[..]),
            (b"\x1b]0;t\x1b", b""),
            (b"\\x\x1b[1m", b"x\x1b[1m"),
            (b"\x1bPq#0\x1b\\y", b"y"),
            (b"\x1b_a\x18z\x1b^b\x1az", b"zz"),
            (b"\x1b\x1b[2J\x1b", b"\x1b\x1b[2J"),
            (b"7", b"\x1b7"),
        ] {
            assert_eq!(
                strings.strip(input),
                out,
                "{}",
                String::from_utf8_lossy(input)
            );
        }
        let mut s = Screen::new((3, 20), b"");
        s.feed(b"\x1b]2;title\x07hello\x1b]52;c;");
        s.feed(&[b'a'; 1 << 20]);
        s.feed(b"\x1b\\ world");
        assert_eq!(shown(&s), "hello world");
    }

    #[test]
    fn rules_look_at_the_last_lines_or_the_whole_screen() {
        let screen = "Allow this command?\n\n  › Yes   No\n\nenter to confirm · esc to cancel\n\n";
        let bottom = |all, none| Rule { rows: 2, all, none };
        assert!(matches(
            &[bottom(&["Yes", "enter to confirm"], &[])],
            screen
        ));
        // Only in the last two lines that are not blank.
        assert!(!matches(&[bottom(&["Allow this command?"], &[])], screen));
        assert!(matches(
            &[Rule {
                rows: 0,
                all: &["Allow this command?"],
                none: &[]
            }],
            screen
        ));
        // Every one of `all`, none of `none`.
        assert!(!matches(&[bottom(&["Yes", "Maybe"], &[])], screen));
        assert!(!matches(&[bottom(&["Yes"], &["esc to cancel"])], screen));
        // Any rule.
        assert!(matches(
            &[bottom(&["Maybe"], &[]), bottom(&["No"], &[])],
            screen
        ));
        assert!(!matches(&[], screen));
        assert!(!matches(&[bottom(&["Yes"], &[])], ""));
        // Lines are screen rows, also where a long line wraps.
        let wrapped = Screen::new((4, 6), b"Allow?abcdef").text().unwrap();
        let last = |all| Rule {
            rows: 1,
            all,
            none: &[],
        };
        assert!(!matches(&[last(&["Allow?"])], &wrapped));
        assert!(matches(&[last(&["abcdef"])], &wrapped));
    }
}
