use std::io::{Read, Write};
use std::sync::Arc;

pub trait ProgressSink: Send + Sync {
    fn report(&self, done: u64, total: u64);
}

pub struct CountingReader<R: Read> {
    inner: R,
    base: u64,
    done: u64,
    total: u64,
    sink: Option<Arc<dyn ProgressSink>>,
}

impl<R: Read> CountingReader<R> {
    pub fn new(inner: R, base: u64, total: u64, sink: Option<Arc<dyn ProgressSink>>) -> Self {
        Self {
            inner,
            base,
            done: 0,
            total,
            sink,
        }
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.done += n as u64;
        if let Some(sink) = &self.sink {
            sink.report(self.base + self.done, self.total);
        }
        Ok(n)
    }
}

pub struct CountingWriter<W: Write> {
    inner: W,
    base: u64,
    done: u64,
    total: u64,
    sink: Option<Arc<dyn ProgressSink>>,
}

impl<W: Write> CountingWriter<W> {
    pub fn new(inner: W, base: u64, total: u64, sink: Option<Arc<dyn ProgressSink>>) -> Self {
        Self {
            inner,
            base,
            done: 0,
            total,
            sink,
        }
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.done += n as u64;
        if let Some(sink) = &self.sink {
            sink.report(self.base + self.done, self.total);
        }
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
