//! ACP 接続の状態、stdio ワーカー、プロセスの寿命。
use super::{Config, Delivery, reader::Reader, stdio, writer::Writer};
use crate::{acp::Client, harness::Cancellation};
use async_channel::{Receiver, Sender};
use serde_json::json;
use std::{
    io::{self, BufReader},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, MutexGuard},
};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
enum State {
    #[default]
    Starting,
    Idle(String),
    Running {
        session_id: String,
        cancellation: Cancellation,
    },
    Disconnected,
}

type SharedChild = Arc<Mutex<Option<Child>>>;

pub(super) struct Connection {
    child: SharedChild,
    state: Mutex<State>,
    writer: Writer,
    prompts: Sender<String>,
    shutdown: CancellationToken,
    pub(super) output: Sender<Delivery>,
}

impl Connection {
    pub(super) fn is_disconnected(&self) -> bool {
        self.shutdown.is_cancelled()
    }

    pub(super) fn initialized(&self, session_id: String) -> io::Result<()> {
        let mut state = lock(&self.state);
        if !matches!(*state, State::Starting) {
            return Err(closed());
        }
        *state = State::Idle(session_id);
        Ok(())
    }

    pub(super) fn start_turn(&self, cancellation: Cancellation) -> io::Result<()> {
        let mut state = lock(&self.state);
        let State::Idle(session_id) = &*state else {
            return Err(closed());
        };
        if self.shutdown.is_cancelled() {
            cancellation.cancel();
        }
        *state = State::Running {
            session_id: session_id.clone(),
            cancellation,
        };
        Ok(())
    }

    pub(super) fn finish_turn(&self) {
        let mut state = lock(&self.state);
        if let State::Running { session_id, .. } = &*state {
            *state = State::Idle(session_id.clone());
        }
    }

    fn cancel(&self) {
        {
            let state = lock(&self.state);
            if let State::Running {
                session_id,
                cancellation,
            } = &*state
            {
                if cancellation.is_cancelled() {
                    return;
                }
                // キューへの投入後に承認待ちを解除し、cancel 通知が返答に先行する順序を保つ。
                if self.writer.notify(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":session_id}})).is_ok() {
                    cancellation.cancel();
                    return;
                }
            }
        }
        self.disconnect();
    }

    fn disconnect(&self) {
        self.transport_closed();
        *lock(&self.state) = State::Disconnected;
        self.output.close();
    }

    fn transport_closed(&self) {
        self.shutdown.cancel();
        {
            let state = lock(&self.state);
            if let State::Running { cancellation, .. } = &*state {
                cancellation.cancel();
            }
        }
        self.prompts.close();
        self.writer.close();
        stop_child(&self.child);
    }
}

pub struct Controller {
    connection: Arc<Connection>,
}

impl Controller {
    pub fn prompt(&self, text: String) -> io::Result<()> {
        self.connection.prompts.try_send(text).map_err(|_| closed())
    }
    pub fn cancel(&self) {
        self.connection.cancel();
    }
    pub fn disconnect(&self) {
        self.connection.disconnect();
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.disconnect();
    }
}

struct ProcessCleanup(SharedChild);
impl Drop for ProcessCleanup {
    fn drop(&mut self) {
        let child = {
            let mut slot = lock(&self.0);
            if let Some(child) = slot.as_mut() {
                let _ = child.kill();
            }
            slot.take()
        };
        // 待機中に UI が同じ mutex で止まらないよう、所有権を取り出してから回収する。
        if let Some(mut child) = child {
            let _ = child.wait();
        }
    }
}

pub(super) struct WorkerCleanup {
    _process: ProcessCleanup,
    pub(super) connection: Arc<Connection>,
}

impl Drop for WorkerCleanup {
    fn drop(&mut self) {
        self.connection.disconnect();
    }
}

pub(super) struct Transport {
    pub(super) controller: Controller,
    pub(super) receiver: Receiver<Delivery>,
    pub(super) client: Client<BufReader<Reader>, Writer>,
    pub(super) input: Receiver<String>,
    pub(super) cleanup: WorkerCleanup,
}

