#![allow(unused_braces)]

use std::{fs::File, io::Write, path::Path};
use wit_bindgen_core::Files;
use wit_parser::Resolve;

pub fn generate_rust(rust_project: &Path) -> anyhow::Result<()> {
    let config = wit_bindgen_rust::Opts { rustfmt: false, generate_unused_types: true, ..wit_bindgen_rust::Opts::default() };
    let mut generator = config.build();
    let mut resolve = Resolve::default();
    let (pkg, _) = resolve.push_dir(&rust_project.join("wit"))?;
    let world = resolve.select_world(pkg, None)?;
    let mut files = Files::default();
    generator.generate(&resolve, world, &mut files)?;

    let wit = rust_project.join("src").join("exports");
    // 检查目录是否存在并生成
    if !wit.exists() {
        std::fs::create_dir_all(&wit)?;
    }

    for (file, text) in files.iter() {
        let mut file = File::create(wit.join(file))?;
        file.write_all(text)?;
    }
    Ok(())
}
