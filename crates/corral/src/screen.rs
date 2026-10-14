//! Reading the screen, for agents whose hooks leave a state out (R7). The pen keeps a screen only
//! for an adapter that has screen rules, and gives its text with its status; `corral` then checks
//! the rules against it whenever it works out a state.

/// The agent's screen, kept by its pen from the agent's output.
pub struct Screen(vt100::Parser);
impl Screen {
    /// A screen of `size` (rows, columns) that has shown `history`.
    pub fn new(size: (u16, u16), history: &[u8]) -> Self {
        let mut s = Self(vt100::Parser::new(size.0, size.1, 0));
        s.feed(history);
        s
    }
    pub fn feed(&mut self, data: &[u8]) {
        self.0.process(data);
    }
    pub fn resize(&mut self, size: (u16, u16)) {
        self.0.screen_mut().set_size(size.0, size.1);
    }
    /// What is on the screen, one line per row.
    pub fn text(&self) -> String {
        self.0.screen().contents()
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

    #[test]
    fn keeps_what_the_screen_shows() {
        let mut s = Screen::new((5, 20), b"");
        s.feed(b"one\r\ntwo\r\nthree");
        assert_eq!(s.text(), "one\ntwo\nthree");
        // Clearing the screen and drawing elsewhere.
        s.feed(b"\x1b[2J\x1b[H\x1b[3;5Hmiddle");
        assert_eq!(s.text(), "\n\n    middle");
        // A character split across two reads.
        let e = "é".as_bytes();
        s.feed(b"\x1b[2J\x1b[Hcaf");
        s.feed(&e[..1]);
        s.feed(&e[1..]);
        assert_eq!(s.text(), "café");
        // Scrolling off the top.
        s.feed(b"\x1b[2J\x1b[H1\r\n2\r\n3\r\n4\r\n5\r\n6");
        assert_eq!(s.text(), "2\n3\n4\n5\n6");
        // A narrower screen: the cursor stops at its last column; a wrapped line stays one line.
        s.resize((5, 3));
        s.feed(b"\x1b[2J\x1b[H\x1b[1;10Hx\r\nabcdef");
        assert_eq!(s.text(), "  x\nabcdef");
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
    }
}
