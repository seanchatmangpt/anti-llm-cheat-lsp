// Substring check smells
fn verify_file(content: &str, path_str: &str) {
    if content.contains("TODO") {
        println!("Contains TODO");
    }
    if path_str.ends_with(".rs") {
        println!("Ends with .rs");
    }
}
