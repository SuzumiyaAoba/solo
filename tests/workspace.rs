use serde_json::json;
use solo::harness::{Cancellation, ToolCall, ToolExecutor, workspace::WorkspaceTools};
use std::{
    thread,
    time::{Duration, Instant},
};

fn exec(tools: &mut WorkspaceTools, command: &str) -> solo::harness::ToolResult {
    tools.execute(&ToolCall {
        id: "command".into(),
        name: "exec".into(),
        arguments: json!({"command":command}),
    })
}

#[test]
fn file_reads_enforce_byte_limits_and_require_utf8() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("text");
    std::fs::write(&path, "日本語🙂").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let call = ToolCall {
        id: "read".into(),
        name: "read".into(),
        arguments: json!({"path":"text"}),
    };
    tools.max_output_bytes = "日本語🙂".len();
    let result = tools.execute(&call);
    assert!(!result.is_error);
    assert_eq!(result.content, "日本語🙂");
    tools.max_output_bytes -= 1;
    assert!(tools.execute(&call).is_error);
    std::fs::write(&path, [0xff]).unwrap();
    assert!(tools.execute(&call).is_error);
}

#[test]
fn search_skips_oversized_and_non_utf8_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("note.txt"), "first\n候補が一致\n").unwrap();
    std::fs::write(dir.path().join("large"), "候補".repeat(1024 * 1024 / 6 + 1)).unwrap();
    std::fs::write(dir.path().join("binary"), [0xff]).unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let result = tools.execute(&ToolCall {
        id: "search".into(),
        name: "search".into(),
        arguments: json!({"query":"候補"}),
    });
    assert!(!result.is_error);
    assert_eq!(result.content, "note.txt:2:候補が一致\n");
}

#[test]
fn nonzero_exit_is_a_tool_error_with_stdout_and_stderr_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let result = exec(&mut tools, "printf output; printf problem >&2; exit 7");
    assert!(
        result.is_error,
        "a failed command must not be reported as successful"
    );
    assert!(result.content.contains("output"));
    assert!(result.content.contains("problem"));
    assert!(result.content.lines().next().unwrap().contains('7'));
    assert!(!exec(&mut tools, "printf ok").is_error);
}

#[test]
fn command_output_stays_within_its_byte_limit_after_utf8_decoding() {
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    for limit in 0..=8 {
        tools.max_output_bytes = limit;
        let result = exec(&mut tools, "printf 'あい'; printf 'うえ' >&2");
        assert!(!result.is_error);
        let (_, output) = result.content.split_once('\n').unwrap();
        assert!(
            output.len() <= limit,
            "{limit} byte limit produced {} bytes: {output:?}",
            output.len()
        );
        assert!(
            !output.contains('\u{fffd}'),
            "a truncated UTF-8 character must not become a replacement character"
        );
    }
    tools.max_output_bytes = 4;
    let result = exec(&mut tools, "printf '\\377\\377\\377'");
    let (_, output) = result.content.split_once('\n').unwrap();
    assert!(
        output.len() <= 4,
        "replacement characters also consume the byte budget"
    );
}

#[test]
fn commands_honor_timeout_and_cancellation_before_and_during_execution() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = Cancellation::default();
    cancel.cancel();
    let mut tools = WorkspaceTools::new(dir.path())
        .unwrap()
        .with_cancellation(cancel);
    assert!(
        exec(&mut tools, "printf unexpected > marker")
            .content
            .contains("中止")
    );
    assert!(!dir.path().join("marker").exists());

    let cancel = Cancellation::default();
    let mut tools = WorkspaceTools::new(dir.path())
        .unwrap()
        .with_cancellation(cancel.clone());
    tools.timeout = Duration::from_millis(20);
    let result = exec(&mut tools, "exec sleep 5");
    assert!(result.is_error);
    assert!(result.content.contains("制限時間"));

    tools.timeout = Duration::from_secs(10);
    thread::scope(|scope| {
        let worker = scope.spawn(|| exec(&mut tools, "printf ready > ready; exec sleep 5"));
        let deadline = Instant::now() + Duration::from_secs(3);
        while !dir.path().join("ready").exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        cancel.cancel();
        let result = worker.join().unwrap();
        assert!(
            dir.path().join("ready").exists(),
            "the command must start before cancellation"
        );
        assert!(result.is_error);
        assert!(result.content.contains("中止"));
    });
}

