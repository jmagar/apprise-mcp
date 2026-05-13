use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// Maximum log file size in bytes (10 MB). When exceeded the file is truncated.
pub const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

/// A simple single-file writer that truncates on size limit.
///
/// Wraps a `Mutex<std::fs::File>` so it can be cloned cheaply and shared across
/// the tracing subscriber's internal layering.
#[derive(Clone)]
pub struct RollingFileWriter {
    path: PathBuf,
    inner: Arc<Mutex<fs::File>>,
    max_bytes: u64,
}

impl RollingFileWriter {
    /// Open `path` for appending, creating it (and parent dirs) if necessary.
    /// If the file already exceeds `max_bytes`, truncate it before opening.
    pub fn open(path: PathBuf, max_bytes: u64) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Truncate if over limit
        if path.exists() {
            let meta = fs::metadata(&path)?;
            if meta.len() >= max_bytes {
                fs::write(&path, b"")?;
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| anyhow::anyhow!("failed to open log file {}: {e}", path.display()))?;

        Ok(Self {
            path,
            inner: Arc::new(Mutex::new(file)),
            max_bytes,
        })
    }
}

impl Write for RollingFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log mutex poisoned"))?;

        // Check size before writing; truncate if needed
        if let Ok(meta) = guard.metadata() {
            if meta.len() >= self.max_bytes {
                // Re-open the file truncated
                drop(guard);
                let _ = fs::write(&self.path, b"");
                let file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(&self.path)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                *self
                    .inner
                    .lock()
                    .map_err(|_| io::Error::new(io::ErrorKind::Other, "log mutex poisoned"))? =
                    file;
                return self.write(buf);
            }
        }

        guard.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log mutex poisoned"))?
            .flush()
    }
}

// tracing-subscriber requires `MakeWriter` — implement it so we can use this as
// a writer factory.
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RollingFileWriter {
    type Writer = RollingFileWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}
