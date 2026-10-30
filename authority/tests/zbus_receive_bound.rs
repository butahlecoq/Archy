use std::io;
use std::os::fd::OwnedFd;
use std::os::fd::{AsFd, AsRawFd};
use std::time::{Duration, Instant};
use zbus::connection::socket::ReadHalf;

#[derive(Debug)]
struct HeaderOnly {
    header: [u8; 16],
    reads: usize,
}

#[zbus::export::async_trait::async_trait]
impl ReadHalf for HeaderOnly {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
        assert_eq!(self.reads, 0, "body must not be read");
        assert_eq!(
            buf.len(),
            16,
            "body must not be allocated before header check"
        );
        buf.copy_from_slice(&self.header);
        self.reads += 1;
        Ok((16, vec![]))
    }
}

fn header(body_len: u32, fields_len: u32) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[0] = b'l';
    bytes[1] = 1;
    bytes[3] = 1;
    bytes[4..8].copy_from_slice(&body_len.to_le_bytes());
    bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&fields_len.to_le_bytes());
    bytes
}

#[test]
fn impossible_lengths_and_malformed_frames_stop_at_primary_header() {
    for bytes in [
        header(128 * 1024 * 1024, 0),
        header(0, u32::MAX),
        {
            let mut invalid = header(0, 0);
            invalid[0] = b'X';
            invalid
        },
        {
            let mut invalid = header(70 * 1024, 0);
            invalid[3] = 2;
            invalid
        },
    ] {
        let mut reader = HeaderOnly {
            header: bytes,
            reads: 0,
        };
        let result = zbus::block_on(reader.receive_message(1, &mut vec![], &mut vec![], 64 * 1024));
        assert!(result.is_err());
        assert_eq!(reader.reads, 1);
    }
}

#[derive(Debug)]
struct RemainderProbe {
    primary: [u8; 16],
    expected_remainder: usize,
    reads: usize,
}

#[zbus::export::async_trait::async_trait]
impl ReadHalf for RemainderProbe {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
        self.reads += 1;
        if self.reads == 1 {
            assert_eq!(buf.len(), 16);
            buf.copy_from_slice(&self.primary);
            Ok((16, vec![]))
        } else {
            assert_eq!(self.reads, 2);
            assert_eq!(buf.len(), self.expected_remainder);
            Ok((0, vec![]))
        }
    }
}

#[test]
fn exact_frame_limit_enters_bounded_body_read_and_one_byte_over_does_not() {
    let mut exact = RemainderProbe {
        primary: header(65_520, 0),
        expected_remainder: 65_520,
        reads: 0,
    };
    assert!(zbus::block_on(exact.receive_message(1, &mut vec![], &mut vec![], 65_536)).is_err());
    assert_eq!(exact.reads, 2);
    let mut excess = RemainderProbe {
        primary: header(65_521, 0),
        expected_remainder: 4096,
        reads: 0,
    };
    assert!(zbus::block_on(excess.receive_message(1, &mut vec![], &mut vec![], 65_536)).is_err());
    assert_eq!(excess.reads, 2);
}

#[derive(Debug)]
struct MalformedFields(usize);

#[zbus::export::async_trait::async_trait]
impl ReadHalf for MalformedFields {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
        self.0 += 1;
        match self.0 {
            1 => {
                buf.copy_from_slice(&header(0, 8));
                Ok((16, vec![]))
            }
            2 => {
                assert_eq!(buf.len(), 8);
                buf.fill(0xff);
                Ok((8, vec![]))
            }
            _ => panic!("malformed fields must stop parsing"),
        }
    }
}

#[test]
fn malformed_dynamic_fields_fail_within_small_allocated_frame() {
    let mut reader = MalformedFields(0);
    assert!(zbus::block_on(reader.receive_message(1, &mut vec![], &mut vec![], 65_536)).is_err());
    assert_eq!(reader.0, 2);
}

#[derive(Debug)]
struct FdFlood {
    file: std::fs::File,
    reads: usize,
}

