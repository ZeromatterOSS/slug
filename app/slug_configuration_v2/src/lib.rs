//! Immutable Bazel 9.2 native configuration metadata.

pub mod native;

pub use native::SlugConfiguration;
pub use native::SlugConfigurationError;
pub use native::SlugConfigurationKind;
pub use native::SlugConfigurationProjection;
pub use native::StarlarkOption;
pub use native::StarlarkOptionScope;
pub use native::StarlarkOptionValue;
pub use native::StarlarkOptions;
