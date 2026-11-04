//! IoQueue: priority order, cancellation, panics; and the 5 s timeout seen
//! from HTTP.
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; allow-unwrap-in-tests only covers #[test] fns"
)]

#[path = "api_support.rs"]
mod support;

use photosee_core::io::{FsBackend, ReadAt};
use photosee_core::testfs::MemFs;
use photosee_core::{Entry, ErrorCode, Result};
use photosee_server::queue::{IO_TIMEOUT, IoQueue, Priority};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;

type Log = Arc<Mutex<Vec<&'static str>>>;

/// A job that holds its slot until the returned sender fires.
fn blocker(q: &Arc<IoQueue>) -> (mpsc::Sender<()>, JoinHandle<()>) {
    let (release, hold) = mpsc::channel::<()>();
    let q = q.clone();
    let task = tokio::spawn(async move {
        let _ = q
            .run(Priority::Current, move || {
                let _ = hold.recv();
            })
            .await;
    });
    (release, task)
}

async fn until(what: &str, mut ok: impl FnMut() -> bool) {
    for _ in 0..2000 {
        if ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    panic!("timed out waiting for {what}");
}

fn enqueue(q: &Arc<IoQueue>, prio: Priority, name: &'static str, log: &Log) -> JoinHandle<()> {
    let (q, log) = (q.clone(), log.clone());
    tokio::spawn(async move {
        q.run(prio, move || log.lock().unwrap().push(name))
            .await
            .unwrap();
    })
}

#[tokio::test]
async fn waiting_jobs_start_by_priority_then_arrival() {
    let q = Arc::new(IoQueue::new(1));
    let (release, held) = blocker(&q);
    until("blocker running", || q.running() == 1).await;
    let log: Log = Log::default();
    let mut tasks = Vec::new();
    // Arrival order: thumb, thumb, prefetch, current, prefetch.
    let order = [
        (Priority::Thumb, "thumb1"),
        (Priority::Thumb, "thumb2"),
        (Priority::Prefetch, "prefetch1"),
        (Priority::Current, "current"),
        (Priority::Prefetch, "prefetch2"),
    ];
    for (i, (prio, name)) in order.into_iter().enumerate() {
        tasks.push(enqueue(&q, prio, name, &log));
        until("job queued", || q.waiting() == i + 1).await;
    }
    release.send(()).unwrap();
    held.await.unwrap();
    for t in tasks {
        t.await.unwrap();
    }
    assert_eq!(
        *log.lock().unwrap(),
        ["current", "prefetch1", "prefetch2", "thumb1", "thumb2"]
    );
    assert_eq!((q.running(), q.waiting()), (0, 0));
}

#[tokio::test]
async fn limit_caps_concurrency() {
    let q = Arc::new(IoQueue::new(3));
    let (now, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let mut tasks = Vec::new();
    for _ in 0..12 {
        let (q, now, peak) = (q.clone(), now.clone(), peak.clone());
        tasks.push(tokio::spawn(async move {
            q.run(Priority::Current, move || {
                let n = now.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(n, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(10));
                now.fetch_sub(1, Ordering::SeqCst);
            })
            .await
            .unwrap();
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }
    assert_eq!(peak.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn a_dropped_waiter_never_runs_and_leaves_the_line() {
    let q = Arc::new(IoQueue::new(1));
    let (release, held) = blocker(&q);
    until("blocker running", || q.running() == 1).await;
    let ran = Arc::new(AtomicBool::new(false));
    let flag = ran.clone();
    let q2 = q.clone();
    let doomed = tokio::spawn(async move {
        let _ = q2
            .run(Priority::Current, move || {
                flag.store(true, Ordering::SeqCst)
            })
            .await;
    });
    until("waiter queued", || q.waiting() == 1).await;
    doomed.abort();
    let _ = doomed.await;
    assert_eq!(q.waiting(), 0, "the cancelled caller must leave the line");
    release.send(()).unwrap();
    held.await.unwrap();
    // The slot goes to the next real caller, and the doomed job stays unrun.
    assert_eq!(q.run(Priority::Thumb, || 7).await.unwrap(), 7);
    assert!(!ran.load(Ordering::SeqCst));
    assert_eq!((q.running(), q.waiting()), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelling_at_random_moments_never_leaks_a_slot() {
    let q = Arc::new(IoQueue::new(2));
    let mut tasks = Vec::new();
    for i in 0..300u64 {
        let q = q.clone();
        let prio = [Priority::Current, Priority::Prefetch, Priority::Thumb][(i % 3) as usize];
        tasks.push(tokio::spawn(async move {
            let _ = q
                .run(prio, || std::thread::sleep(Duration::from_micros(300)))
                .await;
        }));
    }
    for (i, t) in tasks.iter().enumerate() {
        if i % 3 == 0 {
            tokio::time::sleep(Duration::from_micros(200)).await;
            t.abort();
        }
    }
    for t in tasks {
        let _ = t.await;
    }
    until("queue drained", || q.running() == 0 && q.waiting() == 0).await;
    assert_eq!(q.run(Priority::Current, || 1).await.unwrap(), 1);
}

#[tokio::test]
async fn a_panicking_job_is_an_io_error_and_frees_its_slot() {
    let q = IoQueue::new(1);
    let err = q
        .run(Priority::Current, || -> u8 {
            panic!("boom (expected by the test)")
        })
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);
    assert!(err.message.contains("panicked"), "{}", err.message);
    assert_eq!(q.run(Priority::Current, || 5).await.unwrap(), 5);
    assert_eq!(q.running(), 0);
}

#[tokio::test]
async fn waiting_counts_toward_the_timeout_and_the_waiter_leaves() {
    let q = Arc::new(IoQueue::new(1).with_timeout(Duration::from_millis(60)));
    let (release, held) = blocker(&q);
    let q2 = q.clone();
    // The blocker itself times out, but keeps its slot until released.
    held.await.unwrap();
    assert_eq!(q.running(), 1);
    let err = q2.run(Priority::Thumb, || ()).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Timeout);
    assert_eq!(q.waiting(), 0);
    release.send(()).unwrap();
    until("slot freed", || q.running() == 0).await;
}

#[test]
fn the_default_timeout_is_five_seconds() {
    assert_eq!(IO_TIMEOUT, Duration::from_secs(5));
    assert_eq!(IoQueue::new(4).timeout, IO_TIMEOUT);
}

/// `MemFs` whose stat and read_dir sleep, standing in for a NAS that stopped answering.
struct SlowFs {
    inner: MemFs,
    delay: Duration,
}

impl FsBackend for SlowFs {
    fn read_dir(&self, dir: &Path) -> Result<Vec<Entry>> {
        std::thread::sleep(self.delay);
        self.inner.read_dir(dir)
    }
    fn open(&self, path: &Path) -> Result<Box<dyn ReadAt>> {
        self.inner.open(path)
    }
    fn stat(&self, path: &Path) -> Result<Entry> {
        std::thread::sleep(self.delay);
        self.inner.stat(path)
    }
}

#[tokio::test]
async fn a_stalled_filesystem_answers_504_timeout() {
    use support::*;
    let slow = SlowFs {
        inner: golden_tree(),
        delay: Duration::from_millis(400),
    };
    let queue = IoQueue::new(4).with_timeout(Duration::from_millis(100));
    let env = env_with(Arc::new(slow), queue);
    for uri in [
        "/api/v1/list?p=/photos",
        "/api/v1/meta?p=/photos/a2.jpg",
        "/api/v1/img?p=/photos/a2.jpg&w=1&h=1",
        "/api/v1/thumb?p=/photos/a2.jpg",
        "/api/v1/nav?p=/photos&root=/photos&d=next",
    ] {
        let r = get(&env.app, uri).await;
        assert_eq!(r.status.as_u16(), 504, "{uri}");
        assert_eq!(r.json()["code"], serde_json::json!("timeout"), "{uri}");
    }
}
