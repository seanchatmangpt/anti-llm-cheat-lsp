//! Little's Law WIP observation and closure-frontier engine.
//!
//! This module is intentionally read-only. It converts admitted repository/source
//! observations into WIP objects and closure intents. It never executes a closure
//! action and therefore cannot bypass the repository's actuation/receipt boundary.

#![allow(missing_docs)]
#![allow(clippy::pedantic)]

mod analyze;
mod frontier;
mod model;
mod pareto;
mod render;
mod source;
mod tree_sitter_scan;

pub use analyze::{analyze, analyze_path, load_snapshot};
pub use frontier::closure_priority;
pub use model::*;
pub use pareto::{
    errc_lane, errc_pareto, wip_impact, ErrcLane, ParetoSummary, ParetoWip,
    DEFAULT_PARETO_TARGET_SHARE,
};
pub use source::scan_source_wip;
pub use tree_sitter_scan::{is_tree_sitter_path, supported_tree_sitter_languages};
