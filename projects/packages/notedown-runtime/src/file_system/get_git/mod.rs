use super::*;

impl VMFileSystem {
    #[cfg(feature = "native")]
    pub fn find_git_root(base: &Path) -> Result<PathBuf> {
        let mut path: PathBuf = base.into();
        let git_root = Path::new(".git");
        loop {
            path.push(git_root);

            if path.is_file() {
                break Ok(path);
            }

            if !(path.pop() && path.pop()) {
                // remove file && remove parent
                break Err(QError::runtime_error("git root not found"));
            }
        }
    }
    #[cfg(feature = "native")]
    pub fn find_git_repo(base: &Path) -> Result<Repository> {
        let root = Self::find_git_root(base)?;
        Ok(Repository::open(root)?)
    }
}
