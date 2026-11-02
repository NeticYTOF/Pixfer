//! Reed-Solomon RS(255,223) over GF(256): systematic encoder + full
//! soft-decode pipeline (syndromes, Berlekamp-Massey, Chien search,
//! Forney magnitudes). Corrects up to 16 symbol errors per codeword.

use crate::gf256::Gf256;

pub const RS_N: usize = 255;
pub const RS_K: usize = 223;
pub const RS_T: usize = 16;
pub const RS_PARITY: usize = RS_N - RS_K; // 32

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RSError {
    BadLength,
    TooManyErrors,
    CheckFailed,
}

pub struct ReedSolomon {
    gf: Gf256,
    gen: Vec<u8>,
}

impl ReedSolomon {
    /// The generator polynomial g(x) (descending coefficients, g[0] = 1).
    pub fn generator(&self) -> &[u8] {
        &self.gen
    }

    pub fn new() -> Self {
        let gf = Gf256::new();
        // g(x) = prod_{i=0}^{2T-1} (x - a^i), stored in DESCENDING order
        // so gen[0] == 1 (the leading coefficient) for the division encoder.
        let mut gen: Vec<u8> = vec![1];
        for i in 0..(2 * RS_T) {
            let root = gf.exp[i] as u8;
            let mut next = vec![0u8; gen.len() + 1];
            for (j, &c) in gen.iter().enumerate() {
                if c != 0 {
                    // (x - root) * g(x): x*g shifts up, root*g stays;
                    // contributions to the same coefficient are XORed (GF addition).
                    next[j] ^= c;
                    next[j + 1] ^= gf.mul(c, root);
                }
            }
            gen = next;
        }
        Self { gf, gen }
    }

    /// Systematic encode. Codeword layout: [parity_0..31][msg_0..222] —
    /// the remainder of x^(N-K)*m(x) mod g(x) occupies the LOW positions.
    pub fn encode(&self, msg: &[u8]) -> Vec<u8> {
        if msg.len() != RS_K {
            panic!("expected {RS_K} message bytes, got {}", msg.len());
        }
        let mut dividend = vec![0u8; RS_N];
        for (i, &b) in msg.iter().enumerate() {
            dividend[i + RS_PARITY] = b;
        }
        // Long division: process from the highest coefficient downward; gen is
        // stored in descending order (gen[j] = coeff of x^(N-K-j)), so
        // x^(i-N+K)*g places gen[j] at position i - j.
        // Long division must run from the highest coefficient downward.
        // (RS_PARITY..RS_N) is ascending 32..255; .rev() yields 254 -> 32.
        for i in (RS_PARITY..RS_N).rev() {
            let c = dividend[i];
            if c != 0 {
                for (j, g) in self.gen.iter().enumerate() {
                    if *g != 0 {
                        dividend[i - j] ^= self.gf.mul(c, *g);
                    }
                }
            }
        }
        let mut out = dividend[..RS_PARITY].to_vec();
        out.extend(msg.to_vec());
        out
    }

    /// Decode and correct up to RS_T symbol errors.
    /// `coded` layout: [parity_0..31][payload_0..222].
    pub fn decode(&self, coded: &[u8]) -> Result<Vec<u8>, RSError> {
        if coded.len() != RS_N {
            return Err(RSError::BadLength);
        }
        let syndromes = self.syndromes(coded);
        if syndromes.iter().all(|s| *s == 0) {
            return Ok(coded[RS_PARITY..].to_vec());
        }
        let loc = self.berlekamp_massey(&syndromes);
        let omega = self.omega(&syndromes, &loc);

        // Chien search: the BM connection polynomial is
        // C(x) = prod_j (1 - a^j x), so its roots are x = a^{-j}.
        // A root a^i therefore corresponds to error position j = (N - i) mod N.
        let mut errs: Vec<usize> = Vec::new();
        for i in 0..RS_N {
            let x = self.gf.exp[i] as u8;
            if self.poly_eval(&loc, x) == 0 {
                errs.push((RS_N - i) % RS_N);
            }
        }
        if errs.len() > RS_T {
            return Err(RSError::TooManyErrors);
        }

        let mut out = coded.to_vec();
        for &pos in &errs {
            // Forney (b=0): evaluate at x = a^{-pos} = a^{(N - pos) mod N}.
            let x = self.gf.exp[(RS_N - pos) % RS_N] as u8;
            let num = self.poly_eval(&omega, x);
            let den = self.poly_der_eval(&loc, x);
            if den == 0 {
                return Err(RSError::CheckFailed);
            }
            let mag = self
                .gf
                .mul(self.gf.div(num, den), self.gf.exp[pos % RS_N] as u8);
            out[pos] ^= mag;
        }
        // Recheck: if more errors exist than the code can correct, residual
        // syndromes remain after correction.
        if self.syndromes(&out).iter().any(|s| *s != 0) {
            return Err(RSError::TooManyErrors);
        }
        Ok(out[RS_PARITY..].to_vec())
    }

