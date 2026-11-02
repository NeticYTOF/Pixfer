//! Throughput estimation and the auto-zip decision:
//! if the estimated transfer time exceeds 30 s, the file is zipped first.

/// Effective multicolor throughput estimate (~2 MB/s after FEC + sync overhead).
pub const DEFAULT_THROUGHPUT: f64 = 2_000_000.0;
/// Zip if estimated transfer takes longer than this many seconds.
pub const ZIP_THRESHOLD_S: f64 = 30.0;

pub fn estimate_seconds(size: u64, throughput: f64) -> f64 {
    (size as f64) / throughput
}

pub fn needs_zip(size: u64) -> bool {
    estimate_seconds(size, DEFAULT_THROUGHPUT) > ZIP_THRESHOLD_S
}

/// (should_zip, estimated_seconds) for a file of `size` bytes.
pub fn transfer_plan(size: u64) -> (bool, f64) {
    let est = estimate_seconds(size, DEFAULT_THROUGHPUT);
    (est > ZIP_THRESHOLD_S, est)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_boundary() {
        // 60 MB / 2 MB/s = exactly 30 s -> NOT > 30, so no zip
        assert!(!needs_zip(60_000_000));
        assert!(needs_zip(61_000_000));
        assert!(!needs_zip(1_000_000));
        let (zip, est) = transfer_plan(61_000_000);
        assert!(zip);
        assert!((est - 30.5).abs() < 0.01);
    }
}
