use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct StreamingSession {
    inner: Arc<SessionInner>,
}

#[derive(Debug)]
struct SessionInner {
    path: PathBuf,
    state: Mutex<SessionState>,
    wake: Condvar,
}

#[derive(Debug, Clone)]
struct SessionState {
    available_bytes: u64,
    finished: bool,
    failed: Option<String>,
    cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub available_bytes: u64,
    pub finished: bool,
    pub failed: Option<String>,
    pub cancelled: bool,
}

#[derive(Debug)]
pub struct ProgressiveReader {
    file: File,
    session: StreamingSession,
    cursor: u64,
}

impl StreamingSession {
    pub fn new(path: PathBuf) -> Self {
        Self {
            inner: Arc::new(SessionInner {
                path,
                state: Mutex::new(SessionState {
                    available_bytes: 0,
                    finished: false,
                    failed: None,
                    cancelled: false,
                }),
                wake: Condvar::new(),
            }),
        }
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        let state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        SessionSnapshot {
            available_bytes: state.available_bytes,
            finished: state.finished,
            failed: state.failed.clone(),
            cancelled: state.cancelled,
        }
    }

    pub fn mark_available(&self, available_bytes: u64) {
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        if available_bytes > state.available_bytes {
            state.available_bytes = available_bytes;
            self.inner.wake.notify_all();
        }
    }

    pub fn finish(&self, total_bytes: u64) {
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        state.available_bytes = state.available_bytes.max(total_bytes);
        state.finished = true;
        self.inner.wake.notify_all();
    }

    pub fn fail(&self, message: impl Into<String>) {
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        state.failed = Some(message.into());
        self.inner.wake.notify_all();
    }

    pub fn cancel(&self) {
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        state.cancelled = true;
        self.inner.wake.notify_all();
    }

    pub fn open_reader(&self) -> io::Result<ProgressiveReader> {
        Ok(ProgressiveReader {
            file: File::open(self.path())?,
            session: self.clone(),
            cursor: 0,
        })
    }

    fn wait_for_read(&self, cursor: u64) -> io::Result<Option<u64>> {
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if state.cancelled {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "streaming session cancelled",
                ));
            }
            if cursor < state.available_bytes {
                return Ok(Some(state.available_bytes));
            }
            if let Some(message) = state.failed.clone() {
                return Err(io::Error::other(message));
            }
            if state.finished {
                return Ok(None);
            }
            state = self
                .inner
                .wake
                .wait(state)
                .unwrap_or_else(|e| e.into_inner());
        }
    }
}

impl ProgressiveReader {
    fn resolve_seek_target(&self, pos: SeekFrom) -> io::Result<u64> {
        let snapshot = self.session.snapshot();
        let current = self.cursor as i128;
        let buffered_end = snapshot.available_bytes as i128;
        let target = match pos {
            SeekFrom::Start(offset) => offset as i128,
            SeekFrom::Current(offset) => current
                .checked_add(offset as i128)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek overflow"))?,
            SeekFrom::End(offset) => {
                if !snapshot.finished {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "stream is not fully buffered yet",
                    ));
                }
                buffered_end
                    .checked_add(offset as i128)
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek overflow"))?
            }
        };

        if target < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before start of stream",
            ));
        }

        let target = target as u64;
        if target > snapshot.available_bytes {
            let kind = if snapshot.finished {
                io::ErrorKind::UnexpectedEof
            } else {
                io::ErrorKind::WouldBlock
            };
            return Err(io::Error::new(
                kind,
                "target offset is beyond currently buffered data",
            ));
        }

        Ok(target)
    }
}

impl Read for ProgressiveReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            let Some(available_bytes) = self.session.wait_for_read(self.cursor)? else {
                return Ok(0);
            };
            let readable = available_bytes.saturating_sub(self.cursor);
            if readable == 0 {
                continue;
            }

            let cap = readable.min(buf.len() as u64) as usize;
            let n = self.file.read(&mut buf[..cap])?;
            if n == 0 {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }

            self.cursor = self.cursor.saturating_add(n as u64);
            return Ok(n);
        }
    }
}

impl Seek for ProgressiveReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target = self.resolve_seek_target(pos)?;
        let next = self.file.seek(SeekFrom::Start(target))?;
        self.cursor = next;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::StreamingSession;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn snapshot_tracks_progress_and_finish() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stream.bin");
        std::fs::File::create(&path).expect("create file");

        let session = StreamingSession::new(path);
        session.mark_available(128);
        session.finish(256);

        let snapshot = session.snapshot();
        assert_eq!(snapshot.available_bytes, 256);
        assert!(snapshot.finished);
        assert!(!snapshot.cancelled);
        assert_eq!(snapshot.failed, None);
    }

    #[test]
    fn reader_blocks_until_more_bytes_are_available() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stream.bin");
        let mut writer = std::fs::File::create(&path).expect("create file");
        writer.write_all(b"abc").expect("write first chunk");
        writer.flush().expect("flush first chunk");

        let session = StreamingSession::new(path);
        session.mark_available(3);
        let mut reader = session.open_reader().expect("open reader");

        let mut first = [0u8; 3];
        reader.read_exact(&mut first).expect("read first chunk");
        assert_eq!(&first, b"abc");

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut second = [0u8; 3];
            let result = reader.read_exact(&mut second).map(|_| second);
            let _ = tx.send(result);
        });

        std::thread::sleep(Duration::from_millis(50));
        assert!(rx.try_recv().is_err(), "reader should still be waiting");

        writer.write_all(b"def").expect("write second chunk");
        writer.flush().expect("flush second chunk");
        session.mark_available(6);
        session.finish(6);

        let second = rx
            .recv_timeout(Duration::from_secs(1))
            .expect("reader result")
            .expect("read second chunk");
        assert_eq!(&second, b"def");
    }

    #[test]
    fn reader_rejects_seek_past_buffered_end_before_finish() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stream.bin");
        let mut writer = std::fs::File::create(&path).expect("create file");
        writer.write_all(b"abcd").expect("write chunk");
        writer.flush().expect("flush chunk");

        let session = StreamingSession::new(path);
        session.mark_available(4);
        let mut reader = session.open_reader().expect("open reader");
        let err = reader
            .seek(SeekFrom::Start(5))
            .expect_err("seek should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
    }

    #[test]
    fn reader_reports_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stream.bin");
        std::fs::File::create(&path).expect("create file");

        let session = StreamingSession::new(path);
        let mut reader = session.open_reader().expect("open reader");
        session.fail("boom");

        let mut buf = [0u8; 1];
        let err = reader.read(&mut buf).expect_err("read should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::Other);
    }
}
