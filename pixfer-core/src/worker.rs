//! The transfer worker: a state machine that drives the stop-and-wait ARQ
//! loop, reassembles chunks, and decides auto-zip. This is the "orchestration"
//! layer that a real product would run on a background thread.

use std::sync::Arc;

use crate::compress::{is_zip, unzip_first, zip_bytes};
use crate::config::Config;
use crate::errors::PixferError;
use crate::protocol::{Frame, FrameKind};
use crate::rs::RS_K;
use crate::session::TransferSession;

/// Direction of the transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Tx,
    Rx,
}

/// A worker event, consumed by the UI / metrics layer.
#[derive(Debug, Clone)]
pub enum WorkerEvent {
    Started {
        direction: Direction,
        total_chunks: u32,
    },
    ChunkSent {
        idx: u32,
    },
    ChunkAcked {
        idx: u32,
    },
    ChunkReceived {
        idx: u32,
    },
    Retry {
        idx: u32,
        attempt: u32,
    },
    Completed {
        bytes: u64,
    },
    Failed(PixferError),
}

/// The ARQ transfer worker.
pub struct TransferWorker {
    config: Config,
    direction: Direction,
    session_id: u16,
    data: Arc<Vec<u8>>,
    session: Option<TransferSession>,
    total_chunks: u32,
    reassembled: Vec<u8>,
    event_sink: Box<dyn Fn(WorkerEvent) + Send>,
}

impl TransferWorker {
    pub fn tx(
        config: &Config,
        session_id: u16,
        data: Arc<Vec<u8>>,
        on_event: Box<dyn Fn(WorkerEvent) + Send>,
    ) -> Self {
        // Auto-zip when the estimated time exceeds the threshold.
        let data = if config.needs_zip(data.len() as u64) {
            Arc::new(zip_bytes(data.as_slice(), "pixfer"))
        } else {
            data
        };
        let total_chunks = (data.len() + RS_K - 1) / RS_K;
        Self {
            config: config.clone(),
            direction: Direction::Tx,
            session_id,
            data,
            session: None,
            total_chunks: total_chunks as u32,
            reassembled: Vec::new(),
            event_sink: on_event,
        }
    }

    pub fn rx(config: &Config, session_id: u16, on_event: Box<dyn Fn(WorkerEvent) + Send>) -> Self {
        Self {
            config: config.clone(),
            direction: Direction::Rx,
            session_id,
            data: Arc::new(Vec::new()),
            session: None,
            total_chunks: 0,
            reassembled: Vec::new(),
            event_sink: on_event,
        }
    }

    /// Feed a decoded control/chunk frame into the worker.
    pub fn on_frame(&mut self, frame: &Frame) {
        if self.session.is_none() {
            self.session = Some(match self.direction {
                Direction::Tx => TransferSession::tx(self.session_id, self.total_chunks),
                Direction::Rx => TransferSession::rx(self.session_id, frame.total_chunks),
            });
            if self.direction == Direction::Rx {
                self.total_chunks = frame.total_chunks;
            }
            (self.event_sink)(WorkerEvent::Started {
                direction: self.direction,
                total_chunks: self.total_chunks,
            });
        }
        let session = self.session.as_mut().unwrap();
        match frame.kind {
            FrameKind::Pong => {
                session.on_pong();
            }
            FrameKind::Ack => {
                session.on_ack(frame.chunk_idx);
                (self.event_sink)(WorkerEvent::ChunkAcked {
                    idx: frame.chunk_idx,
                });
                if session.next_frame().is_none() {
                    (self.event_sink)(WorkerEvent::Completed {
                        bytes: self.data.len() as u64,
                    });
                }
            }
            FrameKind::Nak => {
                session.on_nak(frame.chunk_idx);
                (self.event_sink)(WorkerEvent::Retry {
                    idx: frame.chunk_idx,
                    attempt: 1,
                });
            }
            FrameKind::Chunk => {
                if self.direction == Direction::Rx {
                    self.reassembled.extend_from_slice(&frame.payload);
                    session.on_chunk_ok(frame.chunk_idx);
                    (self.event_sink)(WorkerEvent::ChunkReceived {
                        idx: frame.chunk_idx,
                    });
                }
            }
            FrameKind::End => {
                if self.direction == Direction::Rx && session.all_received() {
                    let out = self.maybe_unzip();
                    (self.event_sink)(WorkerEvent::Completed {
                        bytes: out.len() as u64,
                    });
                }
            }
            FrameKind::Ping => {
                if self.direction == Direction::Rx {
                    session.on_ping();
                }
            }
        }
    }

    /// The worker's config (throughput, zip threshold, profile overrides).
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The next frame to emit on our screen (or None when idle/done).
    pub fn next_frame(&mut self) -> Option<Frame> {
        self.session.as_mut().and_then(|s| s.next_frame())
    }

    /// After receiving a bad chunk on RX, mark it failed so the TX retransmits.
    pub fn on_chunk_err(&mut self, idx: u32) {
        if let Some(s) = self.session.as_mut() {
            s.on_chunk_err(idx);
        }
    }

    /// If the reassembled RX buffer is a zip archive, decompress it.
    fn maybe_unzip(&self) -> Vec<u8> {
        if is_zip(&self.reassembled) {
            if let Ok((_, bytes)) = unzip_first(&self.reassembled) {
                return bytes;
            }
        }
        self.reassembled.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::default()
    }

    #[test]
    fn tx_auto_zips_large_files() {
        let big = Arc::new(vec![0xAB; 70_000_000]); // 70 MB > 60 MB threshold
        let events = std::cell::RefCell::new(Vec::new());
        let w = TransferWorker::tx(
            &cfg(),
            7,
            big,
            Box::new(move |e| events.borrow_mut().push(e)),
        );
        // After zip the data is smaller; total_chunks reflects the zipped size.
        assert!(w.total_chunks < ((70_000_000 + RS_K - 1) / RS_K) as u32);
    }
}