impl Transport {
    pub(super) fn start(config: &Config) -> io::Result<Self> {
        let mut child = Command::new(&config.profile.command)
            .args(&config.profile.args)
            .current_dir(&config.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stdin = child.stdin.take().expect("piped stdin");
        let child = Arc::new(Mutex::new(Some(child)));
        // writer / worker のどちらのスレッド生成に失敗しても、子プロセスを回収する。
        let process = ProcessCleanup(child.clone());
        let (reader, writer, io_worker) = stdio::channel();
        let (prompts, input) = async_channel::bounded(1);
        let (output, receiver) = async_channel::bounded(256);
        let connection = Arc::new(Connection {
            child,
            state: Mutex::new(State::Starting),
            writer: writer.clone(),
            prompts,
            shutdown: CancellationToken::new(),
            output,
        });
        let weak = Arc::downgrade(&connection);
        io_worker.start(stdin, stdout, connection.shutdown.clone(), move || {
            if let Some(connection) = weak.upgrade() {
                connection.transport_closed();
            }
        })?;
        Ok(Self {
            controller: Controller {
                connection: connection.clone(),
            },
            receiver,
            client: Client::new(BufReader::new(reader), writer),
            input,
            cleanup: WorkerCleanup {
                connection,
                _process: process,
            },
        })
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

fn stop_child(child: &SharedChild) {
    if let Some(child) = lock(child).as_mut() {
        let _ = child.kill();
    }
}

fn closed() -> io::Error {
    io::Error::new(
        io::ErrorKind::BrokenPipe,
        "ACP agent に入力を送れませんでした",
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::tests::test_agent;
    use super::*;
    use std::{
        thread,
        time::{Duration, Instant},
    };

    const INITIALIZE: &str = r#"#!/bin/sh
IFS= read -r line
echo '{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":1}}'
IFS= read -r line
echo '{"jsonrpc":"2.0","id":1,"result":{"sessionId":"acp-1"}}'
"#;

    fn wait_until(check: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !check() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(check(), "worker state did not settle");
    }

    fn wait_ready(receiver: &Receiver<Delivery>) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match receiver.try_recv() {
                Ok(Delivery::Event(_)) => return,
                Ok(Delivery::Error(error)) => panic!("{error}"),
                _ if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                _ => panic!("agent did not initialize"),
            }
        }
    }

    #[test]
    fn disconnect_releases_a_reader_while_a_descendant_holds_stdout() {
        let body = r#"(
  : > holding
  i=0
  while [ ! -f release ] && [ "$i" -lt 100 ]; do
    sleep 0.05
    i=$((i + 1))
  done
  : > released
) &
IFS= read -r line
IFS= read -r line
"#;
        let (dir, controller, receiver) = test_agent(&format!("{INITIALIZE}{body}"));
        wait_ready(&receiver);
        wait_until(|| dir.path().join("holding").exists());
        controller.prompt("wait for response".into()).unwrap();
        // prompt の送信後に応答待ちへ入ったことを確認する。
        wait_until(|| matches!(*lock(&controller.connection.state), State::Running { .. }));
        let worker = Arc::downgrade(&controller.connection);
        controller.disconnect();
        let deadline = Instant::now() + Duration::from_secs(1);
        while worker.strong_count() > 1 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let stopped_before_pipe_closed = worker.strong_count() == 1;
        std::fs::write(dir.path().join("release"), "").unwrap();
        wait_until(|| dir.path().join("released").exists());
        wait_until(|| worker.strong_count() == 1);
        assert!(
            stopped_before_pipe_closed,
            "disconnect must not depend on an inherited stdout closing"
        );
    }

    #[test]
    fn disconnect_releases_a_writer_while_a_descendant_holds_stdin() {
        let body = r#"exec 3<&0
(
  dd bs=1 count=1 <&3 > first-byte 2>/dev/null
  i=0
  while [ ! -f release ] && [ "$i" -lt 100 ]; do
    sleep 0.05
    i=$((i + 1))
  done
  : > released
) &
wait
"#;
        let (dir, controller, receiver) = test_agent(&format!("{INITIALIZE}{body}"));
        wait_ready(&receiver);
        controller.prompt("x".repeat(1024 * 1024)).unwrap();
        wait_until(|| {
            std::fs::metadata(dir.path().join("first-byte"))
                .is_ok_and(|metadata| metadata.len() == 1)
        });
        let worker = Arc::downgrade(&controller.connection);
        controller.disconnect();
        let deadline = Instant::now() + Duration::from_secs(1);
        while worker.strong_count() > 1 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let stopped_before_pipe_closed = worker.strong_count() == 1;
        std::fs::write(dir.path().join("release"), "").unwrap();
        wait_until(|| dir.path().join("released").exists());
        wait_until(|| worker.strong_count() == 1);
        assert!(
            stopped_before_pipe_closed,
            "disconnect must interrupt a partially written request"
        );
    }

    #[test]
    fn final_response_is_delivered_before_transport_eof() {
        use crate::projection::{SessionProjection, Status};
        for _ in 0..8 {
            let body = r#"IFS= read -r line
echo '{"jsonrpc":"2.0","id":2,"result":{"stopReason":"end_turn"}}'
"#;
            let (_dir, controller, receiver) = test_agent(&format!("{INITIALIZE}{body}"));
            controller.prompt("finish before exiting".into()).unwrap();
            let mut session = SessionProjection::new(
                crate::event::SessionId::parse("local").unwrap(),
                "test".into(),
            );
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                match receiver.try_recv() {
                    Ok(Delivery::Event(event)) => {
                        session.apply(event);
                    }
                    Ok(Delivery::Error(error)) => panic!("{error}"),
                    Err(async_channel::TryRecvError::Closed) => break,
                    _ if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                    _ => panic!("transport did not finish"),
                }
            }
            assert_eq!(
                session.status(),
                Status::Completed,
                "EOF must preserve the last buffered response"
            );
            assert_eq!(session.rejected(), 0);
            assert!(controller.prompt("after EOF".into()).is_err());
        }
    }

