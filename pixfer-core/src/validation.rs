//! Input validation framework: a `Validator` trait, a registry of named
//! validators, and concrete validators for payloads, configs, and frames.
//! (Hilariously over-engineered: a validation layer for a light link.)

use std::collections::BTreeMap;

use crate::config::Config;
use crate::protocol::{Frame, FrameKind};
use crate::rs::RS_K;

/// A named validator with a human-readable rule.
pub trait Validator: Send {
    fn name(&self) -> &'static str;
    fn rule(&self) -> &'static str;
    /// Validate `value` and return the list of problems (empty = valid).
    fn problems(&self, value: &[u8]) -> Vec<String>;
}

/// Validate that a payload is within the wire-format bounds.
pub struct PayloadValidator;

impl Validator for PayloadValidator {
    fn name(&self) -> &'static str {
        "payload"
    }
    fn rule(&self) -> &'static str {
        "payload non-empty and <= RS_K bytes"
    }
    fn problems(&self, value: &[u8]) -> Vec<String> {
        let mut p = Vec::new();
        if value.is_empty() {
            p.push("payload must be non-empty".into());
        }
        if value.len() > RS_K {
            p.push(format!(
                "payload {} bytes exceeds RS_K ({})",
                value.len(),
                RS_K
            ));
        }
        p
    }
}

/// Validate a config instance (delegates to Config::validate).
pub struct ConfigValidator;

impl ConfigValidator {
    pub fn problems(&self, config: &Config) -> Vec<String> {
        config.validate()
    }
}

/// Validate a decoded frame's invariants.
pub struct FrameValidator2;

impl FrameValidator2 {
    pub fn problems(&self, f: &Frame) -> Vec<String> {
        let mut p = Vec::new();
        if f.payload.len() > RS_K {
            p.push(format!(
                "frame payload {} bytes exceeds RS_K ({})",
                f.payload.len(),
                RS_K
            ));
        }
        if matches!(f.kind, FrameKind::Chunk) && f.total_chunks > 0 && f.chunk_idx >= f.total_chunks
        {
            p.push(format!(
                "chunk_idx {} out of range for total {}",
                f.chunk_idx, f.total_chunks
            ));
        }
        p
    }
}

/// A registry of named payload validators.
#[derive(Default)]
pub struct ValidatorRegistry {
    validators: BTreeMap<&'static str, Box<dyn Validator>>,
}

impl ValidatorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, v: Box<dyn Validator>) {
        self.validators.insert(v.name(), v);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Validator>> {
        self.validators.get(name)
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.validators.keys().copied().collect()
    }

    /// Run every registered validator over `value`; return the combined problems.
    pub fn run_all(&self, value: &[u8]) -> Vec<String> {
        let mut all = Vec::new();
        for v in self.validators.values() {
            all.extend(v.problems(value));
        }
        all
    }

    /// The standard registry: payload + config + frame validators.
    pub fn standard() -> Self {
        let mut r = Self::new();
        r.register(Box::new(PayloadValidator));
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_validator_bounds() {
        let v = PayloadValidator;
        assert_eq!(
            v.problems(&[]),
            vec!["payload must be non-empty".to_string()]
        );
        assert!(v.problems(&[1]).is_empty());
        let big = vec![0u8; RS_K + 1];
        let p = v.problems(&big);
        assert!(p.iter().any(|x| x.contains("exceeds RS_K")));
    }

    #[test]
    fn config_validator_delegates() {
        let v = ConfigValidator;
        assert!(v.problems(&Config::default()).is_empty());
        let mut bad = Config::default();
        bad.default_throughput = 0.0;
        assert!(!v.problems(&bad).is_empty());
    }

    #[test]
    fn frame_validator_checks() {
        let v = FrameValidator2;
        let good = Frame::new(crate::protocol::FrameKind::Chunk, 1, 0, 4, vec![9]);
        assert!(v.problems(&good).is_empty());
        let bad = Frame::new(crate::protocol::FrameKind::Chunk, 1, 4, 4, vec![9]);
        assert!(!v.problems(&bad).is_empty());
    }

    #[test]
    fn registry_runs_all() {
        let r = ValidatorRegistry::standard();
        assert_eq!(r.names(), vec!["payload"]);
        assert!(r.run_all(&[1, 2, 3]).is_empty());
        let big = vec![0u8; RS_K + 1];
        assert!(!r.run_all(&big).is_empty());
        assert!(r.get("payload").is_some());
        assert!(r.get("nope").is_none());
    }

    #[test]
    fn registry_run_all_combines_problems() {
        let mut r = ValidatorRegistry::new();
        r.register(Box::new(PayloadValidator));
        let empty: Vec<u8> = Vec::new();
        let p = r.run_all(&empty);
        assert_eq!(p, vec!["payload must be non-empty".to_string()]);
    }
}
