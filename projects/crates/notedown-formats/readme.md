# notedown-formats

Notedown document semantic import and export modules. Composes Oak text parsers and Acorn binary views into `notedown-ir`.

Panduck orchestrates these contracts. This crate does not depend on Panduck.

Format dependencies are feature-gated. Skeleton modules return `FormatError::NotImplemented` until Oak/Acorn wiring lands in `R-7.3+`.
