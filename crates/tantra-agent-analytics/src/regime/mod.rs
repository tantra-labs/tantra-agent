//! Market regime detection engines (plan 0007 Track 4 Phase 9).

pub mod hmm;

pub use hmm::{HmmConfig, HmmResult, RegimeLabel, fit_gaussian_hmm};
