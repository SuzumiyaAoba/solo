use crate::projection::{SessionProjection, Speaker};

/// Speaker を "## ユーザー / ## アシスタント / ## 通知" の見出しに、ブロックを順に連結。
/// 同一 message_id+speaker の連続ブロックは結合して1段落に( block 分割は内部都合)。
/// 末尾にタイトル・status・usage のフッタを付ける(None は「不明」)。
pub(crate) fn transcript_markdown(session: &SessionProjection) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let mut speaker: Option<&Speaker> = None;
    let mut paragraph = String::new();
    let mut paragraph_key: Option<(&str, &Speaker)> = None;
    let flush_paragraph = |out: &mut String, paragraph: &mut String| {
        if !paragraph.is_empty() {
            let _ = writeln!(out, "{paragraph}");
            paragraph.clear();
        }
    };
    for block in session.chat() {
        let key = (block.message_id.as_str(), &block.speaker);
        if paragraph_key != Some(key) {
            flush_paragraph(&mut out, &mut paragraph);
            if speaker != Some(&block.speaker) {
                let heading = match block.speaker {
                    Speaker::User => "## ユーザー",
                    Speaker::Assistant => "## アシスタント",
                    Speaker::Notice => "## 通知",
                };
                if !out.is_empty() {
                    out.push('\n');
                }
                let _ = writeln!(out, "{heading}\n");
                speaker = Some(&block.speaker);
            }
            paragraph_key = Some(key);
        }
        paragraph.push_str(&block.text);
    }
    flush_paragraph(&mut out, &mut paragraph);
    let format_option =
        |value: Option<u64>| value.map_or_else(|| "不明".to_owned(), |value| value.to_string());
    let _ = write!(
        out,
        "\n---\n- title: {}\n- status: {}\n- input_tokens: {}\n- output_tokens: {}\n- cost_usd: {}\n",
        session.title(),
        session.status().label(),
        format_option(session.usage().input_tokens),
        format_option(session.usage().output_tokens),
        session
            .usage()
            .cost_usd
            .map_or_else(|| "不明".to_owned(), |cost| format!("{cost:.4}")),
    );
    out
}
