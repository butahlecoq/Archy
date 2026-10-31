use archy_mcp::bounds::{DepthGuard, MAX_FRAME, ResponseGuard};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn depth_guard_accepts_strings_and_refuses_deep_json() {
    let (mut writer, reader) = tokio::io::duplex(1024);
    writer
        .write_all(br#"{"quoted":"[[[[","nested":[{"value":1}]}"#)
        .await
        .unwrap();
    drop(writer);
    let mut guarded = DepthGuard::new(reader);
    let mut bytes = Vec::new();
    guarded.read_to_end(&mut bytes).await.unwrap();
    assert!(bytes.ends_with(b"}"));
    let (mut writer, reader) = tokio::io::duplex(1024);
    writer.write_all(&[b'['; 65]).await.unwrap();
    drop(writer);
    let mut guarded = DepthGuard::new(reader);
    assert!(guarded.read_to_end(&mut Vec::new()).await.is_err());
}

#[tokio::test]
async fn response_guard_refuses_oversize_before_any_write() {
    let (writer, mut reader) = tokio::io::duplex(1024);
    let mut bounded = ResponseGuard::new(writer);
    assert!(bounded.write_all(&vec![b'x'; MAX_FRAME + 1]).await.is_err());
    drop(bounded);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.unwrap();
    assert!(bytes.is_empty());
}

#[tokio::test]
async fn response_guard_flushes_complete_frame() {
    let (writer, mut reader) = tokio::io::duplex(1024);
    let mut bounded = ResponseGuard::new(writer);
    bounded.write_all(b"{\"ok\":true}\n").await.unwrap();
    bounded.flush().await.unwrap();
    drop(bounded);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.unwrap();
    assert_eq!(bytes, b"{\"ok\":true}\n");
}

#[tokio::test]
async fn depth_state_resets_after_malformed_frame_newline() {
    let (mut writer, reader) = tokio::io::duplex(1024);
    writer.write_all(b"\"unterminated\n").await.unwrap();
    writer.write_all(&[b'['; 65]).await.unwrap();
    drop(writer);
    let mut guarded = DepthGuard::new(reader);
    assert!(guarded.read_to_end(&mut Vec::new()).await.is_err());
}

#[tokio::test]
async fn depth_guard_delivers_complete_line_before_later_depth_error() {
    let (mut writer, reader) = tokio::io::duplex(1024);
    writer.write_all(b"{\"ok\":true}\n").await.unwrap();
    writer.write_all(&[b'['; 65]).await.unwrap();
    drop(writer);
    let mut guarded = DepthGuard::new(reader);
    let mut first = [0_u8; 1024];
    let n = guarded.read(&mut first).await.unwrap();
    assert_eq!(&first[..n], b"{\"ok\":true}\n");
    assert!(guarded.read(&mut first).await.is_err());
}

#[tokio::test]
async fn depth_guard_detects_fragmented_65th_opening_bracket() {
    let (mut writer, reader) = tokio::io::duplex(1024);
    let mut guarded = DepthGuard::new(reader);
    writer.write_all(&[b'['; 64]).await.unwrap();
    let mut bytes = [0_u8; 128];
    let n = guarded.read(&mut bytes).await.unwrap();
    assert_eq!(&bytes[..n], &[b'['; 64]);
    writer.write_all(b"[").await.unwrap();
    assert!(guarded.read(&mut bytes).await.is_err());
}
