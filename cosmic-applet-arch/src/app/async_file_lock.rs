//! Module to asynchronously provide a file locking mechanism using advisory
//! locks. The concept for this has largely been borrowed from `fd-lock` crate,
//! but simplified in the following ways:
//!
//! - Only non-Solaris unix is supported.
//! - Asynchronous interface to obtain the lock (using a task)
//! - Lock simply provides a semaphore-like mechanism - no read/write access is
//!   provided to the lockfile.
//!
//! # My understanding of advisory lock behaviour
//! Advisory locks are managed by the kernel and are automatically dropped by
//! the kernel when the process holding them is killed. This means we shouldn't
//! have the risk of a process crashing and not releasing the lock.
//!
//! # Note from fd-lock
//! “advisory locks” are locks which programs must opt-in to adhere to. This
//! means that they can be used to coordinate file access, but not prevent
//! access. Use this to coordinate file access between multiple instances of the
//! same program. But do not use this to prevent actors from accessing or
//! modifying files.

use std::path::Path;

/// Acquire an exclusive write lock asynchronously
/// This can be used to communicate with another process that a lock is applied.
#[must_use = "if unused the lock will immediately unlock"]
pub struct AsyncFileLock(std::fs::File);

impl AsyncFileLock {
    /// Locks file at `path` until this is dropped.
    /// Creates the file if it does not exist.
    pub async fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .await?
            .into_std()
            .await;
        let file = tokio::task::spawn_blocking(move || {
            let err = rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive);
            err.map(|_| file)
        })
        .await
        .unwrap()?;
        Ok(Self(file))
    }
}

impl Drop for AsyncFileLock {
    fn drop(&mut self) {
        let _ = rustix::fs::flock(&self.0, rustix::fs::FlockOperation::Unlock);
    }
}

#[cfg(test)]
mod tests {
    use crate::app::async_file_lock::AsyncFileLock;
    use std::time::Duration;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_async_lock() {
        let (tx, rx) = std::sync::mpsc::channel();
        let dir = tempdir().unwrap();
        eprintln!("Attempting to acquire first lock");
        let lock = AsyncFileLock::new(dir.path().join("tmp.lock"))
            .await
            .unwrap();
        eprintln!("First lock acquired");
        let tx_2 = tx.clone();
        let handle = tokio::spawn(async move {
            eprintln!("Attempting to acquire second lock");
            let _lock = AsyncFileLock::new(dir.path().join("tmp.lock"))
                .await
                .unwrap();
            // This will not run until first lock is dropped.
            tx_2.send(2).unwrap();
            eprintln!("Second lock acquired");
        });
        eprintln!("Sleeping 1s");
        tokio::time::sleep(Duration::from_secs(1)).await;
        tx.send(1).unwrap();
        drop(lock);
        eprintln!("Dropped first lock");
        // Must drop all senders to collect rx
        drop(tx);
        handle.await.unwrap();
        assert_eq!(rx.iter().collect::<Vec<_>>(), vec![1, 2])
    }
}
