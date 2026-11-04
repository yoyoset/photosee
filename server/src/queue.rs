//! Prioritized blocking-IO queue. Package G. At most `limit` jobs touch the
//! filesystem at once (4 by default: more saturates a NAS and slows the
//! image the user is waiting for); waiting jobs start in priority order.
//!
//! Three pitfalls shaped this file:
//! - A `spawn_blocking` job cannot be cancelled. When the HTTP request is
//!   dropped mid-job the read is still in flight, so the slot is owned by the
//!   *job* (`Permit` moves into the closure), not by the awaiting future.
//! - A waiter that is dropped must leave the queue at once. Waiters live in a
//!   `BTreeMap` keyed by `(priority, sequence)`: first key = next to run, and
//!   removal on drop is O(log n) instead of a heap scan.
//! - Granting a slot and dropping the waiter can race: the grant is already
//!   in the oneshot when the future dies. `Ticket::drop` closes the channel
//!   and gives back a grant that nobody will use.

use photosee_core::{Error, ErrorCode, Result};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::oneshot;

/// How long one request may wait for the filesystem, queue time included.
/// The UI's NAS banner says "超时 5s"; keep both in step. Queue time counts
/// on purpose: if a hung share has pinned every slot, queued requests must
/// fail too instead of waiting forever.
pub const IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// The image on screen. Always first.
    Current = 0,
    Prefetch = 1,
    Thumb = 2,
}

type Key = (Priority, u64);

#[derive(Default)]
struct State {
    running: usize,
    next_seq: u64,
    waiting: BTreeMap<Key, oneshot::Sender<()>>,
}

pub struct IoQueue {
    pub limit: usize,
    /// Per-request budget; `IO_TIMEOUT` unless a test shortens it.
    pub timeout: Duration,
    state: Arc<Mutex<State>>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    // The state is plain counters and a map; a panic elsewhere cannot leave
    // it half-updated, so a poisoned lock is safe to keep using.
    state.lock().unwrap_or_else(|p| p.into_inner())
}

/// Starts waiters while there is room. Called with the lock held.
fn dispatch(st: &mut State, limit: usize) {
    while st.running < limit {
        let Some((_, tx)) = st.waiting.pop_first() else {
            return;
        };
        // Err = the receiver is gone; the slot stays free for the next one.
        if tx.send(()).is_ok() {
            st.running += 1;
        }
    }
}

/// One execution slot. Dropping it (job done, panicked, or never started)
/// starts the next waiter.
struct Permit {
    state: Arc<Mutex<State>>,
    limit: usize,
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut st = lock(&self.state);
        st.running -= 1;
        dispatch(&mut st, self.limit);
    }
}

/// A place in line. Dropping it before the grant removes the place.
struct Ticket {
    key: Key,
    rx: Option<oneshot::Receiver<()>>,
    state: Arc<Mutex<State>>,
    limit: usize,
}

impl Ticket {
    async fn granted(&mut self) -> Result<Permit> {
        // Polled through `&mut` and kept in `self` until it completes, so a
        // future dropped mid-wait still lets `Drop` see an unclaimed grant.
        let Some(rx) = self.rx.as_mut() else {
            return Err(Error::new(ErrorCode::Io, "queue ticket reused"));
        };
        let got = rx.await;
        self.rx = None; // a completed receiver must not be polled again
        // The sender lives in the map until granted; it vanishing without a
        // grant would be a bug in this file, reported rather than hung on.
        got.map_err(|_| Error::new(ErrorCode::Io, "io queue dropped a waiter"))?;
        Ok(Permit {
            state: self.state.clone(),
            limit: self.limit,
        })
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let mut st = lock(&self.state);
        if st.waiting.remove(&self.key).is_some() {
            return; // never granted
        }
        // Granted (or being granted) but `granted()` did not finish: hand
        // the slot back.
        if let Some(mut rx) = self.rx.take() {
            rx.close();
            if rx.try_recv().is_ok() {
                st.running -= 1;
                dispatch(&mut st, self.limit);
            }
        }
    }
}

impl IoQueue {
    pub fn new(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            timeout: IO_TIMEOUT,
            state: Arc::default(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Jobs currently executing or granted, for tests and diagnostics.
    pub fn running(&self) -> usize {
        lock(&self.state).running
    }

    /// Requests waiting for a slot, for tests and diagnostics.
    pub fn waiting(&self) -> usize {
        lock(&self.state).waiting.len()
    }

    /// Waits for a slot, runs `job` on a blocking thread, and gives up with
    /// `Timeout` after `self.timeout` (waiting included). A job that panics
    /// is an `Io` error and still frees its slot. On timeout the job cannot
    /// be stopped; it keeps its slot until it returns.
    pub async fn run<T, F>(&self, prio: Priority, job: F) -> Result<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let within = tokio::time::timeout(self.timeout, self.run_unbounded(prio, job));
        within.await.unwrap_or_else(|_| {
            Err(Error::new(
                ErrorCode::Timeout,
                format!("filesystem did not answer within {:?}", self.timeout),
            ))
        })
    }

    /// `run` for jobs that already return a core `Result`.
    pub async fn run_try<T, F>(&self, prio: Priority, job: F) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        self.run(prio, job).await?
    }

    async fn run_unbounded<T, F>(&self, prio: Priority, job: F) -> Result<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let permit = self.acquire(prio).await?;
        let handle = tokio::task::spawn_blocking(move || {
            let _slot = permit; // released when the job returns or unwinds
            job()
        });
        handle.await.map_err(|e| {
            let what = if e.is_panic() {
                "panicked"
            } else {
                "was cancelled"
            };
            Error::new(ErrorCode::Io, format!("blocking job {what}"))
        })
    }

    async fn acquire(&self, prio: Priority) -> Result<Permit> {
        let mut ticket = {
            let mut st = lock(&self.state);
            if st.running < self.limit && st.waiting.is_empty() {
                st.running += 1;
                return Ok(Permit {
                    state: self.state.clone(),
                    limit: self.limit,
                });
            }
            let key = (prio, st.next_seq);
            st.next_seq += 1;
            let (tx, rx) = oneshot::channel();
            st.waiting.insert(key, tx);
            Ticket {
                key,
                rx: Some(rx),
                state: self.state.clone(),
                limit: self.limit,
            }
        };
        ticket.granted().await
    }
}
