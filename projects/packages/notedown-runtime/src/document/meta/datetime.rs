use super::*;

#[derive(Debug, Clone)]
pub enum DocumentTime {
    /// `\date: runtime-today`
    RuntimeToday,
    /// `\date: file-changed`
    FileChanged,
    /// `\date: file-created`
    FileCreated,
    /// `\date: git-changed`
    GitChanged,
    /// `\date: git-created`
    GitCreated,
    /// UTC time in any cases
    DateTime(NaiveDateTime),
}

impl Default for DocumentTime {
    fn default() -> Self {
        Self::RuntimeToday
    }
}

impl DocumentTime {
    #[inline]
    pub fn parse_unix(date: i64) -> Self {
        let time = NaiveDateTime::from_timestamp(date, 0);
        Self::DateTime(time)
    }
    #[inline]
    pub fn parse_fmt(date: &str, fmt: &str) -> Result<Self> {
        let time = NaiveDateTime::parse_from_str(date, fmt)?;
        Ok(Self::DateTime(time))
    }
    #[inline]
    pub fn now() -> DocumentTime {
        Self::DateTime(Utc::now().naive_utc())
    }

    pub fn instantiate(&self, file: &Url) -> Option<NaiveDateTime> {
        #[cfg(feature = "native")]
        {
            return self.instantiate_native(file);
        }
        #[cfg(feature = "wasm")]
        {
            return self.instantiate_wasm(file);
        }
    }
}

#[cfg(feature = "native")]
impl DocumentTime {
    #[inline(always)]
    fn instantiate_native(&self, file: &Url) -> Option<NaiveDateTime> {
        match self {
            Self::RuntimeToday => None,
            Self::FileChanged => Self::instantiate_file_time(false, file).ok(),
            Self::FileCreated => Self::instantiate_file_time(true, file).ok(),
            Self::GitChanged => Self::instantiate_git_time(false, file).ok(),
            Self::GitCreated => Self::instantiate_git_time(true, file).ok(),
            Self::DateTime(t) => Some(t.to_owned()),
        }
    }

    fn instantiate_git_time(created: bool, file: &Url) -> Result<NaiveDateTime> {
        let path = file.to_file_path()?;
        let git_root = VMFileSystem::find_git_root(&path)?;

        let _ = created;

        todo!()
    }

    fn instantiate_file_time(created: bool, file: &Url) -> Result<NaiveDateTime> {
        let meta = file.to_file_path()?.metadata()?;
        let sys_time = match created {
            true => meta.created()?,
            false => meta.modified()?,
        };
        Ok(Self::system_time_to_date_time(sys_time).naive_utc())
    }

    fn system_time_to_date_time(t: SystemTime) -> DateTime<Utc> {
        let (sec, nsec) = match t.duration_since(UNIX_EPOCH) {
            Ok(dur) => (dur.as_secs() as i64, dur.subsec_nanos()),
            Err(e) => {
                // unlikely but should be handled
                let dur = e.duration();
                let (sec, nsec) = (dur.as_secs() as i64, dur.subsec_nanos());
                if nsec == 0 { (-sec, 0) } else { (-sec - 1, 1_000_000_000 - nsec) }
            }
        };
        Utc.timestamp(sec, nsec)
    }
}

#[cfg(feature = "wasm")]
impl DocumentTime {
    #[inline(always)]
    fn instantiate_wasm(&self, _: &Url) -> Option<NaiveDateTime> {
        match self {
            Self::RuntimeToday | Self::FileChanged | Self::FileCreated | Self::GitChanged | Self::GitCreated => None,
            Self::DateTime(t) => Some(t.to_owned()),
        }
    }
}

/// Methods about [`DocumentDate`]
impl NoteDocument {
    #[inline]
    pub fn get_date(&self) -> &DocumentTime {
        &self.meta.date
    }
    #[inline]
    pub fn set_date(&mut self, date: DocumentTime) {
        self.meta.date = date;
    }
}
