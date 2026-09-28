//! Unified diff の行番号・追加削除数と表示上限。
use crate::event::preview;

pub const MAX_DIFF_LINES: usize = 20_000;
const MAX_LINE_BYTES: usize = 2_048;

#[derive(Clone, Debug)]
pub struct DiffLine {
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
    /// 符号(+/-/空白)を除いた本文中の変化範囲。隣接する削除・追加行の語レベル差分。
    pub changed: Option<std::ops::Range<usize>>,
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
    /// 描画対象の行だけを `lines` のインデックスで保持する(---/+++ 等のノイズを除外)。
    pub rows: Vec<usize>,
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
                changed: None,
                kind,
            });
        }
        // diff --git や ---/+++ のメタ行はエディタ同様に描画しない。
        // @@ と \ (改行なし) の注記は残す。
        let rows: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                line.kind != DiffKind::Header
                    || line.text.starts_with("@@")
                    || line.text.starts_with('\\')
            })
            .map(|(i, _)| i)
            .collect();
        // 連続する削除行と追加行をペアにして語レベルの差分範囲を求める。
        for i in 0..lines.len() {
            if lines[i].kind != DiffKind::Added {
                continue;
            }
            // 直前の同種ブロック内で、未ペアの削除行を探す。
            let Some(j) = (0..i)
                .rev()
                .take_while(|&k| lines[k].kind != DiffKind::Header)
                .find(|&k| lines[k].kind == DiffKind::Removed && lines[k].changed.is_none())
            else {
                continue;
            };
            let added_body = lines[i].text.get(1..).unwrap_or("");
            let removed_body = lines[j].text.get(1..).unwrap_or("");
            let (ra, rb) = changed_ranges(removed_body, added_body);
            if !ra.is_empty() || !rb.is_empty() {
                lines[j].changed = (!ra.is_empty()).then_some(ra);
                lines[i].changed = (!rb.is_empty()).then_some(rb);
            }
        }
        Self {
            path,
            lines,
            rows,
            truncated,
            reviewed: false,
        }
    }

    /// 表示行ベースでの @@ ハンク見出しの行番号(スクロール用)。
    pub fn hunk_rows(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|item| {
                self.lines[*item.1].kind == DiffKind::Header
                    && self.lines[*item.1].text.starts_with("@@")
            })
            .map(|(row, _)| row)
            .collect()
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

/// 削除行・追加行の本文(符号を除く)から、共通 prefix/suffix を除いた差分範囲を両側に返す。
/// 行全体の強調の内側に、変化した部分だけをさらに濃く出すために使う。
fn changed_ranges(a: &str, b: &str) -> (std::ops::Range<usize>, std::ops::Range<usize>) {
    let prefix = a
        .char_indices()
        .zip(b.chars())
        .take_while(|((_, x), y)| *x == *y)
        .map(|((i, c), _)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    let (mut sa, mut sb) = (a.len(), b.len());
    for ((ia, x), (ib, y)) in a.char_indices().rev().zip(b.char_indices().rev()) {
        if x != y || ia < prefix || ib < prefix {
            break;
        }
        sa = ia;
        sb = ib;
    }
    (prefix..sa.max(prefix), prefix..sb.max(prefix))
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
