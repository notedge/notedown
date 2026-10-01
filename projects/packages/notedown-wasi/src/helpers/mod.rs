#[cfg(feature = "serde_json")]
mod for_json;
#[cfg(feature = "toml")]
mod for_toml;

#[cfg(feature = "toml")]
pub use for_toml::toml_to_object;

#[cfg(feature = "serde_json")]
pub use serde_json::Value as Json;
