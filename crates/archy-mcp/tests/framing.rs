use archy_mcp::{
    bounds::DepthGuard,
    framing::{FrameWriter, JsonLineFilter},
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn cancelled_read_keeps_the_partial_frame() {
    let (mut peer, input) = tokio::io::duplex(1024);
    let (output, _reader) = tokio::io::duplex(1024);
    let mut filter = JsonLineFilter::new(DepthGuard::new(input), FrameWriter::new(output));
    peer.write_all(b"{\"id\":").await.unwrap();
    let mut bytes = [0_u8; 64];
    assert!(
        tokio::time::timeout(Duration::from_millis(20), filter.read(&mut bytes))
            .await
            .is_err()
    );
    peer.write_all(b"1}\n").await.unwrap();
    let n = tokio::time::timeout(Duration::from_secs(10), filter.read(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes[..n], b"{\"id\":1}\n");
}

#[tokio::test]
async fn shared_writer_keeps_frames_whole_under_backpressure() {
    let (output, mut reader) = tokio::io::duplex(4);
    let mut first = FrameWriter::new(output);
    let mut second = first.clone();
    let a = tokio::spawn(async move {
        first.write_all(b"aaaaaaaaaaaaaaaa\n").await.unwrap();
        first.flush().await.unwrap();
    });
    let b = tokio::spawn(async move {
        second.write_all(b"bbbbbbbbbbbbbbbb\n").await.unwrap();
        second.flush().await.unwrap();
    });
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), reader.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    a.await.unwrap();
    b.await.unwrap();
    assert!(
        bytes == b"aaaaaaaaaaaaaaaa\nbbbbbbbbbbbbbbbb\n"
            || bytes == b"bbbbbbbbbbbbbbbb\naaaaaaaaaaaaaaaa\n"
    );
}

#[tokio::test]
async fn cancelled_parse_error_finishes_before_another_writer() {
    let (mut peer, input) = tokio::io::duplex(1024);
    let (output, mut reader) = tokio::io::duplex(8);
    let mut sdk_writer = FrameWriter::new(output);
    let mut filter = JsonLineFilter::new(DepthGuard::new(input), sdk_writer.clone());
    peer.write_all(b"{\n{\"ok\":1}\n").await.unwrap();
    drop(peer);
    let mut scratch = [0_u8; 64];
    assert!(
        tokio::time::timeout(Duration::from_millis(20), filter.read(&mut scratch))
            .await
            .is_err()
    );
    let resumed = tokio::spawn(async move {
        let mut bytes = [0_u8; 64];
        let n = filter.read(&mut bytes).await.unwrap();
        assert_eq!(&bytes[..n], b"{\"ok\":1}\n");
    });
    let sdk = tokio::spawn(async move {
        sdk_writer.write_all(b"{\"sdk\":true}\n").await.unwrap();
        sdk_writer.flush().await.unwrap();
    });
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), reader.read_to_end(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    resumed.await.unwrap();
    sdk.await.unwrap();
    assert_eq!(
        bytes,
        b"{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-32700,\"message\":\"Parse error\"},\"id\":null}\n{\"sdk\":true}\n"
    );
}
