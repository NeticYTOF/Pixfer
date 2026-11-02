//! Link matrix: a 2D grid of per-session, per-metric values for dashboards.
//! (Hilariously over-engineered: a "matrix" for a link that only moves one
//! file at a time.)

/// A single metric value in the matrix.
#[derive(Debug, Clone, Copy)]
pub struct MetricCell {
    pub session_id: u32,
    pub metric: &'static str,
    pub value: f64,
}

/// A 2D matrix of link metrics keyed by (session, metric).
#[derive(Debug, Default)]
pub struct LinkMatrix {
    /// rows: session_id -> (metric -> value)
    data: std::collections::HashMap<u32, std::collections::HashMap<&'static str, f64>>,
}

impl LinkMatrix {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a cell value.
    pub fn set(&mut self, session_id: u32, metric: &'static str, value: f64) {
        self.data
            .entry(session_id)
            .or_default()
            .insert(metric, value);
    }

    /// Get a cell value (0.0 if unset).
    pub fn get(&self, session_id: u32, metric: &'static str) -> f64 {
        self.data
            .get(&session_id)
            .and_then(|m| m.get(metric))
            .copied()
            .unwrap_or(0.0)
    }

    /// All cells as a flat list.
    pub fn cells(&self) -> Vec<MetricCell> {
        let mut out = Vec::new();
        for (sid, metrics) in &self.data {
            for (name, value) in metrics {
                out.push(MetricCell {
                    session_id: *sid,
                    metric: name,
                    value: *value,
                });
            }
        }
        out
    }

    pub fn session_count(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut m = LinkMatrix::new();
        m.set(7, "mbps", 2.0);
        assert!((m.get(7, "mbps") - 2.0).abs() < 1e-9);
        assert_eq!(m.get(7, "nope"), 0.0);
        assert_eq!(m.get(8, "mbps"), 0.0);
    }

    #[test]
    fn cells_flattened() {
        let mut m = LinkMatrix::new();
        m.set(7, "mbps", 2.0);
        m.set(7, "retries", 3.0);
        m.set(8, "mbps", 4.0);
        let c = m.cells();
        assert_eq!(c.len(), 3);
        assert_eq!(m.session_count(), 2);
    }

    #[test]
    fn overwrite_cell() {
        let mut m = LinkMatrix::new();
        m.set(1, "mbps", 1.0);
        m.set(1, "mbps", 5.0);
        assert!((m.get(1, "mbps") - 5.0).abs() < 1e-9);
        assert_eq!(m.cells().len(), 1);
    }
}
