//! End-to-end protocol tests: render -> (simulate channel) -> decode,
//! FEC error injection, CRC failure modes, auto-zip decisions, and the
//! full ARQ session flow.

use pixfer_core::protocol::{Frame, FrameKind, Protocol, ProtocolError};
use pixfer_core::rs::RS_K;
use pixfer_core::session::TransferSession;
use pixfer_core::transfer;

fn chunk_payload(seed: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(RS_K);
    let mut x = seed;
    for _ in 0..RS_K {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        v.push((x >> 16) as u8);
    }
    v
}

#[test]
fn frame_roundtrip_chunk() {
    let payload = chunk_payload(1);
    let frame = Frame::new(FrameKind::Chunk, 7, 3, 9, payload.clone());
    let blocks = Protocol::render(&frame);
    let dec = Protocol::decode(&blocks).expect("decode");
    assert_eq!(dec.kind, FrameKind::Chunk);
    assert_eq!(dec.session_id, 7);
    assert_eq!(dec.chunk_idx, 3);
    assert_eq!(dec.total_chunks, 9);
    assert_eq!(dec.payload, payload);
}

#[test]
fn frame_roundtrip_control() {
    let frame = Frame::new(FrameKind::Ping, 42, 0, 9, Vec::new());
    let blocks = Protocol::render(&frame);
    let dec = Protocol::decode(&blocks).expect("decode");
    assert_eq!(dec.kind, FrameKind::Ping);
    assert!(dec.payload.is_empty());
}

#[test]
fn fec_corrects_parity_block_corruption() {
    let payload = chunk_payload(5);
    let frame = Frame::new(FrameKind::Chunk, 9, 4, 6, payload.clone());
    let mut blocks = Protocol::render(&frame);
    blocks[111] ^= 0b111; // flip the first 3 parity bits (1 symbol error)
    let dec = Protocol::decode(&blocks).expect("FEC correction");
    assert_eq!(dec.payload, payload);
}

#[test]
fn crc_catches_header_corruption() {
    let frame = Frame::new(FrameKind::Chunk, 1, 2, 5, chunk_payload(2));
    let mut blocks = Protocol::render(&frame);
    blocks[99] ^= 0b101; // corrupt header/CRC region
    assert_eq!(Protocol::decode(&blocks), Err(ProtocolError::BadCrc));
}

#[test]
fn needs_zip_threshold() {
    assert!(!transfer::needs_zip(60_000_000)); // exactly 30 s is not > 30
    assert!(transfer::needs_zip(61_000_000));
    assert!(!transfer::needs_zip(1_000_000));
}

#[test]
fn arq_session_flow() {
    // TX side: PING -> PONG -> 4 chunks with ACKs -> END
    let mut tx = TransferSession::tx(7, 4);
    assert_eq!(tx.next_frame().unwrap().kind, FrameKind::Ping);
    tx.on_pong();
    for i in 0..4u32 {
        let f = tx.next_frame().unwrap();
        assert_eq!(f.kind, FrameKind::Chunk);
        assert_eq!(f.chunk_idx, i);
        tx.on_ack(i);
    }
    assert_eq!(tx.next_frame().unwrap().kind, FrameKind::End);
    assert!(tx.next_frame().is_none());

    // RX side: PONG, then ACK on verified chunk, NAK on failed chunk
    let mut rx = TransferSession::rx(7, 4);
    rx.on_ping();
    assert_eq!(rx.next_frame().unwrap().kind, FrameKind::Pong);
    rx.on_chunk_ok(1);
    assert_eq!(rx.next_frame().unwrap().kind, FrameKind::Ack);
    rx.on_chunk_err(2);
    assert_eq!(rx.next_frame().unwrap().kind, FrameKind::Nak);

    rx.mark_received(0);
    assert!(!rx.all_received());
    for i in 1..4u32 {
        rx.mark_received(i);
    }
    assert!(rx.all_received());
}
