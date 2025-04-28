//! Module to asynchronously provide a file locking mechanism (wrapping
//! `fd-lock`).
//! # Note from fd-lock
//! “advisory locks” are locks which programs must opt-in to adhere to. This
//! means that they can be used to coordinate file access, but not prevent
//! access. Use this to coordinate file access between multiple instances of the
//! same program. But do not use this to prevent actors from accessing or
//! modifying files.

use fd_lock::RwLock;
use std::fs::File;
use std::io;
use std::path::Path;

/// Acquire an exclusive write lock asynchronously
/// This can be used to communicate with another process that a lock is applied.
pub struct AsyncFileRwLock(RwLock<File>);
pub struct AsyncFileRwLockWriteGuard<'lock>(fd_lock::RwLockWriteGuard<'lock, File>);

impl AsyncFileRwLock {
    pub async fn new<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .await?
            .into_std()
            .await;
        let handle = RwLock::new(file);
        Ok(AsyncFileRwLock(handle))
    }
    /// Acquire an exclusive write lock asynchronously
    /// This doesn't actually do anything, but can be used to communicate with
    /// another process that a lock is applied. NOTE: This spawns a
    /// dedicated thread.
    pub async fn write_lock(&mut self) -> io::Result<AsyncFileRwLockWriteGuard<'_>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::scope(move |s| {
            s.spawn(move || {
                tx.send(self.0.write()).unwrap();
            });
        });
        let guard = rx.await.unwrap()?;
        Ok(AsyncFileRwLockWriteGuard(guard))
    }
}

#[cfg(test)]
mod tests {
    use crate::app::async_file_lock::AsyncFileRwLock;
    use std::time::Duration;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_async_rw_lock() {
        let (tx, rx) = std::sync::mpsc::channel();
        let dir = tempdir().unwrap();
        let mut lock = AsyncFileRwLock::new(dir.path().join("tmp.lock"))
            .await
            .unwrap();
        let guard = lock.write_lock().await.unwrap();
        let tx_2 = tx.clone();
        let thread = tokio::spawn(async move {
            let mut lock = AsyncFileRwLock::new(dir.path().join("tmp.lock"))
                .await
                .unwrap();
            let guard = lock.write_lock().await.unwrap();
            // This will not run until guard is dropped.
            tx_2.send(2).unwrap();
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
        tx.send(1).unwrap();
        drop(guard);
        tokio::time::timeout(std::time::Duration::from_secs(1), thread)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rx.iter().collect::<Vec<_>>(), vec![1, 2])
    }
}
