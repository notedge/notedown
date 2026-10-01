// #![deny(missing_debug_implementations)]
// #[cfg(all(feature = "wasm", feature = "native"))]
// compile_error!("You can only choose one of `wasm` or `native` as the runtime!");
//
// pub mod document;
//
// mod builtin;
// mod file_system;
// mod plugin;
// mod plugin_system;
// mod vm;
//
// pub use self::{
//     file_system::VMFileSystem,
//     vm::{ContextKind, NoteVM},
// };
//
// pub use document::NoteDocument;
// pub use plugin_system::{ExtendedPackage, Parser, PluginParser, PluginSystem};
