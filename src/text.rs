//! macOS の UTF-16 範囲と Rust の UTF-8 範囲の変換、および IME composition。
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Default)]
pub struct TextBuffer {
    pub content: String,
    pub selection: Range<usize>,
    pub reversed: bool,
    pub marked: Option<Range<usize>>,
}

pub fn from_utf16(text: &str, offset: usize) -> usize {
    let mut count = 0;
    for (index, ch) in text.char_indices() {
        if count >= offset {
            return index;
        }
        count += ch.len_utf16();
    }
    text.len()
}

pub fn to_utf16(text: &str, offset: usize) -> usize {
    text.char_indices()
        .take_while(|(index, _)| *index < offset)
        .map(|(_, ch)| ch.len_utf16())
        .sum()
}

impl TextBuffer {
    pub fn range_from_utf16(&self, range: Range<usize>) -> Range<usize> {
        let start = from_utf16(&self.content, range.start);
        start..from_utf16(&self.content, range.end).max(start)
    }

    pub fn range_to_utf16(&self, range: Range<usize>) -> Range<usize> {
        to_utf16(&self.content, range.start)..to_utf16(&self.content, range.end)
    }

    pub fn cursor(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    pub fn move_to(&mut self, offset: usize, extend: bool) {
        let offset = self.content.floor_char_boundary(offset);
        if extend {
            let anchor = if self.reversed {
                self.selection.end
            } else {
                self.selection.start
            };
            self.reversed = offset < anchor;
            self.selection = offset.min(anchor)..offset.max(anchor);
        } else {
            self.selection = offset..offset;
            self.reversed = false;
        }
    }

    pub fn previous(&self) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find(|(i, _)| *i < self.cursor())
            .map_or(0, |(i, _)| i)
    }

    pub fn next(&self) -> usize {
        self.content
            .grapheme_indices(true)
            .find(|(i, _)| *i > self.cursor())
            .map_or(self.content.len(), |(i, _)| i)
    }

    fn replacement_range(&self, range: Option<Range<usize>>) -> Range<usize> {
        range
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone())
    }

    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str) {
        let range = self.replacement_range(range);
        self.content.replace_range(range.clone(), text);
        let cursor = range.start + text.len();
        self.selection = cursor..cursor;
        self.reversed = false;
        self.marked = None;
    }

    pub fn replace_and_mark(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) {
        let range = self.replacement_range(range);
        self.content.replace_range(range.clone(), text);
        self.marked = (!text.is_empty()).then_some(range.start..range.start + text.len());
        // selected は「挿入した文字列」に対する UTF-16 範囲。
        // 既存 content や置換前 range.end を基準にしない。
        self.selection = selected
            .map(|selection| {
                let start = from_utf16(text, selection.start);
                let end = from_utf16(text, selection.end).max(start);
                range.start + start..range.start + end
            })
            .unwrap_or(range.start + text.len()..range.start + text.len());
        self.reversed = false;
    }

    pub fn backspace(&mut self) {
        if self.selection.is_empty() {
            self.move_to(self.previous(), true);
        }
        self.replace(None, "");
    }

    pub fn delete(&mut self) {
        if self.selection.is_empty() {
            self.move_to(self.next(), true);
        }
        self.replace(None, "");
    }

    pub fn take_committed(&mut self) -> Option<String> {
        if self.marked.is_some() || self.content.trim().is_empty() {
            return None;
        }
        let text = std::mem::take(&mut self.content);
        self.selection = 0..0;
        self.reversed = false;
        Some(text)
    }
}
