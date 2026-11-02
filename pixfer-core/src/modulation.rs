//! Modulation strategies grounded in IEEE 802.15.7 (Ook, VPPM, CSK),
//! behind a `ModulationStrategy` trait. The browser demo uses the
//! multicolor block grid; this module documents the modulation alternatives
//! and their relative spectral efficiency.

/// The family of modulation schemes a VLC link can use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modulation {
    /// On-Off Keying: 1 bit per symbol. Simplest, lowest rate.
    OOK,
    /// Variable Pulse Position Modulation: log2(N) bits per symbol.
    VPPM,
    /// Coded Shift Keying: multi-level, higher bits/symbol.
    CSK,
}

impl Modulation {
    pub fn bits_per_symbol(&self) -> u32 {
        match self {
            Modulation::OOK => 1,
            // 4-level VPPM -> 2 bits/symbol
            Modulation::VPPM => 2,
            // 8-level CSK -> 3 bits/symbol (our multicolor grid is exactly this)
            Modulation::CSK => 3,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Modulation::OOK => "OOK",
            Modulation::VPPM => "VPPM",
            Modulation::CSK => "CSK",
        }
    }
}

/// Strategy interface so the link can swap modulation schemes.
pub trait ModulationStrategy: Send {
    fn modulation(&self) -> Modulation;
    /// Encode `payload` into the on-wire symbol stream length (symbols).
    fn symbol_count(&self, payload_len: usize) -> usize;
    /// Relative spectral efficiency vs OOK baseline.
    fn efficiency_vs_ook(&self) -> f64;
}

/// OOK strategy: 1 bit/symbol.
pub struct OokStrategy;
impl ModulationStrategy for OokStrategy {
    fn modulation(&self) -> Modulation {
        Modulation::OOK
    }
    fn symbol_count(&self, payload_len: usize) -> usize {
        payload_len * 8
    }
    fn efficiency_vs_ook(&self) -> f64 {
        1.0
    }
}

/// VPPM strategy: 2 bits/symbol (4-level).
pub struct VppmStrategy;
impl ModulationStrategy for VppmStrategy {
    fn modulation(&self) -> Modulation {
        Modulation::VPPM
    }
    fn symbol_count(&self, payload_len: usize) -> usize {
        (payload_len * 8 + 1) / 2
    }
    fn efficiency_vs_ook(&self) -> f64 {
        2.0
    }
}

/// CSK strategy: 3 bits/symbol — the multicolor block grid the demo uses.
pub struct CskStrategy;
impl ModulationStrategy for CskStrategy {
    fn modulation(&self) -> Modulation {
        Modulation::CSK
    }
    fn symbol_count(&self, payload_len: usize) -> usize {
        (payload_len * 8 + 2) / 3
    }
    fn efficiency_vs_ook(&self) -> f64 {
        3.0
    }
}

/// Choose the best strategy for a given throughput target.
pub fn pick_strategy(target_mbps: f64) -> Box<dyn ModulationStrategy> {
    // Higher throughput targets favor the denser CSK (3 bits/symbol).
    if target_mbps >= 2.0 {
        Box::new(CskStrategy)
    } else if target_mbps >= 1.0 {
        Box::new(VppmStrategy)
    } else {
        Box::new(OokStrategy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RS_K;

    #[test]
    fn bits_per_symbol_scales() {
        assert_eq!(Modulation::OOK.bits_per_symbol(), 1);
        assert_eq!(Modulation::VPPM.bits_per_symbol(), 2);
        assert_eq!(Modulation::CSK.bits_per_symbol(), 3);
    }

    #[test]
    fn csk_matches_block_grid() {
        // A 223-byte chunk is 1784 bits -> 595 blocks at 3 bits/block.
        let s = CskStrategy;
        assert_eq!(s.symbol_count(RS_K), (RS_K * 8 + 2) / 3);
        assert_eq!(s.symbol_count(RS_K), 595);
    }

    #[test]
    fn strategy_picking() {
        assert_eq!(pick_strategy(2.5).modulation(), Modulation::CSK);
        assert_eq!(pick_strategy(1.2).modulation(), Modulation::VPPM);
        assert_eq!(pick_strategy(0.4).modulation(), Modulation::OOK);
    }
}
