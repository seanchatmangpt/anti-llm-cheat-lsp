// Safe usage — these should NOT trigger
fn is_done() -> bool { false }
fn solve_problem() -> Result<(), ()> { Ok(()) }
fn guarantee_delivery() {}
fn ensure_clean_state() {}

pub fn main() {}
