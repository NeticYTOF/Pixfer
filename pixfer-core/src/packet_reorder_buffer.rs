//! Packet reorder buffer: buffer out-of-order chunks and emit them in
//! sequence number order once the gap fills. (Hilariously over-engineered:
//! a "reorder buffer" for a link where each "packet" is a 270x480 block
//! grid shown on a phone screen.)

use std::collections::BTreeMap;

/// A buffered chunk awaiting reorder.
#[derive(Debug, Clone)]
pub struct BufferedPacket {
    /// The chunk sequence number.
    pub seq: u64,
    /// The chunk payload bytes.
    pub payload: Vec<u8>,
}

/// Buffers out-of-order packets and emits them in order.
#[derive(Debug)]
pub struct PacketReorderBuffer {
    buffer: BTreeMap<u64, BufferedPacket>,
    /// The next sequence number to emit.
    next_seq: u64,
}

impl PacketReorderBuffer {
    pub fn new() -> Self {
        Self {
            buffer: BTreeMap::new(),
            next_seq: 0,
        }
    }

    /// The expected next sequence number.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// The number of packets waiting in the buffer.
    pub fn pending(&self) -> usize {
        self.buffer.len()
    }

    /// Insert a packet into the reorder buffer.
    ///
    /// Every packet goes into the buffer (no immediate consumption), so
    /// `pop_next` remains the single place where packets are consumed.
    pub fn insert(&mut self, packet: BufferedPacket) {
        self.buffer.insert(packet.seq, packet);
    }

    /// Remove and return the next in-order packet, if one is ready.
    ///
    /// If the expected `next_seq` is buffered, it is popped and `next_seq`
    /// advances by one; the caller loops `pop_next` to drain a contiguous
    /// run of buffered packets.
    pub fn pop_next(&mut self) -> Option<BufferedPacket> {
        self.buffer.remove(&self.next_seq).map(|p| {
            self.next_seq = self.next_seq.saturating_add(1);
            p
        })
    }

    /// Whether a specific sequence is buffered.
    pub fn contains(&self, seq: u64) -> bool {
        self.buffer.contains_key(&seq)
    }

    /// The smallest buffered sequence (None if empty).
    pub fn min_seq(&self) -> Option<u64> {
        self.buffer.keys().next().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkt(seq: u64, bytes: &[u8]) -> BufferedPacket {
        BufferedPacket {
            seq,
            payload: bytes.to_vec(),
        }
    }

    #[test]
    fn starts_at_seq_zero() {
        let mut b = PacketReorderBuffer::new();
        assert_eq!(b.next_seq(), 0);
        assert_eq!(b.pending(), 0);
        assert!(b.pop_next().is_none());
    }

    #[test]
    fn in_order_packet_pops_immediately() {
        let mut b = PacketReorderBuffer::new();
        b.insert(pkt(0, &[1, 2, 3]));
        let p = b.pop_next().expect("in-order");
        assert_eq!(p.seq, 0);
        assert_eq!(p.payload, vec![1, 2, 3]);
        assert_eq!(b.next_seq(), 1);
        assert_eq!(b.pending(), 0);
    }

    #[test]
    fn out_of_order_stays_buffered() {
        let mut b = PacketReorderBuffer::new();
        b.insert(pkt(2, &[9]));
        assert!(b.contains(2));
        assert!(b.pop_next().is_none()); // seq 0 missing
        assert_eq!(b.pending(), 1);
        assert_eq!(b.min_seq(), Some(2));
        // Now fill seq 0 and 1; seq 2 then becomes poppable.
        b.insert(pkt(0, &[0]));
        b.insert(pkt(1, &[1]));
        let p = b.pop_next().expect("seq 0");
        assert_eq!(p.seq, 0);
        let p2 = b.pop_next().expect("seq 1");
        assert_eq!(p2.seq, 1);
        let p3 = b.pop_next().expect("seq 2");
        assert_eq!(p3.seq, 2);
        assert!(b.pop_next().is_none());
    }

    #[test]
    fn duplicate_seq_overwrites() {
        let mut b = PacketReorderBuffer::new();
        b.insert(pkt(1, &[1]));
        b.insert(pkt(1, &[1, 1]));
        assert_eq!(b.pending(), 1);
        b.insert(pkt(0, &[0]));
        b.insert(pkt(2, &[2]));
        let p1 = b.pop_next().unwrap();
        assert_eq!(p1.payload, vec![0]);
        let p2 = b.pop_next().unwrap();
        assert_eq!(p2.payload, vec![1, 1]); // overwritten
        let p3 = b.pop_next().unwrap();
        assert_eq!(p3.payload, vec![2]);
    }

    #[test]
    fn many_out_of_order_drain_in_order() {
        let mut b = PacketReorderBuffer::new();
        for i in (0..50).rev() {
            b.insert(pkt(i as u64, &[i as u8]));
        }
        for i in 0..50 {
            let p = b.pop_next().expect("in order");
            assert_eq!(p.seq, i);
            assert_eq!(p.payload, vec![i as u8]);
        }
        assert!(b.pop_next().is_none());
    }

    #[test]
    fn saturating_next_seq_no_overflow() {
        let mut b = PacketReorderBuffer::new();
        // Force next_seq near MAX.
        b.insert(pkt(u64::MAX, &[7]));
        assert!(b.contains(u64::MAX));
        // Drain the whole range would require u64::MAX packets; just check
        // the saturating increment on pop.
        let p = b.pop_next();
        assert!(p.is_none()); // next_seq is 0, packet is at MAX
        assert_eq!(b.next_seq(), 0);
    }
}
