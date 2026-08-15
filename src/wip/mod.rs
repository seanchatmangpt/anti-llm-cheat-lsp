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
mod render;
mod source;

pub use analyze::{analyze, analyze_path, load_snapshot};
pub use frontier::closure_priority;
pub use model::*;
pub use source::scan_source_wip;
