//! Default-path Apple rule fragment over the sole structural configuration.

use allocative::Allocative;
use dupe::Dupe;

use super::configuration::OptionValue;
use super::configuration::SlugConfiguration;
use super::configuration::SlugConfigurationError;
use super::value::NativeOccurrence;
use super::value::NativeValue;

const APPLE_OPTIONS: &str = "com.google.devtools.build.lib.rules.apple.AppleCommandLineOptions";

/// Evaluator-local view. Non-null dotted versions need a Bazel-shaped
/// Starlark value and are deliberately not exposed as strings.
#[derive(Clone, Debug, Eq, PartialEq, Allocative, Dupe)]
pub struct AppleFragmentProjection {
    configuration: SlugConfiguration,
}

impl AppleFragmentProjection {
    pub fn new(configuration: SlugConfiguration) -> Result<Self, SlugConfigurationError> {
        let projection = Self { configuration };
        projection.xcode_version_flag()?;
        projection.prefer_mutual_xcode()?;
        projection.ios_sdk_version_flag()?;
        projection.macos_sdk_version_flag()?;
        projection.tvos_sdk_version_flag()?;
        projection.watchos_sdk_version_flag()?;
        projection.ios_minimum_os_flag()?;
        projection.macos_minimum_os_flag()?;
        projection.tvos_minimum_os_flag()?;
        projection.watchos_minimum_os_flag()?;
        projection.include_xcode_exec_requirements()?;
        Ok(projection)
    }

    fn option(&self, name: &'static str) -> Result<&OptionValue, SlugConfigurationError> {
        self.configuration.option_value(APPLE_OPTIONS, name)
    }

    fn default_only_optional(
        value: &OptionValue,
        name: &'static str,
    ) -> Result<(), SlugConfigurationError> {
        match value {
            OptionValue::Native(NativeOccurrence::Absent) => Ok(()),
            _ => Err(SlugConfigurationError::UnsupportedAppleConfiguration { option: name }),
        }
    }

    fn absent_text(&self, name: &'static str) -> Result<Option<&str>, SlugConfigurationError> {
        Self::default_only_optional(self.option(name)?, name)?;
        Ok(None)
    }

    fn absent_dotted(&self, name: &'static str) -> Result<Option<&str>, SlugConfigurationError> {
        Self::default_only_optional(self.option(name)?, name)?;
        Ok(None)
    }

    pub fn xcode_version_flag(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.absent_text("xcode_version")
    }

    pub fn prefer_mutual_xcode(&self) -> Result<bool, SlugConfigurationError> {
        self.configuration
            .native_bool(APPLE_OPTIONS, "experimental_prefer_mutual_xcode")
    }

    pub fn ios_sdk_version_flag(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.absent_dotted("ios_sdk_version")
    }

    pub fn macos_sdk_version_flag(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.absent_dotted("macos_sdk_version")
    }

    pub fn tvos_sdk_version_flag(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.absent_dotted("tvos_sdk_version")
    }

    pub fn watchos_sdk_version_flag(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.absent_dotted("watchos_sdk_version")
    }

    pub fn ios_minimum_os_flag(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.absent_dotted("ios_minimum_os")
    }

    pub fn macos_minimum_os_flag(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.absent_dotted("macos_minimum_os")
    }

    pub fn tvos_minimum_os_flag(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.absent_dotted("tvos_minimum_os")
    }

    pub fn watchos_minimum_os_flag(&self) -> Result<Option<&str>, SlugConfigurationError> {
        self.absent_dotted("watchos_minimum_os")
    }

    pub fn include_xcode_exec_requirements(&self) -> Result<bool, SlugConfigurationError> {
        self.configuration.native_bool(
            APPLE_OPTIONS,
            "experimental_include_xcode_execution_requirements",
        )
    }

    /// Bazel 9.2's default selector maps either supported host architecture
    /// to ApplePlatform.MACOS. Keep this check lazy: other admitted Apple
    /// fragment fields remain available with nondefault selector options.
    pub fn single_arch_platform_is_default_macos(&self) -> Result<(), SlugConfigurationError> {
        Self::validate_default_macos_selector(
            self.option("apple_platform_type")?,
            self.option("apple_split_cpu")?,
            self.option("macos_cpus")?,
        )
    }