#[test]
fn search_ignores_vendor_directories() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("node_modules/dep")).unwrap();
    std::fs::write(dir.path().join("node_modules/dep/index.js"), "needle\n").unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "needle\n").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let result = tools.execute(&ToolCall {
        id: "search".into(),
        name: "search".into(),
        arguments: json!({"query":"needle"}),
    });
    assert!(!result.is_error);
    assert_eq!(result.content, "src/lib.rs:1:needle\n");
}

#[cfg(unix)]
#[test]
fn search_skips_unreadable_directories() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::write(locked.join("secret.txt"), "needle\n").unwrap();
    std::fs::write(dir.path().join("open.txt"), "needle\n").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let result = tools.execute(&ToolCall {
        id: "search".into(),
        name: "search".into(),
        arguments: json!({"query":"needle"}),
    });
    // テスト後に tempdir を消せるよう権限を戻してから検査する。
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!result.is_error);
    assert_eq!(result.content, "open.txt:1:needle\n");
}

#[test]
fn search_reports_partial_results_at_the_entry_cap() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..5 {
        std::fs::write(dir.path().join(format!("f{i}.txt")), "needle\n").unwrap();
    }
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    tools.max_search_entries = 3;
    let result = tools.execute(&ToolCall {
        id: "search".into(),
        name: "search".into(),
        arguments: json!({"query":"needle"}),
    });
    assert!(!result.is_error, "the entry cap must not fail the search");
    assert!(result.content.contains("検索対象の上限"));
    let matches = result
        .content
        .lines()
        .filter(|line| !line.starts_with('['))
        .count();
    assert!(matches <= 3, "{matches} match lines: {:?}", result.content);
}

fn call_tool(
    tools: &mut WorkspaceTools,
    name: &str,
    arguments: serde_json::Value,
) -> solo::harness::ToolResult {
    tools.execute(&ToolCall {
        id: name.into(),
        name: name.into(),
        arguments,
    })
}

#[test]
fn read_supports_line_ranges_and_reports_position() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("lines.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let result = call_tool(
        &mut tools,
        "read",
        json!({"path":"lines.txt","offset":2,"limit":2}),
    );
    assert!(!result.is_error);
    assert_eq!(result.content, "two\nthree\n[行 2-3 / 全 4 行]");
    // limit の既定は 2000 で残り全部、offset の既定は 1。
    let result = call_tool(&mut tools, "read", json!({"path":"lines.txt","offset":3}));
    assert_eq!(result.content, "three\nfour\n[行 3-4 / 全 4 行]");
    // offset が行数を超えるとエラー。
    let result = call_tool(&mut tools, "read", json!({"path":"lines.txt","offset":5}));
    assert!(result.is_error);
    assert!(result.content.contains("offset 5 がファイルの行数 4"));
    // 0 は受け付けない。
    let result = call_tool(&mut tools, "read", json!({"path":"lines.txt","offset":0}));
    assert!(result.is_error);
    assert!(result.content.contains("1 以上"));
}

#[test]
fn full_read_error_suggests_ranges_and_ranged_read_covers_larger_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    tools.max_output_bytes = 64;
    std::fs::write(dir.path().join("big.txt"), "x".repeat(200)).unwrap();
    let result = call_tool(&mut tools, "read", json!({"path":"big.txt"}));
    assert!(result.is_error);
    assert!(result.content.contains("offset と limit"));
    // 範囲指定なら上限超過のファイルも行単位で読める。
    let result = call_tool(
        &mut tools,
        "read",
        json!({"path":"big.txt","offset":1,"limit":1}),
    );
    assert!(!result.is_error);
    assert!(
        result
            .content
            .contains("[行 1-1 / 全 1 行 · 出力上限のため 1 行目までを表示]")
    );
}

