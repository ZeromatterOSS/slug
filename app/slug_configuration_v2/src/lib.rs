//! Immutable Bazel 9.2 native configuration metadata.

mod command;
pub mod native;

pub use command::CommandConfigurationOccurrence;
pub use command::CommandConfigurationOverlay;
pub use native::NativeStringListOption;
pub use native::PreparedCommandNativeOptions;
pub use native::SlugConfiguration;
pub use native::SlugConfigurationError;
pub use native::SlugConfigurationKind;
pub use native::SlugConfigurationProjection;
pub use native::StarlarkOption;
pub use native::StarlarkOptionScope;
pub use native::StarlarkOptionValue;
pub use native::StarlarkOptions;
