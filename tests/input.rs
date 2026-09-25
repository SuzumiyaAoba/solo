use solo::text::{TextBuffer, from_utf16, to_utf16};

#[test]
fn utf16_roundtrips_at_every_utf8_character_boundary() {
    let text = "日本語🙂👨‍👩‍👧‍👦e\u{301}";
    for (index, _) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        assert_eq!(from_utf16(text, to_utf16(text, index)), index);
    }
    for offset in 0..100 {
        assert!(text.is_char_boundary(from_utf16(text, offset)));
    }
}

#[test]
fn ime_selection_is_relative_to_inserted_text_after_emoji_prefix() {
    let mut buffer = TextBuffer::default();
    buffer.replace(None, "前🙂後");
    buffer.move_to("前🙂".len(), false);
    buffer.replace_and_mark(None, "にほん🙂", Some(1..5));
    assert_eq!(&buffer.content[buffer.selection.clone()], "ほん🙂");
    assert_eq!(&buffer.content[buffer.marked.clone().unwrap()], "にほん🙂");
    assert!(buffer.take_committed().is_none());
    buffer.replace_and_mark(None, "日本語", Some(3..3));
    assert_eq!(buffer.cursor(), "前🙂日本語".len());
    buffer.replace(None, "日本語");
    assert!(buffer.marked.is_none());
    assert_eq!(buffer.take_committed().unwrap(), "前🙂日本語後");
}

#[test]
fn explicit_utf16_range_overrides_marked_range() {
    let mut buffer = TextBuffer::default();
    buffer.replace(None, "A🙂B");
    buffer.replace(Some(1..3), "日本語");
    assert_eq!(buffer.content, "A日本語B");
}

#[test]
fn deleting_graphemes_never_splits_emoji_or_combining_marks() {
    let mut buffer = TextBuffer::default();
    buffer.replace(None, "あe\u{301}👨‍👩‍👧‍👦🧑🏽‍💻");
    buffer.backspace();
    assert_eq!(buffer.content, "あe\u{301}👨‍👩‍👧‍👦");
    buffer.backspace();
    assert_eq!(buffer.content, "あe\u{301}");
    buffer.backspace();
    assert_eq!(buffer.content, "あ");
    buffer.move_to(0, false);
    buffer.delete();
    assert!(buffer.content.is_empty());
}

#[test]
fn selection_can_reverse_direction_and_collapse() {
    let mut buffer = TextBuffer::default();
    buffer.replace(None, "abcd");
    buffer.move_to(2, false);
    buffer.move_to(0, true);
    assert!(buffer.reversed);
    assert_eq!(buffer.selection, 0..2);
    buffer.move_to(4, true);
    assert!(!buffer.reversed);
    assert_eq!(buffer.selection, 2..4);
    buffer.move_to(1, false);
    assert!(!buffer.reversed);
    assert_eq!(buffer.selection, 1..1);
}

#[test]
fn cancelling_composition_keeps_prefix_and_suffix() {
    let mut buffer = TextBuffer::default();
    buffer.replace(None, "前後");
    buffer.move_to(3, false);
    buffer.replace_and_mark(None, "へんかん", None);
    buffer.replace_and_mark(None, "", None);
    assert_eq!(buffer.content, "前後");
    assert!(buffer.marked.is_none());
    assert_eq!(buffer.selection, 3..3);
}
