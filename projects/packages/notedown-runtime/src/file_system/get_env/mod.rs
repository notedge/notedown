use super::*;

impl VMFileSystem {
    pub const NOTEDOWN_ROOT: &'static str = "NOTEDOWN_ROOT";
    pub fn set_pkg_root<P: AsRef<Path>>(path: P) -> Result<()> {
        Ok(std::env::set_var(Self::NOTEDOWN_ROOT, path.as_ref().display().to_string()))
    }

    pub fn get_pkg_root() -> Result<PathBuf> {
        let maybe_dir = PathBuf::from(get_env_var(Self::NOTEDOWN_ROOT)?);
        match maybe_dir.is_dir() {
            true => Ok(maybe_dir),
            false => Err(QError::runtime_error(&format!("The environment variable {} does not point to a directory!", Self::NOTEDOWN_ROOT))),
        }
    }
}

fn get_env_var(key: &str) -> Result<String> {
    match std::env::var(VMFileSystem::NOTEDOWN_ROOT) {
        Ok(o) => Ok(o),
        Err(VarError::NotPresent) => Err(QError::runtime_error(&format!("The environment variable {} does not found!", key))),
        Err(VarError::NotUnicode(s)) => Err(QError::runtime_error(&format!("The environment variable {} seems not valid unicode: {:?}", key, s))),
    }
}
