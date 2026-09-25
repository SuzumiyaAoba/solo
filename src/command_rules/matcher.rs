//! Anchored `*` matching and conservative shell-boundary checks for auto-approval.
use serde_json::Value;
use std::ops::Range;

#[derive(Clone, Copy, PartialEq)]
enum Token {
    Byte(u8),
    Star,
}

fn tokens(pattern: &str) -> Vec<Token> {
    let bytes = pattern.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if matches!(bytes.get(i + 1), Some(b'*' | b'\\')) => {
                i += 1;
                result.push(Token::Byte(bytes[i]));
            }
            b'*' => result.push(Token::Star),
            byte => result.push(Token::Byte(byte)),
        }
        i += 1;
    }
    result
}

pub(super) fn wildcard_matches(pattern: &str, text: &str, auto_allow: bool) -> bool {
    let pattern = tokens(pattern);
    if !glob(&pattern, text.as_bytes()) {
        return false;
    }
    if !auto_allow || !pattern.contains(&Token::Star) {
        return true;
    }
    // A star may vary arguments, but must never consume an additional shell operator.
    // This is deliberately a limited lexer, not an attempted full shell interpreter.
    let shape: Vec<_> = pattern
        .iter()
        .map(|token| match token {
            Token::Byte(b) => *b,
            Token::Star => b'*',
        })
        .collect();
    let Some((pattern_parts, pattern_ops)) = shell_parts(&shape) else {
        return false;
    };
    let Some((text_parts, text_ops)) = shell_parts(text.as_bytes()) else {
        return false;
    };
    pattern_ops == text_ops
        && pattern_parts.len() == text_parts.len()
        && pattern_parts
            .into_iter()
            .zip(text_parts)
            .all(|(p, t)| glob(&pattern[p], &text.as_bytes()[t]))
}

/// Match literal islands with KMP instead of recursive / quadratic wildcard backtracking.
fn glob(pattern: &[Token], text: &[u8]) -> bool {
    let mut chunks: Vec<Vec<u8>> = vec![Vec::new()];
    for token in pattern {
        match token {
            Token::Byte(byte) => chunks.last_mut().unwrap().push(*byte),
            Token::Star => chunks.push(Vec::new()),
        }
    }
    if chunks.len() == 1 {
        return chunks[0] == text;
    }
    let first = &chunks[0];
    let last = chunks.last().unwrap();
    if !text.starts_with(first) || !text.ends_with(last) || first.len() + last.len() > text.len() {
        return false;
    }
    let mut offset = first.len();
    let end = text.len() - last.len();
    for chunk in &chunks[1..chunks.len() - 1] {
        if chunk.is_empty() {
            continue;
        }
        let Some(index) = find(&text[offset..end], chunk) else {
            return false;
        };
        offset += index + chunk.len();
    }
    true
}

fn find(text: &[u8], needle: &[u8]) -> Option<usize> {
    let mut prefix = vec![0; needle.len()];
    for i in 1..needle.len() {
        let mut length = prefix[i - 1];
        while length > 0 && needle[i] != needle[length] {
            length = prefix[length - 1];
        }
        if needle[i] == needle[length] {
            length += 1;
        }
        prefix[i] = length;
    }
    let mut length = 0;
    for (i, &byte) in text.iter().enumerate() {
        while length > 0 && byte != needle[length] {
            length = prefix[length - 1];
        }
        if byte == needle[length] {
            length += 1;
        }
        if length == needle.len() {
            return Some(i + 1 - length);
        }
    }
    None
}

type ShellParts<'a> = (Vec<Range<usize>>, Vec<&'a [u8]>);
fn shell_parts(text: &[u8]) -> Option<ShellParts<'_>> {
    let mut parts = Vec::new();
    let mut operators = Vec::new();
    let mut quote = None;
    let mut start = 0;
    let mut word_start = true;
    let mut i = 0;
    while i < text.len() {
        let byte = text[i];
        if quote == Some(b'\'') {
            if byte == b'\'' {
                quote = None;
            }
            i += 1;
            continue;
        }
        if byte == b'\\' {
            if i + 1 == text.len() {
                return None;
            }
            i += 2;
            word_start = false;
            continue;
        }
        if matches!(byte, b'$' | b'`') {
            return None;
        }
        if quote == Some(b'"') {
            if byte == b'"' {
                quote = None;
            }
            i += 1;
            continue;
        }
        if matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
            word_start = false;
            i += 1;
            continue;
        }
        // Expansions, grouping, comments and heredocs need full shell parsing; ask instead.
        if matches!(byte, b'(' | b')' | b'{' | b'}' | b'\r')
            || (byte == b'#' && word_start)
            || text[i..].starts_with(b"<<")
        {
            return None;
        }
        if matches!(byte, b';' | b'&' | b'|' | b'<' | b'>' | b'\n') {
            let length = if [
                b"&&", b"||", b">>", b">|", b">&", b"<&", b"<>", b"|&", b";;",
            ]
            .iter()
            .any(|operator| text[i..].starts_with(*operator))
            {
                2
            } else {
                1
            };
            parts.push(start..i);
            operators.push(&text[i..i + length]);
            i += length;
            start = i;
            word_start = true;
        } else {
            word_start = matches!(byte, b' ' | b'\t');
            i += 1;
        }
    }
    if quote.is_some() {
        return None;
    }
    parts.push(start..text.len());
    Some((parts, operators))
}

/// Keep the input representation and all non-command fields (env, shell, timeout, etc.).
pub(super) fn command_context(input: &Value) -> Option<Value> {
    command_text(input)?;
    let mut metadata = input.as_object()?.clone();
    let key = if metadata.contains_key("command") {
        "command"
    } else {
        "cmd"
    };
    let value = metadata.remove(key)?;
    let form = if value.is_array() {
        "argv"
    } else if metadata.contains_key("args") {
        metadata.remove("args");
        "program_args"
    } else {
        "shell"
    };
    Some(serde_json::json!({"command_field": key, "form": form, "input": metadata}))
}

pub(crate) fn command_text(input: &Value) -> Option<String> {
    let command = input.get("command").or_else(|| input.get("cmd"))?;
    if let Some(command) = command.as_str() {
        if let Some(args) = input.get("args") {
            let args: Option<Vec<_>> = args.as_array()?.iter().map(Value::as_str).collect();
            return Some(
                std::iter::once(command)
                    .chain(args?)
                    .map(quote_arg)
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        return Some(command.to_owned());
    }
    let args: Option<Vec<_>> = command.as_array()?.iter().map(Value::as_str).collect();
    let args = args?;
    if args.is_empty() {
        return None;
    }
    Some(
        args.into_iter()
            .map(quote_arg)
            .collect::<Vec<_>>()
            .join(" "),
    )
}
fn quote_arg(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "_./-:=@".contains(ch))
    {
        arg.into()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}
