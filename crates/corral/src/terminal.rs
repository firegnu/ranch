// From Saddle `crates/corral-core/src/terminal.rs` at commit `a31dea2`, unchanged.
use regex::bytes::{Captures, Regex};
use std::{collections::BTreeMap, sync::LazyLock};
fn re(s: &str) -> Regex {
    Regex::new(&format!("(?-u){s}")).unwrap()
}
static CSI: LazyLock<Regex> = LazyLock::new(|| re(r"\x1b\[([0-?]*)([ -/]*)([@-~])"));
static OSC: LazyLock<Regex> = LazyLock::new(|| re(r"\x1b\]([^\x07\x1b]*)(?:\x07|\x1b\\)"));
static ESC: LazyLock<Regex> = LazyLock::new(|| re(r"\x1b([=>])"));
static INCOMPLETE: LazyLock<Regex> =
    LazyLock::new(|| re(r"\x1b(?:\[[0-?]*[ -/]*|\][^\x07\x1b]*\x1b?)?\z"));
const MODES: &[&str] = &[
    "1", "25", "1000", "1002", "1003", "1005", "1006", "1015", "1004", "2004", "47", "1047", "1049",
];
const ALT: &[&str] = &["1049", "1047", "47"];
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Terminal {
    pub title: String,
    modes: BTreeMap<String, u8>,
    kitty: Vec<u32>,
    keypad: bool,
    modify: u32,
    pending: Vec<u8>,
}
impl Terminal {
    pub fn paste(&self) -> bool {
        self.modes.get("2004") == Some(&b'h')
    }
    pub fn feed(&mut self, data: &[u8]) {
        let mut b = std::mem::take(&mut self.pending);
        b.extend_from_slice(data);
        let mut i = 0;
        while let Some(j) = b[i..].iter().position(|v| *v == 27).map(|n| n + i) {
            let mut found = false;
            for (kind, regex) in [(0, &*CSI), (1, &*OSC), (2, &*ESC)] {
                if let Some(c) = regex
                    .captures(&b[j..])
                    .filter(|c| c.get(0).unwrap().start() == 0)
                {
                    self.apply(kind, &c);
                    i = j + c.get(0).unwrap().end();
                    found = true;
                    break;
                }
            }
            if !found {
                if b.len() - j <= 4096 && INCOMPLETE.find(&b[j..]).is_some_and(|m| m.start() == 0) {
                    self.pending.extend_from_slice(&b[j..]);
                    break;
                }
                i = j + 1;
            }
        }
    }
    fn apply(&mut self, kind: u8, c: &Captures<'_>) {
        let p = &c[1];
        if kind == 1 {
            if let Some(i) = p.iter().position(|b| *b == b';')
                && (p[..i] == *b"0" || p[..i] == *b"2")
            {
                self.title = String::from_utf8_lossy(&p[i + 1..]).into_owned();
            }
            return;
        }
        if kind == 2 {
            self.keypad = p == b"=";
            return;
        }
        let final_byte = c[3][0];
        if p.starts_with(b"?") && matches!(final_byte, b'h' | b'l') {
            for s in String::from_utf8_lossy(&p[1..]).split(';') {
                if MODES.contains(&s) {
                    self.modes.insert(s.into(), final_byte);
                }
            }
        } else if final_byte == b'u' && p.first().is_some_and(|b| b"><=".contains(b)) {
            let nums: Vec<u32> = String::from_utf8_lossy(&p[1..])
                .split(';')
                .map(|s| s.parse().unwrap_or(0))
                .collect();
            let flags = nums[0];
            match p[0] {
                b'>' => self.kitty.push(flags),
                b'<' => self
                    .kitty
                    .truncate(self.kitty.len().saturating_sub(flags.max(1) as usize)),
                _ => {
                    let current = self.kitty.pop().unwrap_or(0);
                    let mode = nums.get(1).copied().unwrap_or(1).max(1);
                    self.kitty.push(match mode {
                        1 => flags,
                        2 => current | flags,
                        _ => current & !flags,
                    });
                }
            }
        } else if final_byte == b'm' && p.starts_with(b">4") {
            let nums: Vec<&str> = std::str::from_utf8(&p[1..])
                .unwrap_or("")
                .split(';')
                .collect();
            if nums[0] == "4" {
                self.modify = nums.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
        }
    }
    pub fn replay(&self) -> Vec<u8> {
        self.sequence(false)
    }
    pub fn restore(&self) -> Vec<u8> {
        self.sequence(true)
    }
    fn sequence(&self, restore: bool) -> Vec<u8> {
        let changed: Vec<_> = self
            .modes
            .iter()
            .filter(|(k, v)| **v != if k.as_str() == "25" { b'h' } else { b'l' })
            .collect();
        let mut s = String::new();
        if !restore {
            for a in ALT {
                if self.modes.get(*a) == Some(&b'h') {
                    s += &format!("\x1b[?{a}h");
                }
            }
        }
        for (k, v) in changed {
            if !ALT.contains(&k.as_str()) {
                let ch = if restore {
                    if k == "25" { 'h' } else { 'l' }
                } else {
                    *v as char
                };
                s += &format!("\x1b[?{k}{ch}");
            }
        }
        if restore {
            if !self.kitty.is_empty() {
                s += &format!("\x1b[<{}u", self.kitty.len());
            }
            if self.keypad {
                s += "\x1b>"
            }
            if self.modify != 0 {
                s += "\x1b[>4m"
            }
            for a in ALT {
                if self.modes.get(*a) == Some(&b'h') {
                    s += &format!("\x1b[?{a}l");
                }
            }
        } else {
            if self.keypad {
                s += "\x1b="
            }
            if self.modify != 0 {
                s += &format!("\x1b[>4;{}m", self.modify)
            }
            for f in &self.kitty {
                s += &format!("\x1b[>{f}u");
            }
        }
        s.into_bytes()
    }
}
static AUTO: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"\x1b\[[IO]|\x1b\[\d+;\d+R|\x1b\[[?>=][\d;]*c|\x1b\[\d*n|\x1b\[\?[\d;]*\$y|\x1b\[\?\d*u|\x1b\[\d+;\d+;\d+t|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1bP[^\x1b]*\x1b\\",
    )
});
static MOUSE: LazyLock<Regex> = LazyLock::new(|| re(r"\x1b\[<(\d+);\d+;\d+[Mm]"));
pub fn human(data: &[u8]) -> bool {
    let b = AUTO.replace_all(data, &b""[..]);
    let b = MOUSE.replace_all(&b, |c: &Captures<'_>| {
        let n = std::str::from_utf8(&c[1])
            .unwrap_or("")
            .parse::<u32>()
            .unwrap_or(0);
        if n & 32 != 0 && n & 3 == 3 && n & 64 == 0 {
            Vec::new()
        } else {
            c[0].to_vec()
        }
    });
    !b.is_empty()
}
static DETACH_U: LazyLock<Regex> = LazyLock::new(|| re(r"\x1b\[93(?::[\d:]*)?;(\d+)(?::(\d+))?u"));
static DETACH_M: LazyLock<Regex> = LazyLock::new(|| re(r"\x1b\[27;(\d+);93~"));
pub fn detach(data: &[u8]) -> Option<usize> {
    let mut hits = Vec::new();
    if let Some(i) = data.iter().position(|b| *b == 29) {
        hits.push(i)
    }
    for (r, kitty) in [(&*DETACH_U, true), (&*DETACH_M, false)] {
        for c in r.captures_iter(data) {
            let m = std::str::from_utf8(&c[1])
                .unwrap_or("")
                .parse::<u32>()
                .unwrap_or(0);
            if m > 0
                && (m - 1) & !(64 | 128) == 4
                && (!kitty || c.get(2).is_none_or(|s| s.as_bytes() == b"1"))
            {
                hits.push(c.get(0).unwrap().start())
            }
        }
    }
    hits.into_iter().min()
}
static CONTROL: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[P^_][^\x1b]*\x1b\\|\x1b[@-Z\\-_=>]",
    )
});
pub fn strip(data: &[u8]) -> String {
    let b = CONTROL.replace_all(data, &b""[..]);
    let s = String::from_utf8_lossy(&b)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    s.chars()
        .filter(|c| !matches!(*c,'\0'..='\x08'|'\x0b'..='\x1f'|'\x7f'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_modes_replay_before_input_and_restore_on_detach() {
        let mut t = Terminal::default();
        for part in [
            b"\x1b[?1049h\x1b[?200".as_slice(),
            b"4h\x1b[?25l\x1b[>1u\x1b[>4;2m\x1b]2;title\x1b",
            b"\\",
        ] {
            t.feed(part)
        }
        assert_eq!(t.title, "title");
        assert!(t.paste());
        let replay = String::from_utf8(t.replay()).unwrap();
        assert!(replay.starts_with("\x1b[?1049h"));
        assert!(replay.contains("\x1b[?2004h"));
        assert!(replay.contains("\x1b[>1u"));
        let restore = String::from_utf8(t.restore()).unwrap();
        assert!(restore.contains("\x1b[?25h"));
        assert!(restore.contains("\x1b[?2004l"));
        assert!(restore.contains("\x1b[<1u"));
        assert!(restore.ends_with("\x1b[?1049l"));
    }
    #[test]
    fn only_human_activity_and_detach_key_presses_are_actionable() {
        assert!(!human(
            b"\x1b[12;3R\x1b[<35;4;5M\x1b]11;rgb:0000/0000/0000\x07"
        ));
        for data in [b"text".as_slice(), b"\x1b[<0;4;5M", b"\x1b[<64;4;5M"] {
            assert!(human(data))
        }
        assert_eq!(detach(b"x\x1d"), Some(1));
        assert_eq!(detach(b"\x1b[93;69u"), Some(0));
        assert_eq!(detach(b"\x1b[27;5;93~"), Some(0));
        assert_eq!(detach(b"\x1b[93;5:3u"), None);
        assert_eq!(detach(b"\x1b[93;5:2u"), None);
        assert_eq!(detach(b"\x1b[93;7u"), None);
    }
}
