//! Bounded syntax screening before rmcp, with one serialized stdout writer.
use crate::bounds::MAX_FRAME;
use std::{
    future::Future,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, ready},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::{Mutex, OwnedMutexGuard},
};

const PARSE_ERROR: &[u8] =
    b"{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-32700,\"message\":\"Parse error\"},\"id\":null}\n";
const READ_CHUNK: usize = 8192;
const POLL_STEPS: usize = 32;
type LockFuture<W> = Pin<Box<dyn Future<Output = OwnedMutexGuard<W>> + Send>>;

/// A clone holds the shared writer from its first write through its flush.
pub struct FrameWriter<W> {
    output: Arc<Mutex<W>>,
    waiting: Option<LockFuture<W>>,
    guard: Option<OwnedMutexGuard<W>>,
    closed: bool,
}
impl<W> FrameWriter<W> {
    pub fn new(output: W) -> Self {
        Self {
            output: Arc::new(Mutex::new(output)),
            waiting: None,
            guard: None,
            closed: false,
        }
    }
}
impl<W> Clone for FrameWriter<W> {
    fn clone(&self) -> Self {
        Self {
            output: self.output.clone(),
            waiting: None,
            guard: None,
            closed: false,
        }
    }
}
impl<W: AsyncWrite + Unpin + Send + 'static> FrameWriter<W> {
    fn poll_lock(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if self.guard.is_none() {
            let waiting = self
                .waiting
                .get_or_insert_with(|| Box::pin(self.output.clone().lock_owned()));
            let guard = ready!(waiting.as_mut().poll(cx));
            self.guard = Some(guard);
            self.waiting = None;
        }
        Poll::Ready(())
    }
}
impl<W: AsyncWrite + Unpin + Send + 'static> AsyncWrite for FrameWriter<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.closed {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "frame writer closed",
            )));
        }
        ready!(this.poll_lock(cx));
        let result = Pin::new(&mut **this.guard.as_mut().expect("writer lock")).poll_write(cx, buf);
        if matches!(&result, Poll::Ready(Err(_)) | Poll::Ready(Ok(0))) {
            this.guard = None;
        }
        result
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let Some(guard) = this.guard.as_mut() else {
            return Poll::Ready(Ok(()));
        };
        match Pin::new(&mut **guard).poll_flush(cx) {
            Poll::Ready(result) => {
                this.guard = None;
                Poll::Ready(result)
            }
            Poll::Pending => Poll::Pending,
        }
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.as_mut().poll_flush(cx))?;
        self.waiting = None;
        self.closed = true;
        Poll::Ready(Ok(()))
    }
}

#[derive(Clone, Copy)]
enum InputState {
    Reading,
    Discarding,
    Delivering(usize),
    Reporting(usize),
    Flushing,
}

/// Pass valid JSON frames to rmcp and report unparseable frames without inspecting IDs.
pub struct JsonLineFilter<R, W> {
    input: R,
    errors: FrameWriter<W>,
    chunk: [u8; READ_CHUNK],
    start: usize,
    end: usize,
    line: Vec<u8>,
    state: InputState,
}
impl<R, W> JsonLineFilter<R, W> {
    pub fn new(input: R, errors: FrameWriter<W>) -> Self {
        Self {
            input,
            errors,
            chunk: [0; READ_CHUNK],
            start: 0,
            end: 0,
            line: Vec::new(),
            state: InputState::Reading,
        }
    }
}
impl<R, W> AsyncRead for JsonLineFilter<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if out.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        for _ in 0..POLL_STEPS {
            match this.state {
                InputState::Delivering(offset) => {
                    let count = (this.line.len() - offset).min(out.remaining());
                    out.put_slice(&this.line[offset..offset + count]);
                    if offset + count == this.line.len() {
                        this.line.clear();
                        this.state = InputState::Reading;
                    } else {
                        this.state = InputState::Delivering(offset + count);
                    }
                    return Poll::Ready(Ok(()));
                }
                InputState::Reporting(offset) => {
                    let n =
                        ready!(Pin::new(&mut this.errors).poll_write(cx, &PARSE_ERROR[offset..]))?;
                    if n == 0 {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "parse error response stopped",
                        )));
                    }
                    this.state = if offset + n == PARSE_ERROR.len() {
                        InputState::Flushing
                    } else {
                        InputState::Reporting(offset + n)
                    };
                    continue;
                }
                InputState::Flushing => {
                    ready!(Pin::new(&mut this.errors).poll_flush(cx))?;
                    this.state = InputState::Reading;
                    continue;
                }
                InputState::Reading | InputState::Discarding => {}
            }
            if this.start == this.end {
                let mut buf = ReadBuf::new(&mut this.chunk);
                ready!(Pin::new(&mut this.input).poll_read(cx, &mut buf))?;
                this.start = 0;
                this.end = buf.filled().len();
                if this.end == 0 {
                    this.line.clear();
                    return Poll::Ready(Ok(()));
                }
            }
            let remaining = &this.chunk[this.start..this.end];
            let newline = remaining.iter().position(|byte| *byte == b'\n');
            let take = newline.map_or(remaining.len(), |index| index + 1);
            if matches!(this.state, InputState::Discarding) {
                this.start += take;
                if newline.is_some() {
                    this.state = InputState::Reading;
                }
                continue;
            }
            if this.line.len() + take > MAX_FRAME {
                this.line.clear();
                this.start += take;
                this.state = if newline.is_some() {
                    InputState::Reading
                } else {
                    InputState::Discarding
                };
                continue;
            }
            this.line
                .extend_from_slice(&this.chunk[this.start..this.start + take]);
            this.start += take;
            if newline.is_some() {
                let bytes = this
                    .line
                    .strip_prefix(b"\xef\xbb\xbf")
                    .unwrap_or(&this.line);
                let parsed = serde_json::from_slice::<serde_json::Value>(bytes);
                let malformed = parsed.as_ref().err().is_some_and(|err| {
                    matches!(
                        err.classify(),
                        serde_json::error::Category::Syntax | serde_json::error::Category::Eof
                    )
                });
                if malformed {
                    this.line.clear();
                    this.state = InputState::Reporting(0);
                } else {
                    this.state = InputState::Delivering(0);
                }
            }
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