#[test]
fn ranged_read_stops_at_full_lines() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("lines.txt"), "aaa\nbbb\nccc\n").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    tools.max_output_bytes = 5;
    let result = call_tool(
        &mut tools,
        "read",
        json!({"path":"lines.txt","offset":1,"limit":3}),
    );
    assert_eq!(
        result.content,
        "aaa\n[行 1-1 / 全 3 行 · 出力上限のため 1 行目までを表示]"
    );
}

#[test]
fn list_shows_directories_and_glob_matches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/deep")).unwrap();
    std::fs::create_dir_all(dir.path().join("node_modules/dep")).unwrap();
    std::fs::write(dir.path().join("b.txt"), "b").unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "").unwrap();
    std::fs::write(dir.path().join("src/deep/mod.rs"), "").unwrap();
    std::fs::write(dir.path().join("node_modules/dep/index.js"), "").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    // 直下は除外ディレクトリも名前順で出す。
    let result = call_tool(&mut tools, "list", json!({}));
    assert_eq!(result.content, "Cargo.toml\nb.txt\nnode_modules/\nsrc/\n");
    // glob は workspace ルートからの相対パスで返し、除外ディレクトリには入らない。
    let result = call_tool(&mut tools, "list", json!({"pattern":"**/*.rs"}));
    assert_eq!(result.content, "src/deep/mod.rs\nsrc/lib.rs\n");
    // / を含まない glob は直下のファイル名だけに一致する。
    let result = call_tool(&mut tools, "list", json!({"pattern":"*.toml"}));
    assert_eq!(result.content, "Cargo.toml\n");
    // workspace 外は拒否する。
    let outside = tempfile::tempdir().unwrap();
    let result = call_tool(
        &mut tools,
        "list",
        json!({"path":outside.path().to_str().unwrap()}),
    );
    assert!(result.is_error);
    let result = call_tool(&mut tools, "list", json!({"pattern":"**/*.js"}));
    assert_eq!(result.content, "(一致するファイルはありません)");
}

#[test]
fn search_filters_by_path_glob_and_regex() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src/util")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "needle in rust\n").unwrap();
    std::fs::write(dir.path().join("src/util/mod.rs"), "needle deeper\n").unwrap();
    std::fs::write(dir.path().join("note.txt"), "needle in text\n").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    // path でディレクトリを絞る。
    let result = call_tool(&mut tools, "search", json!({"query":"needle","path":"src"}));
    assert_eq!(
        result.content,
        "src/lib.rs:1:needle in rust\nsrc/util/mod.rs:1:needle deeper\n"
    );
    // path でファイルを絞る。
    let result = call_tool(
        &mut tools,
        "search",
        json!({"query":"needle","path":"note.txt"}),
    );
    assert_eq!(result.content, "note.txt:1:needle in text\n");
    // ファイル名だけの glob。
    let result = call_tool(
        &mut tools,
        "search",
        json!({"query":"needle","glob":"*.txt"}),
    );
    assert_eq!(result.content, "note.txt:1:needle in text\n");
    // パス形式の glob。
    let result = call_tool(
        &mut tools,
        "search",
        json!({"query":"needle","glob":"src/**/*.rs"}),
    );
    assert_eq!(
        result.content,
        "src/lib.rs:1:needle in rust\nsrc/util/mod.rs:1:needle deeper\n"
    );
    // regex。
    let result = call_tool(
        &mut tools,
        "search",
        json!({"query":"needle (in|deeper)","regex":true}),
    );
    assert!(!result.is_error);
    assert_eq!(result.content.lines().count(), 3);
    // 不正な正規表現はエラー。
    let result = call_tool(&mut tools, "search", json!({"query":"(","regex":true}));
    assert!(result.is_error);
    assert!(result.content.contains("正規表現が不正です"));
    // 一致ゼロ。
    let result = call_tool(&mut tools, "search", json!({"query":"missing"}));
    assert_eq!(result.content, "(一致する行はありません)");
}

