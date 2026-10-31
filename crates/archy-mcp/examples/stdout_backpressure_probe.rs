use archy_cli::BoundProjectRead;
use archy_mcp::{
    bounds::{DepthGuard, ResponseGuard},
    framing::{FrameWriter, JsonLineFilter},
    server::ArchyServer,
};
use rmcp::{
    RoleServer, ServiceExt,
    transport::{async_rw::AsyncRwTransport, io::stdio_pipes},
};
use std::{
    fs::File,
    io::{self, Write},
    os::fd::FromRawFd,
    os::unix::fs::MetadataExt,
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::AsyncWrite;

// Test-only adapter at the real stdio_pipes writer, below FrameWriter and ResponseGuard.
struct ObservedWriter<W> {
    inner: W,
    control: File,
    phase: u8,
    wrote: bool,
}

impl<W: AsyncWrite + Unpin> AsyncWrite for ObservedWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, buf);
        match &result {
            Poll::Pending if self.wrote && (self.phase == 0 || self.phase == 2) => {
                self.control.write_all(b"P").expect("write probe event");
                self.phase += 1;
            }
            Poll::Ready(Ok(n)) if *n > 0 => {
                self.wrote = true;
                if self.phase == 1 || self.phase == 3 {
                    self.control.write_all(b"R").expect("write probe event");
                    self.phase += 1;
                }
            }
            _ => {}
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert!(args.len() == 3 && args[1] == "--project");
    let project = BoundProjectRead::open(Path::new(&args[2])).expect("open project");
    let control_fd: i32 = std::env::var("ARCHY_MCP_WRITE_PROBE_FD")
        .expect("probe fd")
        .parse()
        .expect("numeric probe fd");
    let control = unsafe { File::from_raw_fd(control_fd) };
    let control_inode = control.metadata().expect("control pipe").ino();
    assert_ne!(
        control_inode,
        std::fs::metadata("/proc/self/fd/1")
            .expect("stdout pipe")
            .ino()
    );
    assert_ne!(
        control_inode,
        std::fs::metadata("/proc/self/fd/2")
            .expect("stderr pipe")
            .ino()
    );
    let (reader, writer) = stdio_pipes();
    let output = FrameWriter::new(ObservedWriter {
        inner: writer,
        control,
        phase: 0,
        wrote: false,
    });
    let input = JsonLineFilter::new(DepthGuard::new(reader), output.clone());
    let transport =
        AsyncRwTransport::<RoleServer, _, _>::new_server(input, ResponseGuard::new(output))
            .with_max_line_length(1_048_576);
    match ArchyServer::new(project).serve(transport).await {
        Ok(service) => {
            let _ = service.waiting().await;
        }
        Err(_) => std::process::exit(1),
    }
}
