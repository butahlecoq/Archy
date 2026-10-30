use std::io;
use std::os::fd::AsFd;
use std::os::fd::OwnedFd;
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
fn advertised_oversize_and_malformed_frames_stop_at_primary_header() {
    for bytes in [header(70 * 1024, 0), header(0, u32::MAX), {
        let mut invalid = header(0, 0);
        invalid[0] = b'X';
        invalid
    }] {
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
        expected_remainder: 0,
        reads: 0,
    };
    assert!(zbus::block_on(excess.receive_message(1, &mut vec![], &mut vec![], 65_536)).is_err());
    assert_eq!(excess.reads, 1);
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
