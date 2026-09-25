//! GUI の描画時間とは分離した、再現可能な疑似ストリーム/投影の計測。
use solo::{
    mock::{self, Config, Delivery, FRAME_BATCH, Scenario},
    projection::{Session, Status},
    text::TextBuffer,
};
use std::time::{Duration, Instant};

fn main() {
    let scenario = match std::env::args().nth(1).as_deref() {
        None | Some("100k") => Scenario::Events100k,
        Some("10k") => Scenario::Events10k,
        Some("100mib") => Scenario::Log100MiB,
        Some("faults") => Scenario::Faults,
        _ => {
            eprintln!("usage: phase0_bench [10k|100k|100mib|faults]");
            std::process::exit(2);
        }
    };
    let mut config = Config::new("benchmark", scenario);
    config.delay = Duration::ZERO;
    let started = Instant::now();
    let (controller, receiver) = mock::start(config).expect("start fixture");
    let mut session = Session::new("benchmark".into(), "benchmark".into());
    let mut paths = Vec::new();
    let mut batch_times = Vec::new();
    let mut input_times = Vec::new();
    let mut composer = TextBuffer::default();
    while let Ok(first) = receiver.recv_blocking() {
        let mut batch = vec![first];
        while batch.len() < FRAME_BATCH {
            match receiver.try_recv() {
                Ok(event) => batch.push(event),
                Err(_) => break,
            }
        }
        let batch_started = Instant::now();
        for delivery in batch {
            match delivery {
                Delivery::Event(event) => {
                    session.apply(event);
                }
                Delivery::LogOpened(path) => paths.push(path),
                Delivery::Error(error) => panic!("{error}"),
            }
        }
        batch_times.push(batch_started.elapsed().as_secs_f64() * 1000.);
        let input_started = Instant::now();
        composer.replace(None, "前🙂");
        composer.replace_and_mark(None, "にほんご", Some(4..4));
        assert!(composer.take_committed().is_none());
        composer.replace(None, "日本語");
        assert_eq!(composer.take_committed().as_deref(), Some("前🙂日本語"));
        input_times.push(input_started.elapsed().as_secs_f64() * 1000.);
    }
    session.transport_closed();
    controller.shutdown_and_join().expect("worker exits");
    assert_eq!(
        session.status,
        if scenario == Scenario::Faults {
            Status::Disconnected
        } else {
            Status::Completed
        }
    );
    let disk_bytes: u64 = paths
        .iter()
        .map(|p| std::fs::metadata(&**p).unwrap().len())
        .sum();
    assert_eq!(disk_bytes, session.log_bytes);
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "scenario": scenario.label(), "gui": false, "status": session.status.label(),
        "elapsed_ms": started.elapsed().as_secs_f64() * 1000., "events": session.accepted,
        "unknown": session.unknown, "duplicates": session.duplicates, "rejected": session.rejected,
        "log_disk_bytes": disk_bytes, "log_preview_bytes": session.logs.iter().map(|r| r.text.len()).sum::<usize>(),
        "log_rows": session.logs.len(), "chat_blocks": session.chat.len(),
        "batch_projection_ms": stats(&mut batch_times), "text_buffer_edit_ms": stats(&mut input_times),
        "note": "UI input-to-present / frame / RSS measurements are separate."
    })).unwrap());
}

fn stats(values: &mut [f64]) -> serde_json::Value {
    values.sort_by(f64::total_cmp);
    let count = values.len();
    serde_json::json!({"samples": count, "p50": values[(count * 50).div_ceil(100).saturating_sub(1)], "p95": values[(count * 95).div_ceil(100).saturating_sub(1)], "max": values[count - 1]})
}