    fn validate_default_macos_selector(
        platform_type: &OptionValue,
        split_cpu: &OptionValue,
        macos_cpus: &OptionValue,
    ) -> Result<(), SlugConfigurationError> {
        if !matches!(platform_type, OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text(value))) if value == "macos")
        {
            return Err(SlugConfigurationError::UnsupportedAppleConfiguration {
                option: "apple_platform_type",
            });
        }
        if !matches!(split_cpu, OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text(value))) if value.is_empty())
        {
            return Err(SlugConfigurationError::UnsupportedAppleConfiguration {
                option: "apple_split_cpu",
            });
        }
        let default_macos_cpus = match macos_cpus {
            OptionValue::Native(NativeOccurrence::Absent) => true,
            OptionValue::Native(NativeOccurrence::List(values)) => values.0.is_empty(),
            _ => false,
        };
        if !default_macos_cpus {
            return Err(SlugConfigurationError::UnsupportedAppleConfiguration {
                option: "macos_cpus",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::native::host::ActionEnvironmentHost;
    use crate::native::host::ActionEnvironmentHostOs;
    use crate::native::host::AutoCpuToken;
    use crate::native::host::HostConversionInputs;
    use crate::native::host::HostPathFlavor;
    use crate::native::value::NativeValue;
    use crate::native::value::NativeValues;

    fn default_target() -> SlugConfiguration {
        let host = HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap()
        .with_action_environment_host(ActionEnvironmentHost::without_environment(
            ActionEnvironmentHostOs::Linux,
        ));
        SlugConfiguration::default_target(&host).unwrap()
    }

    #[test]
    fn apple_fragment_default_target_and_exec_project_structural_nulls_and_bools() {
        let target = default_target();
        for configuration in [target.clone(), target.to_exec().unwrap()] {
            let apple = AppleFragmentProjection::new(configuration).unwrap();
            assert_eq!(apple.xcode_version_flag().unwrap(), None);
            assert!(apple.prefer_mutual_xcode().unwrap());
            for value in [
                apple.ios_sdk_version_flag().unwrap(),
                apple.macos_sdk_version_flag().unwrap(),
                apple.tvos_sdk_version_flag().unwrap(),
                apple.watchos_sdk_version_flag().unwrap(),
                apple.ios_minimum_os_flag().unwrap(),
                apple.macos_minimum_os_flag().unwrap(),
                apple.tvos_minimum_os_flag().unwrap(),
                apple.watchos_minimum_os_flag().unwrap(),
            ] {
                assert_eq!(value, None);
            }
            assert!(!apple.include_xcode_exec_requirements().unwrap());
            assert_eq!(apple.single_arch_platform_is_default_macos(), Ok(()));
        }
    }

    #[test]
    fn apple_single_arch_platform_rejects_nondefault_and_malformed_selectors() {
        let apple = AppleFragmentProjection::new(default_target()).unwrap();
        let defaults = [
            apple.option("apple_platform_type").unwrap().clone(),
            apple.option("apple_split_cpu").unwrap().clone(),
            apple.option("macos_cpus").unwrap().clone(),
        ];
        assert_eq!(
            AppleFragmentProjection::validate_default_macos_selector(
                &defaults[0],
                &defaults[1],
                &defaults[2]
            ),
            Ok(())
        );
        assert!(matches!(
            &defaults[2],
            OptionValue::Native(NativeOccurrence::List(values)) if values.0.is_empty()
        ));
        for (index, option, invalid) in [
            (
                0,
                "apple_platform_type",
                OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text("ios".into()))),
            ),
            (
                0,
                "apple_platform_type",
                OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(true))),
            ),
            (
                1,
                "apple_split_cpu",
                OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text("arm64".into()))),
            ),
            (
                1,
                "apple_split_cpu",
                OptionValue::Native(NativeOccurrence::Absent),
            ),
            (
                2,
                "macos_cpus",
                OptionValue::Native(NativeOccurrence::List(NativeValues(Arc::from([
                    NativeValue::Text("arm64".into()),
                ])))),
            ),
            (
                2,
                "macos_cpus",
                OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text("arm64".into()))),
            ),
        ] {
            let mut values = defaults.clone();
            values[index] = invalid;
            assert_eq!(
                AppleFragmentProjection::validate_default_macos_selector(
                    &values[0], &values[1], &values[2]
                ),
                Err(SlugConfigurationError::UnsupportedAppleConfiguration { option })
            );
            // The default projection still serves the previously admitted fields;
            // the getter alone invokes the selector validator above.
            assert_eq!(apple.xcode_version_flag().unwrap(), None);
            assert!(apple.prefer_mutual_xcode().unwrap());
        }
        assert_eq!(
            AppleFragmentProjection::validate_default_macos_selector(
                &defaults[0],
                &defaults[1],
                &OptionValue::Native(NativeOccurrence::Absent)
            ),
            Ok(())
        );
    }

    #[test]
    fn apple_fragment_whole_configuration_identity_restores_a_b_a() {
        let a = default_target();
        let b = a.with_incompatible_auto_exec_groups(true);
        let first = AppleFragmentProjection::new(a.clone()).unwrap();
        let changed = AppleFragmentProjection::new(b).unwrap();
        let restored = AppleFragmentProjection::new(a).unwrap();
        assert_ne!(first, changed);
        assert_eq!(first, restored);
        assert_eq!(
            first.xcode_version_flag().unwrap(),
            changed.xcode_version_flag().unwrap()
        );
    }

    #[test]
    fn apple_fragment_nonnull_dotted_and_text_are_not_string_substitutes() {
        // Test the typed gate directly: explicit Apple CLI flags are not admitted
        // into CommandConfigurationOverlay yet, so no fabricated command route.
        for (option, value) in [
            (
                "ios_sdk_version",
                OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Dotted("18.0".into()))),
            ),
            (
                "xcode_version",
                OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Text("16.0".into()))),
            ),
            (
                "ios_sdk_version",
                OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(true))),
            ),
        ] {
            assert_eq!(
                AppleFragmentProjection::default_only_optional(&value, option),
                Err(SlugConfigurationError::UnsupportedAppleConfiguration { option })
            );
        }
        assert_eq!(
            AppleFragmentProjection::default_only_optional(
                &OptionValue::Native(NativeOccurrence::Absent),
                "ios_sdk_version"
            ),
            Ok(())
        );
    }
}
