pub fn run_with_timeout(cmd: &str) -> Result<(), String> {
    Err("timeout".to_string())
}

pub fn run_with_timeout_v2(cmd: &str) -> Result<(), String> {
    Ok(())
}
// dead code — v2 variant defined but never used
