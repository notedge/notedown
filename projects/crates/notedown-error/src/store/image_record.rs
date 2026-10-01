use super::*;

pub enum ImageLocal {
    External { source: Url },
    Managed { image: DynamicImage, source: Option<Url> },
}

impl ImageLocal {
    #[inline]
    pub fn get_path(&self) -> Option<&Url> {
        match self {
            Self::External { source } => Some(source),
            Self::Managed { source: Some(source), .. } => Some(source),
            _ => None,
        }
    }

    #[inline]
    #[rustfmt::skip]
    pub fn get_image(&mut self) -> Option<&DynamicImage> {
        self.as_managed(None).ok()?;
        match &*self {
            Self::Managed { image, .. } => Some(image),
            _ => None,
        }
    }

    #[inline]
    #[rustfmt::skip]
    pub fn as_managed(&mut self, format: Option<ImageFormat>) -> Result<()> {
        if let Self::External { source } = self {
            let mut local_file = Reader::open(source.to_file_path()?)?;
            if let Some(s) = format { local_file.set_format(s) }
            *self = Self::Managed { image: local_file.decode()?, source: Some(source.to_owned()) };
        }
        Ok(())
    }
    #[inline]
    pub fn update(&mut self, url: Url, format: Option<ImageFormat>) -> Result<()> {
        let mut src = Self::External { source: url };
        src.as_managed(format)
    }
}
/// [./img :]
/// [!img/   ]
pub struct ImageRecord<M> {
    md5: u64,
    local: Option<DynamicImage>,
    source: Option<Url>,
    meta_info: M,
}

impl<M> ImageRecord<M> {
    #[inline]
    pub fn get_local_path(&self) -> Option<&Url> {
        match &self.source {
            None => None,
            Some(s) => Some(s),
        }
    }
    #[inline]
    pub fn update(&mut self, f: Option<ImageFormat>) -> Result<DynamicImage> {
        let path = self.get_local_path().ok_or(())?.to_file_path()?;
        match f {
            Some(format) => Ok(Reader::open(path)?.decode()?),
            None => Ok(Reader::open(path)?.decode()?),
        }
    }
    #[inline]
    pub fn unlink(&mut self) {
        self.source = None
    }
    #[inline]
    pub fn check(&mut self) {}
    #[inline]
    pub fn get_meta(&self) -> &M {
        &self.meta_info
    }
}
