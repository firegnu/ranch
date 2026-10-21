use super::{Error, Result};

#[derive(Clone, Debug)]
pub struct Part {
    pub body: String,
    pub opening: String,
    pub closing: String,
}
impl Part {
    pub fn render(&self, index: usize, total: usize) -> String {
        format!(
            "{}{}{}{}",
            self.opening,
            self.body,
            self.closing,
            if total > 1 {
                format!("\n({}/{total})", index + 1)
            } else {
                String::new()
            }
        )
    }
}
#[derive(Clone)]
struct Fence {
    marker: String,
    opening: String,
}
struct Line {
    start: usize,
    end: usize,
    after: Option<Fence>,
    blank: bool,
    table: bool,
}
fn lines(text: &str) -> Vec<Line> {
    let mut fence: Option<Fence> = None;
    let mut offset = 0;
    let mut result = Vec::new();
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        let mark = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
        if let Some(mark) = mark {
            let n = trimmed.chars().take_while(|c| *c == mark).count();
            if n >= 3 {
                if let Some(open) = &fence {
                    if open.marker.starts_with(mark)
                        && n >= open.marker.len()
                        && trimmed[n..].trim().is_empty()
                    {
                        fence = None;
                    }
                } else {
                    fence = Some(Fence {
                        marker: mark.to_string().repeat(n),
                        opening: format!("{trimmed}\n"),
                    });
                }
            }
        }
        result.push(Line {
            start: offset,
            end: offset + line.len(),
            after: fence.clone(),
            blank: trimmed.is_empty(),
            table: fence.is_none() && trimmed.contains('|'),
        });
        offset += line.len();
    }
    result
}
fn context(lines: &[Line], offset: usize) -> Option<&Fence> {
    lines[..lines.partition_point(|l| l.end <= offset)]
        .last()
        .and_then(|l| l.after.as_ref())
}
fn piece(text: &str, lines: &[Line], start: usize, end: usize) -> Part {
    let body = &text[start..end];
    let opening = if start > 0 {
        context(lines, start)
            .map(|f| f.opening.clone())
            .unwrap_or_default()
    } else {
        String::new()
    };
    let closing = if end < text.len() {
        context(lines, end)
            .map(|f| {
                format!(
                    "{}{}",
                    if body.ends_with('\n') { "" } else { "\n" },
                    f.marker
                )
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    Part {
        body: body.into(),
        opening,
        closing,
    }
}
/// Body slices are byte-exact. All additions live separately in opening/closing.
/// Reserve numbering space before choosing boundaries so the rendered limit holds.
pub fn split(text: &str, limit: usize) -> Result<Vec<Part>> {
    let text = if text.is_empty() {
        "（空回复）"
    } else {
        text
    };
    if text.chars().count() <= limit {
        return Ok(vec![Part {
            body: text.into(),
            opening: String::new(),
            closing: String::new(),
        }]);
    }
    let lines = lines(text);
    let reserve = 2 * text.chars().count().to_string().len() + 4;
    let mut parts = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = start;
        let opening = context(&lines, start).map_or(0, |f| f.opening.chars().count());
        for (count, (offset, c)) in text[start..].char_indices().enumerate() {
            let candidate = start + offset + c.len_utf8();
            let closing = if candidate < text.len() {
                context(&lines, candidate).map_or(0, |f| f.marker.len() + usize::from(c != '\n'))
            } else {
                0
            };
            if opening + count + 1 + closing + reserve > limit {
                break;
            }
            end = candidate;
        }
        if end == start {
            return Err(Error::new(
                "message_limit",
                "message limit cannot fit the code fence and numbering",
            ));
        }
        if end < text.len() {
            // Keep a table whole when it fits in a fresh segment.
            let mut table_start = None;
            let mut table_end = 0;
            for line in &lines {
                if line.table {
                    table_start.get_or_insert(line.start);
                    table_end = line.end;
                } else if let Some(begin) = table_start.take()
                    && begin > start
                    && begin < end
                    && table_end > end
                    && text[begin..table_end].chars().count() + reserve <= limit
                {
                    end = begin;
                    break;
                }
            }
            if let Some(begin) = table_start
                && begin > start
                && begin < end
                && table_end > end
                && text[begin..table_end].chars().count() + reserve <= limit
            {
                end = begin;
            }
            let candidates: Vec<_> = lines
                .iter()
                .filter(|l| l.end > start && l.end <= end)
                .collect();
            if let Some(line) = candidates
                .iter()
                .rev()
                .find(|l| l.blank && l.after.is_none())
                .or_else(|| candidates.iter().rev().find(|l| l.after.is_none()))
                .or_else(|| candidates.last())
            {
                end = line.end;
            }
        }
        parts.push(piece(text, &lines, start, end));
        start = end;
    }
    Ok(parts)
}
