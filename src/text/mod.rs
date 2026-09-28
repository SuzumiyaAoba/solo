//! 表示向けテキストユーティリティ。IME/UTF-16 の編集バッファは `buffer` に分離。
mod buffer;

pub use buffer::{TextBuffer, from_utf16, to_utf16};
use unicode_segmentation::UnicodeSegmentation;

/// 巨大な一行も UTF-8 を壊さず、表示側へ渡す量を制限する。
pub fn preview(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }
    let end = text.floor_char_boundary(max_bytes);
    format!("{}…", &text[..end])
}

/// 最初の依頼をセッション名にする。日本語や絵文字の途中で切らない。
pub fn task_title(prompt: &str) -> String {
    let line = prompt
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("新しいタスク");
    let mut graphemes = line.graphemes(true);
    let mut title = graphemes.by_ref().take(36).collect::<String>();
    if graphemes.next().is_some() {
        title.push('…');
    }
    title
}