    /// The 2T syndromes S_l = c(a^l) for l = 0..2T-1.
    pub fn syndromes(&self, coded: &[u8]) -> Vec<u8> {
        let mut s = vec![0u8; 2 * RS_T];
        for (i, slot) in s.iter_mut().enumerate() {
            let x = self.gf.exp[i] as u8;
            let mut acc: u8 = 0;
            // c(x) = c_0 + c_1 x + ... + c_{N-1} x^{N-1}; Horner from the
            // highest coefficient down so acc == c(a^i) (ascending-power convention).
            for &b in coded.iter().rev() {
                acc = self.gf.mul(acc, x) ^ b;
            }
            *slot = acc;
        }
        s
    }

    /// Berlekamp-Massey: the connection polynomial C(x) = 1 + c_1 x + ...
    /// whose roots are 1/λ_j (λ_j = a^j), i.e. a^{-j} for the b=0 convention.
    pub fn berlekamp_massey(&self, s: &[u8]) -> Vec<u8> {
        let mut c = vec![1u8];
        let mut b = vec![1u8];
        let mut l = 0usize;
        let mut m = 1;
        let mut old_diff: u8 = 1;
        for i in 0..s.len() {
            let mut diff = s[i];
            for j in 1..c.len() {
                if i >= j {
                    let cj = c[j];
                    if cj != 0 {
                        diff ^= self.gf.mul(cj, s[i - j]);
                    }
                }
            }
            if diff == 0 {
                m += 1;
            } else {
                let t = c.clone();
                let coef = self.gf.div(diff, old_diff);
                while b.len() < c.len() {
                    b.push(0);
                }
                while c.len() < b.len() + m {
                    c.push(0);
                }
                for (j, &v) in b.iter().enumerate() {
                    if v != 0 {
                        c[j + m] ^= self.gf.mul(coef, v);
                    }
                }
                if 2 * l <= i {
                    l = i + 1 - l;
                    b = t;
                    m = 1;
                    old_diff = diff;
                } else {
                    m += 1;
                }
            }
        }
        c
    }

    /// omega(x) = S(x) * Lambda(x), truncated to degree 2T-1.
    pub fn omega(&self, s: &[u8], loc: &[u8]) -> Vec<u8> {
        let mut o = vec![0u8; 2 * RS_T];
        for (i, &li) in loc.iter().enumerate() {
            if li == 0 {
                continue;
            }
            for (j, &sj) in s.iter().enumerate() {
                if sj == 0 {
                    continue;
                }
                let k = i + j;
                if k < o.len() {
                    o[k] ^= self.gf.mul(li, sj);
                }
            }
        }
        o
    }

    /// Evaluate a polynomial (ascending coefficients) at x.
    /// Horner from the highest coefficient down gives c_0 + c_1 x + ... + c_n x^n.
    #[inline]
    fn poly_eval(&self, poly: &[u8], x: u8) -> u8 {
        let mut acc: u8 = 0;
        for &c in poly.iter().rev() {
            acc = self.gf.mul(acc, x) ^ c;
        }
        acc
    }

    /// Evaluate the formal derivative of a polynomial (ascending coefficients) at x.
    /// In characteristic 2, P'(x) = sum_{j odd} c_j x^{j-1}.
    fn poly_der_eval(&self, poly: &[u8], x: u8) -> u8 {
        let mut acc: u8 = 0;
        // Coefficient of x^k in P'(x) is c_{k+1} when k is even (k+1 odd), else 0.
        // Horner: iterate k from the highest power down to 0.
        for k in (0..poly.len().saturating_sub(1)).rev() {
            let c = if k % 2 == 0 { poly[k + 1] } else { 0 };
            acc = self.gf.mul(acc, x) ^ c;
        }
        acc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_message(seed: u64) -> Vec<u8> {
        let mut s = seed;
        let mut out = Vec::with_capacity(RS_K);
        for _ in 0..RS_K {
            s = s.wrapping_mul(6364136223846793045).wrapping_add(1);
            out.push((s >> 40) as u8);
        }
        out
    }

    #[test]
    fn roundtrip_clean() {
        let rs = ReedSolomon::new();
        let msg = random_message(42);
        let coded = rs.encode(&msg);
        assert_eq!(coded.len(), RS_N);
        let dec = rs.decode(&coded).unwrap();
        assert_eq!(dec, msg);
    }

    #[test]
    fn corrects_16_errors() {
        let rs = ReedSolomon::new();
        let msg = random_message(7);
        let mut coded = rs.encode(&msg);
        for pos in [
            3, 5, 9, 17, 42, 60, 77, 88, 100, 131, 145, 150, 199, 205, 233, 250,
        ] {
            coded[pos] ^= 0xA5;
        }
        let dec = rs.decode(&coded).unwrap();
        assert_eq!(dec, msg);
    }

    #[test]
    fn fails_at_17_errors() {
        let rs = ReedSolomon::new();
        let msg = random_message(7);
        let mut coded = rs.encode(&msg);
        for i in 0..17 {
            coded[i] ^= 0x5A;
        }
        match rs.decode(&coded) {
            Err(RSError::TooManyErrors) => {}
            other => panic!("expected TooManyErrors, got {other:?}"),
        }
    }

    #[test]
    fn wrong_length_rejected() {
        let rs = ReedSolomon::new();
        assert_eq!(rs.decode(&[0u8; 10]), Err(RSError::BadLength));
    }
}
