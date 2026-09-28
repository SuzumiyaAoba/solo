use solo::{
    event::SessionId,
    mock::{self, CHANNEL_CAPACITY, Config, Delivery, Scenario},
    projection::{MAX_LOG_ROWS, SessionProjection, Status},
};

fn sid(s: &str) -> SessionId {
    SessionId::parse(s).unwrap()
}
use std::{
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

fn drain(
    receiver: async_channel::Receiver<Delivery>,
    session: &mut SessionProjection,
) -> Vec<Arc<PathBuf>> {
    let mut paths = Vec::new();
    while let Ok(delivery) = receiver.recv_blocking() {
        match delivery {
            Delivery::Event(event) => {
                session.apply(event);
            }
            Delivery::LogOpened(path) => paths.push(path),
            Delivery::Error(error) => panic!("{error}"),
        }
    }
    session.transport_closed();
    paths
}
fn config(scenario: Scenario) -> Config {
    let mut config = Config::new(sid("s1"), scenario);
    config.delay = Duration::ZERO;
    config
}

#[test]
fn ten_thousand_events_keep_lifecycle_and_diff() {
    let (controller, receiver) = mock::start(config(Scenario::Events10k)).unwrap();
    let mut session = SessionProjection::new(sid("s1"), "".into());
    let paths = drain(receiver, &mut session);
    controller.shutdown_and_join().unwrap();
    assert_eq!(session.status(), Status::Completed);
    assert_eq!(session.accepted(), 10_007);
    assert_eq!(session.rejected(), 0);
    assert_eq!(
        session.tools().values().copied().collect::<Vec<_>>(),
        vec![Some(0)]
    );
    assert_eq!(session.diffs().len(), 1);
    assert_eq!(
        std::fs::metadata(&*paths[0]).unwrap().len(),
        session.log_bytes()
    );
}

#[test]
fn faults_are_visible_and_closed_stream_is_not_success() {
    let (controller, receiver) = mock::start(config(Scenario::Faults)).unwrap();
    let mut session = SessionProjection::new(sid("s1"), "".into());
    drain(receiver, &mut session);
    controller.shutdown_and_join().unwrap();
    assert_eq!(session.status(), Status::Disconnected);
    assert_eq!(session.duplicates(), 1);
    assert_eq!(session.unknown(), 2);
    assert_eq!(session.rejected(), 2);
    assert!(session.tools().values().any(Option::is_none));
}

#[test]
fn backpressure_and_cancel_work_when_queue_is_full() {
    let (controller, receiver) = mock::start(config(Scenario::Events100k)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while receiver.len() < CHANNEL_CAPACITY && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(receiver.len(), CHANNEL_CAPACITY);
    controller.cancel();
    let mut session = SessionProjection::new(sid("s1"), "".into());
    drain(receiver, &mut session);
    controller.shutdown_and_join().unwrap();
    assert_eq!(session.status(), Status::Cancelled);
    assert_eq!(session.rejected(), 0);
    assert!(session.accepted() < 100_000);
}

#[test]
fn dropping_receiver_releases_blocked_producer() {
    let (controller, receiver) = mock::start(config(Scenario::Events100k)).unwrap();
    drop(receiver);
    let deadline = Instant::now() + Duration::from_secs(3);
    while !controller.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert!(controller.is_finished());
    controller.shutdown_and_join().unwrap();
}

#[test]
fn immediate_cancel_still_has_valid_turn_identity() {
    let (controller, receiver) = mock::start(config(Scenario::Events100k)).unwrap();
    controller.cancel();
    let mut session = SessionProjection::new(sid("s1"), "".into());
    session.request_cancel();
    drain(receiver, &mut session);
    assert_eq!(session.status(), Status::Cancelled);
    assert_eq!(session.rejected(), 0);
    controller.shutdown_and_join().unwrap();
}

#[test]
fn another_turn_continues_sequence_and_preserves_history() {
    let mut session = SessionProjection::new(sid("s1"), "".into());
    for _ in 0..2 {
        let mut config = config(Scenario::Demo);
        config.start_sequence = session.last_sequence();
        let (controller, receiver) = mock::start(config).unwrap();
        drain(receiver, &mut session);
        controller.shutdown_and_join().unwrap();
        assert_eq!(session.status(), Status::Completed);
    }
    assert_eq!(session.rejected(), 0);
    assert_eq!(session.duplicates(), 0);
    assert_eq!(session.last_sequence(), 253);
}

#[test]
fn concurrent_streams_do_not_mix_sessions() {
    let (c1, r1) = mock::start(config(Scenario::Demo)).unwrap();
    let mut c = config(Scenario::Demo);
    c.session_id = sid("s2");
    let (c2, r2) = mock::start(c).unwrap();
    let mut s1 = SessionProjection::new(sid("s1"), "".into());
    let mut s2 = SessionProjection::new(sid("s2"), "".into());
    drain(r1, &mut s1);
    drain(r2, &mut s2);
    c1.shutdown_and_join().unwrap();
    c2.shutdown_and_join().unwrap();
    assert_eq!(s1.status(), Status::Completed);
    assert_eq!(s2.status(), Status::Completed);
    assert_eq!(s1.rejected() + s2.rejected(), 0);
}

#[test]
fn hundred_mib_giant_line_is_spooled_with_bounded_preview() {
    // 保存領域を渡すと、全文ログはそのディレクトリに残り、アプリ終了後も参照できる。
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(Scenario::Log100MiB);
    config.log_dir = Some(dir.path().join("logs"));
    let (controller, receiver) = mock::start(config).unwrap();
    let mut session = SessionProjection::new(sid("s1"), "".into());
    let paths = drain(receiver, &mut session);
    controller.shutdown_and_join().unwrap();
    assert_eq!(session.status(), Status::Completed);
    assert_eq!(session.log_bytes(), 100 * 1024 * 1024);
    assert_eq!(
        std::fs::metadata(&*paths[0]).unwrap().len(),
        100 * 1024 * 1024
    );
    assert!(paths[0].starts_with(dir.path().join("logs")));
    assert_eq!(session.logs().len(), MAX_LOG_ROWS);
    assert!(
        session
            .logs()
            .iter()
            .map(|row| row.text.len())
            .sum::<usize>()
            < 520_000
    );
    let path = paths[0].to_path_buf();
    drop(paths);
    assert!(
        path.exists(),
        "logs under the session store survive the last reference"
    );
}