#[test]
fn edit_replace_all_and_ambiguous_match_errors() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("note.txt"), "old mid old end old").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let result = call_tool(
        &mut tools,
        "edit",
        json!({"path":"note.txt","old":"old","new":"new"}),
    );
    assert!(result.is_error);
    assert!(result.content.contains("3 箇所に一致"));
    let result = call_tool(
        &mut tools,
        "edit",
        json!({"path":"note.txt","old":"old","new":"new","replace_all":true}),
    );
    assert!(!result.is_error);
    assert!(result.content.contains("3 箇所を置換"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("note.txt")).unwrap(),
        "new mid new end new"
    );
}

#[test]
fn write_creates_files_and_guards_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    // 新規作成は親ディレクトリも作る。
    let result = call_tool(
        &mut tools,
        "write",
        json!({"path":"sub/dir/new.txt","content":"a\nb\n"}),
    );
    assert!(!result.is_error);
    assert!(
        result
            .content
            .contains("sub/dir/new.txt を作成しました（2 行）")
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("sub/dir/new.txt")).unwrap(),
        "a\nb\n"
    );
    // 未 read の既存ファイルは拒否。
    std::fs::write(dir.path().join("existing.txt"), "old").unwrap();
    let result = call_tool(
        &mut tools,
        "write",
        json!({"path":"existing.txt","content":"new"}),
    );
    assert!(result.is_error);
    assert!(result.content.contains("先に read"));
    // read 済みなら上書きできる。
    call_tool(&mut tools, "read", json!({"path":"existing.txt"}));
    let result = call_tool(
        &mut tools,
        "write",
        json!({"path":"existing.txt","content":"new\n"}),
    );
    assert!(!result.is_error);
    assert!(
        result
            .content
            .contains("existing.txt を上書きしました（1 行）")
    );
    // read 後の外部変更は拒否する。
    std::fs::write(dir.path().join("existing.txt"), "outside").unwrap();
    let result = call_tool(
        &mut tools,
        "write",
        json!({"path":"existing.txt","content":"again"}),
    );
    assert!(result.is_error);
    assert!(result.content.contains("もう一度 read"));
}

#[test]
fn write_rejects_paths_outside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    for raw in [
        "../escape.txt".to_owned(),
        outside.path().join("abs.txt").to_str().unwrap().to_owned(),
    ] {
        let result = call_tool(&mut tools, "write", json!({"path":raw,"content":"x"}));
        assert!(result.is_error, "{raw} must be rejected");
    }
    // 外を指す symlink ディレクトリ経由も拒否する。
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        let result = call_tool(
            &mut tools,
            "write",
            json!({"path":"link/out.txt","content":"x"}),
        );
        assert!(result.is_error);
    }
    // ディレクトリへの書き込みは拒否する。
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let result = call_tool(&mut tools, "write", json!({"path":"sub","content":"x"}));
    assert!(result.is_error);
    assert!(result.content.contains("ディレクトリ"));
    // 1 MiB を超える content は拒否する。
    let result = call_tool(
        &mut tools,
        "write",
        json!({"path":"huge.txt","content":"x".repeat(1024 * 1024 + 1)}),
    );
    assert!(result.is_error);
    assert!(result.content.contains("1 MiB"));
}

#[test]
fn exec_timeout_seconds_and_output_sharing() {
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    for seconds in [0, 601] {
        let result = tools.execute(&ToolCall {
            id: "exec".into(),
            name: "exec".into(),
            arguments: json!({"command":"true","timeout_seconds":seconds}),
        });
        assert!(result.is_error);
        assert!(result.content.contains("timeout_seconds"));
    }
    // stdout が巨大でも小さい stderr はすべて残る。
    let result = exec(
        &mut tools,
        "head -c 100000 /dev/zero | tr '\\0' 'o'; printf err >&2",
    );
    assert!(result.content.contains("err"));
    assert!(result.content.contains("バイト省略"));
    // 予算が十分なら省略せず出力の末尾も残る。
    let result = exec(&mut tools, "printf 'start'; printf 'end'");
    assert!(result.content.starts_with("exit:"));
    assert!(result.content.ends_with("startend"));
    assert!(!result.content.contains("省略"));
    // 長い出力でも末尾が残る。
    tools.max_output_bytes = 256;
    let result = exec(&mut tools, "seq 1 100");
    assert!(result.content.contains("バイト省略"));
    assert!(result.content.trim_end().ends_with("100"));
}

