// This file tests detection of hollow implementation patterns.
// It should trigger ANTI-LLM-HOLLOW-001, ANTI-LLM-HOLLOW-002, ANTI-LLM-HOLLOW-004

fn hollow_function() {
    unimplemented!()
}

fn todo_function() {
    todo!("implement this later")
}

// TODO: implement the real logic here
fn another_hollow() -> i32 {
    42
}
