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
