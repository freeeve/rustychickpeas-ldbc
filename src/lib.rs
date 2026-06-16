//! Shared library for the rustychickpeas LDBC benchmark families.
//!
//! `loader`, `props`, and `harness` are the reusable infrastructure each family
//! binary (`src/bin/*.rs`) builds on; `bi` is the first family — the faithful
//! BI Q1–Q20 plus the simplified BI1–6 patterns. New families (Interactive,
//! Graphalytics, FinBench, SPB) add a sibling module here plus a thin bin.

pub mod harness;
pub mod loader;
pub mod props;

pub mod bi;
pub mod spb;

pub use harness::Result;
pub use loader::{load_graph, Stats};
