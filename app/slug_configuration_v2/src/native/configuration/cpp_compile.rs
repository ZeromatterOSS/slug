//! Checked projections for native CppCompile construction, not new option state.
use super::*;

impl SlugConfiguration {
    /// Bazel9.2 CppConfiguration.shouldScanIncludes: public OR internal flag.
    pub fn cpp_compile_include_scanning(&self) -> Result<bool, SlugConfigurationError> {
        let public = self.native_bool(
            CPP_OPTIONS,
            "experimental_unsupported_and_brittle_include_scanning",
        )?;
        let internal = self.native_bool(CPP_OPTIONS, "cc_include_scanning")?;
        Ok(public || internal)
    }

    pub fn cpp_compile_use_specific_tool_files(&self) -> Result<bool, SlugConfigurationError> {
        self.native_bool(CPP_OPTIONS, "incompatible_use_specific_tool_files")
    }

    /// Regex-backed execution-info modifiers remain unadmitted. Check the
    /// structural value, including configurations not built by a CLI overlay.
    pub fn cpp_compile_execution_modifiers_empty(&self) -> Result<bool, SlugConfigurationError> {
        match self.option_value(CORE_OPTIONS, "modify_execution_info")? {
            OptionValue::Native(NativeOccurrence::List(values)) => Ok(values.0.is_empty()),
            _ => Err(SlugConfigurationError::InvalidCppConfiguration {
                reason: "malformed modify_execution_info is not admitted",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> SlugConfiguration {
        SlugConfiguration::default_target(
            &HostConversionInputs::new(
                Some(AutoCpuToken::K8),
                Some(HostPathFlavor::Unix),
                None,
                Arc::from([]),
                Arc::from([]),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn replace(config: &SlugConfiguration, name: &str, value: OptionValue) -> SlugConfiguration {
        let mut options = config.0.options.to_vec();
        options
            .iter_mut()
            .find(|row| row.canonical_name == name)
            .unwrap()
            .value = value;
        finish_configuration(
            config.kind(),
            options.into(),
            config.starlark_options().clone(),
            config.action_environment_host().cloned(),
        )
    }

    fn boolean(value: bool) -> OptionValue {
        OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Bool(value)))
    }

    #[test]
    fn cpp_compile_policy_projects_both_scan_flags_and_specific_tools_structurally() {
        let original = base();
        assert!(!original.cpp_compile_include_scanning().unwrap());
        assert!(original.cpp_compile_use_specific_tool_files().unwrap());
        assert!(original.cpp_compile_execution_modifiers_empty().unwrap());
        for (public, internal) in [(false, false), (true, false), (false, true), (true, true)] {
            let changed = replace(
                &replace(
                    &original,
                    "experimental_unsupported_and_brittle_include_scanning",
                    boolean(public),
                ),
                "cc_include_scanning",
                boolean(internal),
            );
            assert_eq!(
                changed.cpp_compile_include_scanning().unwrap(),
                public || internal
            );
            let restored = replace(
                &replace(
                    &changed,
                    "experimental_unsupported_and_brittle_include_scanning",
                    boolean(false),
                ),
                "cc_include_scanning",
                boolean(false),
            );
            assert_eq!(restored, original);
        }
        let changed = replace(
            &original,
            "incompatible_use_specific_tool_files",
            boolean(false),
        );
        assert!(!changed.cpp_compile_use_specific_tool_files().unwrap());
        assert_ne!(changed, original);
        assert_eq!(
            replace(
                &changed,
                "incompatible_use_specific_tool_files",
                boolean(true)
            ),
            original
        );
        for name in [
            "cc_include_scanning",
            "experimental_unsupported_and_brittle_include_scanning",
        ] {
            assert!(
                replace(
                    &original,
                    name,
                    OptionValue::Native(NativeOccurrence::Absent)
                )
                .cpp_compile_include_scanning()
                .is_err()
            );
        }
    }

    #[test]
    fn cpp_compile_modifier_projection_rejects_nonempty_or_malformed_values() {
        let original = base();
        let changed = replace(
            &original,
            "modify_execution_info",
            OptionValue::Native(NativeOccurrence::List(NativeValues(Arc::from([
                NativeValue::Text("CppCompile=+custom".into()),
            ])))),
        );
        assert!(!changed.cpp_compile_execution_modifiers_empty().unwrap());
        assert!(
            replace(&original, "modify_execution_info", boolean(false))
                .cpp_compile_execution_modifiers_empty()
                .is_err()
        );
        assert_eq!(
            replace(
                &changed,
                "modify_execution_info",
                OptionValue::Native(NativeOccurrence::List(NativeValues(Arc::from([]))))
            ),
            original
        );
    }
}