#[zbus::export::async_trait::async_trait]
impl ReadHalf for FdFlood {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
        self.reads += 1;
        let count = if self.reads == 1 { 16 } else { 1 };
        let fds = (0..count)
            .map(|_| self.file.as_fd().try_clone_to_owned().unwrap())
            .collect();
        if self.reads == 1 {
            buf.copy_from_slice(&header(1, 0));
            Ok((16, fds))
        } else {
            assert_eq!(self.reads, 2);
            assert_eq!(buf.len(), 1);
            buf[0] = 0;
            Ok((1, fds))
        }
    }
}

#[test]
fn descriptors_across_multiple_receives_have_one_message_bound() {
    let mut reader = FdFlood {
        file: std::fs::File::open("/dev/null").unwrap(),
        reads: 0,
    };
    assert!(matches!(
        zbus::block_on(reader.receive_message(1, &mut vec![], &mut vec![], 65_536)),
        Err(zbus::Error::ExcessData)
    ));
    assert_eq!(reader.reads, 2);
}

#[derive(Debug)]
struct ChunkedFrames {
    bytes: Vec<u8>,
    offset: usize,
    fd: Option<OwnedFd>,
    largest_read: usize,
}

#[zbus::export::async_trait::async_trait]
impl ReadHalf for ChunkedFrames {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
        self.largest_read = self.largest_read.max(buf.len());
        let n = buf.len().min(512).min(self.bytes.len() - self.offset);
        buf[..n].copy_from_slice(&self.bytes[self.offset..self.offset + n]);
        self.offset += n;
        Ok((n, self.fd.take().into_iter().collect()))
    }
}

#[test]
fn oversized_frame_with_fd_is_discarded_before_next_message() {
    let valid = zbus::message::Message::method_call("/org/archy/Authority1", "Ping")
        .unwrap()
        .build(&())
        .unwrap();
    let mut bytes = header(70 * 1024, 0).to_vec();
    bytes.resize(16 + 70 * 1024, 0);
    bytes.extend_from_slice(valid.data().bytes());
    let fd = std::fs::File::open("/dev/null").unwrap();
    let transferred = fd.as_fd().try_clone_to_owned().unwrap();
    let raw = transferred.as_raw_fd();
    let mut reader = ChunkedFrames {
        bytes,
        offset: 0,
        fd: Some(transferred),
        largest_read: 0,
    };
    let next =
        zbus::block_on(reader.receive_message(1, &mut vec![], &mut vec![], 64 * 1024)).unwrap();
    assert_eq!(next.header().member().unwrap(), "Ping");
    assert_eq!(reader.offset, reader.bytes.len());
    assert!(reader.largest_read <= 4096);
    assert_eq!(unsafe { libc::fcntl(raw, libc::F_GETFD) }, -1);
}

#[derive(Debug)]
struct SlowHeader(usize);

#[zbus::export::async_trait::async_trait]
impl ReadHalf for SlowHeader {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
        self.0 += 1;
        if self.0 == 1 {
            buf[0] = b'l';
            Ok((1, vec![]))
        } else {
            async_io::Timer::after(Duration::from_secs(5)).await;
            Ok((0, vec![]))
        }
    }
}

#[test]
fn partial_primary_header_has_a_deadline() {
    let mut reader = SlowHeader(0);
    let started = Instant::now();
    assert!(
        zbus::block_on(reader.receive_message(1, &mut vec![], &mut vec![], 64 * 1024)).is_err()
    );
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[derive(Debug)]
struct SlowOversize(usize);

#[zbus::export::async_trait::async_trait]
impl ReadHalf for SlowOversize {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
        self.0 += 1;
        match self.0 {
            1 => {
                buf.copy_from_slice(&header(70 * 1024, 0));
                Ok((16, vec![]))
            }
            2 => {
                buf[0] = 0;
                Ok((1, vec![]))
            }
            _ => {
                async_io::Timer::after(Duration::from_secs(5)).await;
                Ok((0, vec![]))
            }
        }
    }
}

#[test]
fn partial_oversized_frame_has_a_deadline() {
    let mut reader = SlowOversize(0);
    let started = Instant::now();
    assert!(
        zbus::block_on(reader.receive_message(1, &mut vec![], &mut vec![], 64 * 1024)).is_err()
    );
    assert!(started.elapsed() < Duration::from_secs(3));
}