    #[test]
    fn disconnect_releases_an_idle_worker_and_rejects_new_prompts() {
        let (_dir, controller, receiver) = test_agent(&format!("{INITIALIZE}IFS= read -r line\n"));
        wait_ready(&receiver);
        let worker = Arc::downgrade(&controller.connection);
        controller.disconnect();
        assert!(controller.prompt("after disconnect".into()).is_err());
        assert!(receiver.is_closed());
        // cleanup の connection は process の回収後に解放され、controller だけが残る。
        wait_until(|| worker.strong_count() == 1);
    }

    #[test]
    fn dropping_controller_releases_approval_wait_even_while_ui_owns_the_reply() {
        let body = r#"IFS= read -r line
echo '{"jsonrpc":"2.0","id":77,"method":"session/request_permission","params":{"sessionId":"acp-1","toolCall":{"toolCallId":"one"},"options":[{"optionId":"a","kind":"allow_once"}]}}'
IFS= read -r line
"#;
        let (_dir, controller, receiver) = test_agent(&format!("{INITIALIZE}{body}"));
        wait_ready(&receiver);
        controller.prompt("wait for permission".into()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let reply = loop {
            match receiver.try_recv() {
                Ok(Delivery::Approval { reply, .. }) => break reply,
                Ok(Delivery::Error(error)) => panic!("{error}"),
                _ if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                _ => panic!("agent did not request permission"),
            }
        };
        let worker = Arc::downgrade(&controller.connection);
        drop(controller);
        assert!(receiver.is_closed());
        wait_until(|| reply.is_closed() && worker.strong_count() == 0);
    }

    #[test]
    fn disconnect_releases_a_worker_blocked_by_a_full_delivery_queue() {
        let body = r#"IFS= read -r line
i=0
while [ "$i" -lt 2000 ]; do
  echo '{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"acp-1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"delta"}}}}'
  i=$((i + 1))
done
IFS= read -r line
"#;
        let (_dir, controller, receiver) = test_agent(&format!("{INITIALIZE}{body}"));
        wait_ready(&receiver);
        controller.prompt("fill the queue".into()).unwrap();
        wait_until(|| receiver.is_full());
        let worker = Arc::downgrade(&controller.connection);
        controller.disconnect();
        assert!(receiver.is_closed());
        wait_until(|| worker.strong_count() == 1);
    }
}
