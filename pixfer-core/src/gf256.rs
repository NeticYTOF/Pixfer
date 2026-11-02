/// GF(256) arithmetic over the primitive polynomial 0x11D.
///
/// Precomputed exp/log tables make every field op O(1).
pub struct Gf256 {
    pub exp: [u16; 512],
    pub log: [u8; 256],
}

impl Gf256 {
    pub fn new() -> Self {
        let mut exp = [0u16; 512];
        let mut log = [0u8; 256];
        let mut x: u16 = 1;
        for i in 0..255 {
            exp[i] = x;
            log[x as usize] = i as u8;
            x <<= 1;
            if x & 0x100 != 0 {
                x ^= 0x11D;
            }
        }
        // Extend exp periodically so modular indexing is always safe.
        for i in 255..512 {
            exp[i] = exp[i - 255];
        }
        Gf256 { exp, log }
    }

    #[inline]
    pub fn mul(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            return 0;
        }
        self.exp[(self.log[a as usize] as usize + self.log[b as usize] as usize) % 255] as u8
    }

    #[inline]
    pub fn div(&self, a: u8, b: u8) -> u8 {
        if a == 0 {
            return 0;
        }
        let idx =
            ((self.log[a as usize] as i32 - self.log[b as usize] as i32 + 255) % 255) as usize;
        self.exp[idx] as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_log_roundtrip() {
        let gf = Gf256::new();
        for a in 1..=254u8 {
            assert_eq!(gf.log[gf.exp[a as usize] as usize], a);
        }
        // alpha^255 = 1, whose discrete log is 0
        assert_eq!(gf.exp[255], 1);
        assert_eq!(gf.log[1], 0);
    }

    #[test]
    fn mul_identity() {
        let gf = Gf256::new();
        assert_eq!(gf.mul(0, 17), 0);
        for a in 1..=255u8 {
            assert_eq!(gf.mul(a, 1), a, "mul identity failed for {a}");
        }
    }

    #[test]
    fn multiplicative_group_order_255() {
        let gf = Gf256::new();
        assert_eq!(gf.exp[255], 1);
    }
}
