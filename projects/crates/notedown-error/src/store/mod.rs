mod image_record;
use crate::Result;
use dashmap::DashMap;
use image::{io::Reader, DynamicImage, ImageFormat};

pub use self::image_record::ImageRecord;
use url::Url;

pub struct ImageStorage {
    force_backup: bool,
    local_store: DashMap<String, ImageRecord<()>>,
    remote: DashMap<String, ImageRemoteService>,
}

pub struct ImageRemoteService {
    name: String,
    remote_store: DashMap<u64, Url>,
}

impl ImageStorage {
    pub fn get_image_png(&self, name: &str) {}
    #[inline]
    pub fn get_image_local_path(&self, name: &str) -> Option<Url> {
        self.local_store.get(name).and_then(|f| f.get_local_path().cloned())
    }
    pub fn get_image_www() {}
}
