pub fn run_with_timeout(cmd: &str) -> Result<(), String> {
    Err("timeout".to_string())
}

pub fn run_with_timeout_v2(cmd: &str) -> Result<(), String> {
    Ok(())
}
// run_with_timeout_v2 is never called
