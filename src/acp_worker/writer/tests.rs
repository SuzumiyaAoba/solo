use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, mpsc},
    task::{Context, Poll},
    thread,
    time::{Duration, Instant},
};
use tokio::io::AsyncWrite;
use tokio_util::sync::CancellationToken;

struct GatedOutput {
    gate: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,
    entered: Option<mpsc::Sender<()>>,
    finished: mpsc::Sender<()>,
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
    fail: bool,
}

impl AsyncWrite for GatedOutput {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Some(entered) = self.entered.take() {
            let _ = entered.send(());
        }
        if let Some(gate) = &mut self.gate {
            std::task::ready!(gate.as_mut().poll(cx));
            self.gate = None;
        }
        if self.fail {
            return Poll::Ready(Err(closed()));
        }
        self.frames.lock().unwrap().push(bytes.to_vec());
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
impl Drop for GatedOutput {
    fn drop(&mut self) {
        let _ = self.finished.send(());
    }
}

fn spawn(
    output: GatedOutput,
    writes: Writes,
    shutdown: CancellationToken,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _ = runtime.block_on(shutdown.run_until_cancelled(writes.pump(output)));
        writes.close();
    })
}

#[test]
fn notifications_do_not_wait_for_io_and_remain_in_order() {
    let (release, gate) = async_channel::bounded(1);
    let (entered, started) = mpsc::channel();
    let (finished, stopped) = mpsc::channel();
    let frames = Arc::new(Mutex::new(Vec::new()));
    let output = GatedOutput {
        gate: Some(Box::pin(async move {
            let _ = gate.recv().await;
        })),
        entered: Some(entered),
        finished,
        frames: frames.clone(),
        fail: false,
    };
    let (writer, writes) = Writer::channel();
    let worker = spawn(output, writes, CancellationToken::new());
    let mut normal = writer.clone();
    let request = thread::spawn(move || normal.write_all(b"request\n"));
    let entered = started.recv_timeout(Duration::from_secs(1));
    let control = writer.clone();
    let (done, result) = mpsc::channel();
    let notify = thread::spawn(move || {
        let _ = done.send(control.notify(serde_json::json!({"method":"session/cancel"})));
    });
    let notification = result.recv_timeout(Duration::from_secs(1));
    let _ = release.try_send(());
    let request_result = request.join().unwrap();
    notify.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    while frames.lock().unwrap().len() < 2 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    writer.close();
    worker.join().unwrap();
    stopped.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(entered.is_ok());
    notification
        .expect("notification must return while the first write is blocked")
        .unwrap();
    request_result.unwrap();
    let frames = frames.lock().unwrap();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0], b"request\n");
    assert_eq!(
        serde_json::from_slice::<Value>(&frames[1]).unwrap(),
        serde_json::json!({"method":"session/cancel"})
    );
}

#[test]
fn closing_or_failing_a_writer_releases_queued_receipts() {
    for fail in [false, true] {
        let (release, gate) = async_channel::bounded(1);
        let (entered, started) = mpsc::channel();
        let (finished, stopped) = mpsc::channel();
        let output = GatedOutput {
            gate: Some(Box::pin(async move {
                let _ = gate.recv().await;
            })),
            entered: Some(entered),
            finished,
            frames: Arc::default(),
            fail,
        };
        let (writer, writes) = Writer::channel();
        let probe = writes.0.clone();
        let worker = spawn(output, writes, CancellationToken::new());
        let mut first = writer.clone();
        let first = thread::spawn(move || first.write_all(b"first"));
        let entered = started.recv_timeout(Duration::from_secs(1));
        let mut second = writer.clone();
        let (done, result) = mpsc::channel();
        let second = thread::spawn(move || {
            let _ = done.send(second.write_all(b"second"));
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while writer.0.is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let queued = !writer.0.is_empty();
        if !fail {
            writer.close();
        }
        let _ = release.try_send(());
        let first_result = first.join().unwrap();
        let second_result = result.recv_timeout(Duration::from_secs(1));
        writer.close();
        if second_result.is_err() {
            while let Ok(frame) = probe.try_recv() {
                if let Some(done) = frame.done {
                    let _ = done.try_send(Err(closed()));
                }
            }
        }
        second.join().unwrap();
        worker.join().unwrap();
        stopped.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(entered.is_ok() && queued);
        assert_eq!(first_result.is_err(), fail);
        assert!(
            second_result
                .expect("queued write must receive a failure")
                .is_err()
        );
    }
}

#[test]
fn shutdown_interrupts_the_current_write_without_releasing_the_sink() {
    let (release, gate) = async_channel::bounded::<()>(1);
    let (entered, started) = mpsc::channel();
    let (finished, stopped) = mpsc::channel();
    let output = GatedOutput {
        gate: Some(Box::pin(async move {
            let _ = gate.recv().await;
        })),
        entered: Some(entered),
        finished,
        frames: Arc::default(),
        fail: false,
    };
    let (writer, writes) = Writer::channel();
    let shutdown = CancellationToken::new();
    let worker = spawn(output, writes, shutdown.clone());
    let mut request = writer.clone();
    let (done, result) = mpsc::channel();
    let request = thread::spawn(move || {
        let _ = done.send(request.write_all(b"pending"));
    });
    let entered = started.recv_timeout(Duration::from_secs(1));
    shutdown.cancel();
    let completed = result.recv_timeout(Duration::from_secs(1));
    let _ = release.try_send(());
    writer.close();
    request.join().unwrap();
    worker.join().unwrap();
    stopped.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(entered.is_ok());
    assert!(
        completed
            .expect("shutdown must not wait for a writable sink")
            .is_err()
    );
}
