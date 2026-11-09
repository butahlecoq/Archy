//! Byte stream guards around the SDK codec; no JSON-RPC decoding here.
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub const MAX_FRAME: usize = 1_048_576;
pub struct DepthGuard<R> {
    inner: R,
    depth: usize,
    quoted: bool,
    escaped: bool,
    chunk: [u8; 8192],
    start: usize,
    end: usize,
}
impl<R> DepthGuard<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            depth: 0,
            quoted: false,
            escaped: false,
            chunk: [0; 8192],
            start: 0,
            end: 0,
        }
    }
}
impl<R: AsyncRead + Unpin> AsyncRead for DepthGuard<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        if self.start == self.end {
            let mut scratch = [0u8; 8192];
            let mut staged = ReadBuf::new(&mut scratch);
            match Pin::new(&mut self.inner).poll_read(cx, &mut staged) {
                Poll::Ready(Ok(())) => {
                    let end = staged.filled().len();
                    self.end = end;
                    self.start = 0;
                    self.chunk[..end].copy_from_slice(staged.filled());
                    if end == 0 {
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }
        let mut count = 0;
        while self.start + count < self.end && count < buf.remaining() {
            let byte = self.chunk[self.start + count];
            if byte == b'\n' {
                self.depth = 0;
                self.quoted = false;
                self.escaped = false;
                count += 1;
                break;
            } else if self.escaped {
                self.escaped = false;
            } else if self.quoted {
                match byte {
                    b'\\' => self.escaped = true,
                    b'"' => self.quoted = false,
                    _ => {}
                }
            } else {
                match byte {
                    b'"' => self.quoted = true,
                    b'{' | b'[' => {
                        self.depth += 1;
                        if self.depth > 64 {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "JSON depth exceeds 64",
                            )));
                        }
                    }
                    b'}' | b']' => self.depth = self.depth.saturating_sub(1),
                    _ => {}
                }
            }
            count += 1;
        }
        buf.put_slice(&self.chunk[self.start..self.start + count]);
        self.start += count;
        Poll::Ready(Ok(()))
    }
}

pub struct ResponseGuard<W> {
    inner: W,
    pending: Vec<u8>,
    written: usize,
    rejected: bool,
}
impl<W> ResponseGuard<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            pending: Vec::new(),
            written: 0,
            rejected: false,
        }
    }
}
impl<W: AsyncWrite + Unpin> AsyncWrite for ResponseGuard<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.rejected {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "response too large",
            )));
        }
        if self.pending.len() + buf.len() > MAX_FRAME {
            self.rejected = true;
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "response too large",
            )));
        }
        self.pending.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while self.written < self.pending.len() {
            let written = self.written;
            let chunk = self.pending[written..].to_vec();
            match Pin::new(&mut self.inner).poll_write(cx, &chunk) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "response write stopped",
                    )));
                }
                Poll::Ready(Ok(n)) => self.written += n,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        match Pin::new(&mut self.inner).poll_flush(cx) {
            Poll::Ready(Ok(())) => {
                self.pending.clear();
                self.written = 0;
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.as_mut().poll_flush(cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut self.inner).poll_shutdown(cx),
            other => other,
        }
    }
}
