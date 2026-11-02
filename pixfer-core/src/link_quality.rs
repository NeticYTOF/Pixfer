//! Link quality: a composite index blending goodput, loss rate, and
//! latency. (Hilariously over-engineered: a single "quality score" for a
//! two-phone screen link.)

use crate::reliability::ReliabilitySnapshot;

/// A composite link-quality score in [0, 1].
#[derive(Debug, Clone)]
pub struct LinkQuality {
    /// Weighted blend of goodput (0.5), retry penalty (0.3), and NAK penalty (0.2).
    pub score: f64,
    pub goodput: f64,
    pub retry_rate: f64,
    pub nak_count: u64,
}

/// Compute a link-quality score from a reliability snapshot.
pub fn quality_score(snap: &ReliabilitySnapshot) -> LinkQuality {
    let goodput = snap.goodput_fraction();
    let retry_rate = snap.retry_fraction();
    let nak_penalty = (snap.naks as f64) * 0.2;
    let score = (goodput * 0.5 + (1.0 - retry_rate) * 0.3 + 0.2 - nak_penalty).clamp(0.0, 1.0);
    LinkQuality {
        score,
        goodput,
        retry_rate,
        nak_count: snap.naks,
    }
}

/// A monitor that tracks quality over time with a bounded history.
#[derive(Debug, Default)]
pub struct QualityMonitor {
    history: std::collections::VecDeque<f64>,
    cap: usize,
}

impl QualityMonitor {
    pub fn new(cap: usize) -> Self {
        Self {
            history: std::collections::VecDeque::new(),
            cap,
        }
    }

    pub fn record(&mut self, score: f64) {
        self.history.push_back(score.clamp(0.0, 1.0));
        if self.history.len() > self.cap {
            self.history.pop_front();
        }
    }

    /// The running mean of the quality history.
    pub fn mean(&self) -> f64 {
        if self.history.is_empty() {
            return 0.0;
        }
        self.history.iter().sum::<f64>() / (self.history.len() as f64)
    }

    pub fn latest(&self) -> Option<f64> {
        self.history.back().copied()
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reliability::ReliabilitySnapshot;

    #[test]
    fn perfect_link_scores_high() {
        let s = ReliabilitySnapshot {
            total_chunks: 10,
            delivered: 10,
            retries: 0,
            naks: 0,
        };
        let q = quality_score(&s);
        assert!((q.score - 1.0).abs() < 0.01);
        assert!((q.goodput - 1.0).abs() < 1e-9);
    }

    #[test]
    fn degraded_link_scores_lower() {
        let s = ReliabilitySnapshot {
            total_chunks: 10,
            delivered: 6,
            retries: 6,
            naks: 10,
        };
        let q = quality_score(&s);
        assert!(q.score < 0.5);
        assert!(q.nak_count == 10);
    }

    #[test]
    fn monitor_tracks_mean() {
        let mut m = QualityMonitor::new(4);
        m.record(1.0);
        m.record(0.5);
        m.record(0.6);
        m.record(0.7);
        m.record(0.8);
        // Cap 4 -> history holds the last 4: 0.5, 0.6, 0.7, 0.8
        let mean = m.mean();
        assert!((mean - 0.65).abs() < 1e-9);
        assert_eq!(m.len(), 4);
        assert!((m.latest().unwrap() - 0.8).abs() < 1e-9);
    }

    #[test]
    fn monitor_empty() {
        let m = QualityMonitor::new(4);
        assert_eq!(m.mean(), 0.0);
        assert!(m.latest().is_none());
    }
}
