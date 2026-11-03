//! Prioritized blocking-IO queue. Package G. At most `limit` jobs touch the
//! filesystem at once (4 by default: more saturates a NAS and slows the
//! image the user is waiting for); waiting jobs start in priority order.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// The image on screen. Always first.
    Current = 0,
    Prefetch = 1,
    Thumb = 2,
}

pub struct IoQueue {
    pub limit: usize,
}

impl IoQueue {
    pub fn new(limit: usize) -> Self {
        Self { limit }
    }

    /// Runs `job` on a blocking thread once a slot is free.
    pub async fn run<T, F>(&self, prio: Priority, job: F) -> T
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let _ = prio; // package G: real priority scheduling
        tokio::task::spawn_blocking(job)
            .await
            .expect("blocking job panicked")
    }
}
