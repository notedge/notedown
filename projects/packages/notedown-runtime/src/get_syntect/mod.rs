use crate::get_root_path;
use notedown_ast::Result;
use self_update::Download;
use std::{fs::File, io::Read, path::PathBuf};
use syntect::{dumps::from_binary, parsing::SyntaxSet};

pub fn get_syntect_language_pack() -> Result<SyntaxSet> {
    let file = lang_pack_path()?;
    let mut f = File::open(&file)?;
    let cap = std::fs::metadata(&file)?.len();
    let mut buffer = vec![0; cap as usize];
    f.read(&mut buffer)?;
    let s: SyntaxSet = from_binary(&buffer);
    Ok(s)
}

pub fn get_syntect_language_addition() -> Result<()> {
    let root = get_root_path()?;
    root.join("syntect").join("languages");
    Ok(())
}

pub fn get_syntect_themes() {}

pub fn download_syntect_prebuilt(_: &str) -> Result<()> {
    todo!()
}

#[inline]
fn lang_pack_path() -> Result<PathBuf> {
    let root = get_root_path()?;
    Ok(root.join("syntect").join("languages").join("base.pack"))
}