#[test]
fn ranged_read_preserves_crlf_line_endings() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("crlf.txt"), "one\r\ntwo\r\nthree\r\n").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    let result = call_tool(
        &mut tools,
        "read",
        json!({"path":"crlf.txt","offset":1,"limit":2}),
    );
    assert!(!result.is_error);
    assert_eq!(result.content, "one\r\ntwo\r\n[行 1-2 / 全 3 行]");
    // CRLF の行は \r を含むので、edit の old とも一致する。
    let result = call_tool(
        &mut tools,
        "edit",
        json!({"path":"crlf.txt","old":"two\r\nthree","new":"二\r\n三"}),
    );
    assert!(!result.is_error, "{}", result.content);
}

#[test]
fn binary_output_stays_within_the_share_even_with_replacement_growth() {
    let dir = tempfile::tempdir().unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();
    tools.max_output_bytes = 256;
    let result = exec(&mut tools, "head -c 3000 /dev/zero | tr '\\0' '\\377'");
    let (_, output) = result.content.split_once('\n').unwrap();
    assert!(
        output.len() <= 256,
        "{} bytes after the exit line",
        output.len()
    );
}

/// 各ワークスペースツールは実行スレッドに表示する 1 行の要約を返す。
#[test]
fn tools_return_one_line_summaries() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("note.txt"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "needle\n").unwrap();
    let mut tools = WorkspaceTools::new(dir.path()).unwrap();

    let result = call_tool(&mut tools, "read", json!({"path":"note.txt"}));
    assert_eq!(result.summary.as_deref(), Some("3 行"));
    let result = call_tool(
        &mut tools,
        "read",
        json!({"path":"note.txt","offset":2,"limit":1}),
    );
    assert_eq!(result.summary.as_deref(), Some("行 2-2 / 全 3 行"));

    // ルートには note.txt と src/ の 2 件。
    let result = call_tool(&mut tools, "list", json!({}));
    assert_eq!(result.summary.as_deref(), Some("2 件"));

    let result = call_tool(&mut tools, "search", json!({"query":"needle"}));
    assert_eq!(result.summary.as_deref(), Some("1 件の一致"));

    let result = call_tool(
        &mut tools,
        "edit",
        json!({"path":"note.txt","old":"two","new":"二"}),
    );
    assert_eq!(
        result.summary.as_deref(),
        Some("note.txt の 1 箇所を置き換え")
    );

    let result = call_tool(
        &mut tools,
        "write",
        json!({"path":"made.txt","content":"hello"}),
    );
    assert_eq!(
        result.summary.as_deref(),
        Some("made.txt を作成（5 バイト）")
    );
    call_tool(&mut tools, "read", json!({"path":"made.txt"}));
    let result = call_tool(
        &mut tools,
        "write",
        json!({"path":"made.txt","content":"hi!"}),
    );
    assert_eq!(
        result.summary.as_deref(),
        Some("made.txt を上書き（3 バイト）")
    );

    let result = exec(&mut tools, "printf ok");
    assert_eq!(
        result.summary.as_deref(),
        Some("終了コード 0 · stdout 2 バイト / stderr 0 バイト")
    );
    // 非ゼロ終了でも終了コードを含む要約が付く。
    let result = exec(&mut tools, "exit 3");
    assert!(result.is_error);
    assert!(
        result
            .summary
            .as_deref()
            .unwrap()
            .starts_with("終了コード 3")
    );
}
