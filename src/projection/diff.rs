//! Unified diff の行番号・追加削除数と表示上限。
use crate::event::preview;

pub const MAX_DIFF_LINES: usize = 20_000;
const MAX_LINE_BYTES: usize = 2_048;

#[derive(Clone, Debug)]
pub struct DiffLine {
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
    pub kind: DiffKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Header,
    Context,
    Added,
    Removed,
}

#[derive(Debug)]
pub struct Diff {
    pub path: String,
    pub lines: Vec<DiffLine>,
    pub truncated: bool,
    pub reviewed: bool,
}

impl Diff {
    pub fn parse(path: String, text: &str) -> Self {
        let mut old = LineRange::default();
        let mut new = LineRange::default();
        let mut lines = Vec::new();
        let mut truncated = false;
        for line in text.lines() {
            if lines.len() == MAX_DIFF_LINES {
                truncated = true;
                break;
            }
            truncated |= line.len() > MAX_LINE_BYTES;
            let in_hunk = old.remaining > 0 || new.remaining > 0;
            let kind = if line.starts_with("@@") {
                let mut parts = line.split_whitespace().skip(1);
                old = LineRange::parse(parts.next(), '-');
                new = LineRange::parse(parts.next(), '+');
                DiffKind::Header
            } else if line.starts_with("diff ") || line.starts_with("index ") {
                old.remaining = 0;
                new.remaining = 0;
                DiffKind::Header
            } else if line.starts_with('\\')
                || (!in_hunk && (line.starts_with("+++") || line.starts_with("---")))
            {
                DiffKind::Header
            } else if line.starts_with('+') {
                DiffKind::Added
            } else if line.starts_with('-') {
                DiffKind::Removed
            } else {
                DiffKind::Context
            };
            let uses_old = matches!(kind, DiffKind::Context | DiffKind::Removed);
            let uses_new = matches!(kind, DiffKind::Context | DiffKind::Added);
            let old_number = uses_old.then(|| old.take()).flatten();
            let new_number = uses_new.then(|| new.take()).flatten();
            truncated |= (uses_old && old_number.is_none()) || (uses_new && new_number.is_none());
            lines.push(DiffLine {
                old: old_number,
                new: new_number,
                text: preview(line, MAX_LINE_BYTES),
                kind,
            });
        }
        Self {
            path,
            lines,
            truncated,
            reviewed: false,
        }
    }

    pub fn line_counts(&self) -> (usize, usize) {
        self.lines.iter().fold((0, 0), |(added, removed), line| {
            (
                added + usize::from(line.kind == DiffKind::Added),
                removed + usize::from(line.kind == DiffKind::Removed),
            )
        })
    }
}

struct LineRange {
    next: Option<u32>,
    remaining: u64,
}

impl Default for LineRange {
    fn default() -> Self {
        // hunk 見出しのない簡易差分も従来どおり 0 から表示する。
        Self {
            next: Some(0),
            remaining: 0,
        }
    }
}

impl LineRange {
    fn parse(range: Option<&str>, sign: char) -> Self {
        let Some(range) = range.and_then(|range| range.strip_prefix(sign)) else {
            return Self::default();
        };
        let (start, count) = range.split_once(',').unwrap_or((range, "1"));
        Self {
            next: start.parse().ok(),
            remaining: count.parse().unwrap_or(0),
        }
    }

    fn take(&mut self) -> Option<u32> {
        let number = self.next;
        self.next = number.and_then(|number| number.checked_add(1));
        self.remaining = self.remaining.saturating_sub(1);
        number
    }
}
