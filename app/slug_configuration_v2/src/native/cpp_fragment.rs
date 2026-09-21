//! Phase-scratch Starlark projection of the admitted Bazel C++ fragment slice.

use allocative::Allocative;
use dupe::Dupe;
use slug_identity_v2::CanonicalLabel;

use super::configuration::OptionValue;
use super::configuration::SlugConfiguration;
use super::configuration::SlugConfigurationError;
use super::value::EnumFamily;
use super::value::NativeOccurrence;
use super::value::NativeValue;

const CORE_OPTIONS: &str = "com.google.devtools.build.lib.analysis.config.CoreOptions";
const CPP_OPTIONS: &str = "com.google.devtools.build.lib.rules.cpp.CppOptions";

/// A cheap evaluator-local view over the sole structural configuration owner.
#[derive(Clone, Debug, Eq, PartialEq, Allocative, Dupe)]
pub struct CppFragmentProjection {
    configuration: SlugConfiguration,
}

impl CppFragmentProjection {
    pub fn new(configuration: SlugConfiguration) -> Result<Self, SlugConfigurationError> {
        configuration.validate_cpp_field_state()?;
        let projection = Self { configuration };
        projection.compilation_mode()?;
        projection.optional_text("cs_fdo_absolute_path")?;
        projection.optional_text("propeller_optimize_absolute_cc_profile")?;
        projection.optional_text("propeller_optimize_absolute_ld_profile")?;
        projection.proto_profile()?;
        projection.custom_malloc()?;
        projection.force_pic()?;
        projection.apple_generate_dsym()?;
        projection.dont_enable_host_nonhost()?;
        projection.use_llvm_coverage_map_format()?;
        projection.fdo_instrument()?;
        projection.cs_fdo_instrument()?;
        projection.fdo_prefetch_hints_label()?;
        Ok(projection)
    }

    pub fn compilation_mode(&self) -> Result<&str, SlugConfigurationError> {
        match self
            .configuration
            .option_value(CORE_OPTIONS, "compilation_mode")?
        {
            OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Enum(value)))
                if value.family == EnumFamily::Compilation =>
            {
                Ok(value.member.as_str())
            }
            _ => Err(SlugConfigurationError::InvalidCppConfiguration {
                reason: "compilation_mode has an invalid retained value",
            }),
        }
    }

    /// Label-backed `--fdo_optimize` state is intentionally not an absolute path.
    pub fn fdo_path(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.configuration.validate_cpp_field_state()?;
        Ok(None)
    }

    pub fn cs_fdo_path(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.optional_text("cs_fdo_absolute_path")
    }

    pub fn propeller_optimize_absolute_cc_profile(
        &self,
    ) -> Result<Option<&str>, SlugConfigurationError> {
        self.optional_text("propeller_optimize_absolute_cc_profile")
    }

    pub fn propeller_optimize_absolute_ld_profile(
        &self,
    ) -> Result<Option<&str>, SlugConfigurationError> {
        self.optional_text("propeller_optimize_absolute_ld_profile")
    }

    pub fn proto_profile(&self) -> Result<bool, SlugConfigurationError> {
        match self
            .configuration
            .option_value(CPP_OPTIONS, "proto_profile")?
        {
            OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(value))) => Ok(*value),
            _ => Err(SlugConfigurationError::InvalidCppConfiguration {
                reason: "proto_profile has an invalid retained value",
            }),
        }
    }

    pub fn custom_malloc(&self) -> Result<Option<CanonicalLabel>, SlugConfigurationError> {
        self.configuration.cpp_custom_malloc_label()
    }

    pub fn force_pic(&self) -> Result<bool, SlugConfigurationError> {
        self.configuration.native_bool(CPP_OPTIONS, "force_pic")
    }
    pub fn apple_generate_dsym(&self) -> Result<bool, SlugConfigurationError> {
        self.configuration
            .native_bool(CPP_OPTIONS, "apple_generate_dsym")
    }
    pub fn dont_enable_host_nonhost(&self) -> Result<bool, SlugConfigurationError> {
        self.configuration.native_bool(
            CPP_OPTIONS,
            "incompatible_dont_enable_host_nonhost_crosstool_features",
        )
    }
    pub fn use_llvm_coverage_map_format(&self) -> Result<bool, SlugConfigurationError> {
        self.configuration
            .native_bool(CPP_OPTIONS, "experimental_use_llvm_covmap")
    }
    // Instrumentation strings are raw option text, not absolute profile paths.
    pub fn fdo_instrument(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.configuration.cpp_text("fdo_instrument")
    }
    pub fn cs_fdo_instrument(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.configuration.cpp_text("cs_fdo_instrument")
    }
    pub fn fdo_prefetch_hints_label(
        &self,
    ) -> Result<Option<CanonicalLabel>, SlugConfigurationError> {
        self.configuration.cpp_label("fdo_prefetch_hints")
    }

    fn optional_text(&self, name: &'static str) -> Result<Option<&str>, SlugConfigurationError> {
        match self.configuration.option_value(CPP_OPTIONS, name)? {
            OptionValue::Native(NativeOccurrence::Absent) => Ok(None),
            OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text(_))) => {
                Err(SlugConfigurationError::InvalidCppConfiguration {
                    reason: "absolute C++ profile paths are not admitted",
                })
            }
            _ => Err(SlugConfigurationError::InvalidCppConfiguration {
                reason: "an absolute C++ profile has an invalid retained value",
            }),
        }
    }
}
