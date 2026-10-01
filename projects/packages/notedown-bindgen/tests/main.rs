#![allow(dead_code)]
#![allow(non_camel_case_types)]

use bindgen::generate_rust;
use std::path::Path;

#[test]
fn ready() {
    println!("it, works!")
}

#[test]
fn export() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let _ = generate_rust(&here.join("..").join("notedown-wasi"));
    let _ = generate_rust(Path::new(r#"C:\Users\Dell\CLionProjects\dejavu-engine\projects\dejavu-wasi"#));
    let _ = generate_rust(Path::new(r#"C:\Users\Dell\CLionProjects\notedown-rs\projects\notedown-json"#));
}
