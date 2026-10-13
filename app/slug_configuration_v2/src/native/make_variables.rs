//! Borrowed configuration inputs for Bazel's immutable ctx.var dictionary.
//! Platform and provider suppliers remain with their existing Analysis owners.
use super::configuration::OptionRecord;
use super::configuration::OptionValue;
use super::configuration::SlugConfiguration;
use super::configuration::SlugConfigurationError;
use super::value::EnumFamily;
use super::value::NativeOccurrence;
use super::value::NativeValue;
use super::value::NativeValues;

const CORE_OPTIONS: &str = "com.google.devtools.build.lib.analysis.config.CoreOptions";

#[derive(Debug, Clone, Copy)]
pub struct MakeVariableConfigurationInputs<'a> {
    compilation_mode: &'a str,
    defines: &'a [NativeValue],
}

impl<'a> MakeVariableConfigurationInputs<'a> {
    pub fn compilation_mode(&self) -> &'a str {
        self.compilation_mode
    }

    pub fn defines(&self) -> impl ExactSizeIterator<Item = (&'a str, &'a str)> + 'a {
        self.defines.iter().map(|entry| match entry {
            NativeValue::Entry(key, value) => (key.as_str(), value.as_str()),
            _ => unreachable!("make-variable projection validated every define"),
        })
    }
}

impl SlugConfiguration {
    /// Validate supported controls only when the general make-variable API is
    /// requested. Unrelated configured rules do not acquire these refusals.
    pub fn make_variable_inputs(
        &self,
    ) -> Result<MakeVariableConfigurationInputs<'_>, SlugConfigurationError> {
        let unsupported = |option| SlugConfigurationError::UnsupportedMakeVariableOption { option };
        if !self.native_bool(CORE_OPTIONS, "incompatible_target_cpu_from_platform")? {
            return Err(unsupported("incompatible_target_cpu_from_platform"));
        }
        if !matches!(
            self.option_value(CORE_OPTIONS, "experimental_override_platform_cpu_name")?,
            OptionValue::Label(None)
        ) {
            return Err(unsupported("experimental_override_platform_cpu_name"));
        }
        if self.native_bool(
            CORE_OPTIONS,
            "experimental_exclude_defines_from_exec_config",
        )? {
            return Err(unsupported("experimental_exclude_defines_from_exec_config"));
        }
        if !matches!(
            self.option_value(CORE_OPTIONS, "experimental_propagate_custom_flag")?,
            OptionValue::Mixed(None)
        ) {
            return Err(unsupported("experimental_propagate_custom_flag"));
        }
        if !self.merge_genfiles_directory()? {
            return Err(unsupported("incompatible_merge_genfiles_directory"));
        }
        let compilation_mode = match self.option_value(CORE_OPTIONS, "compilation_mode")? {
            OptionValue::Native(NativeOccurrence::Scalar(NativeValue::Enum(value)))
                if value.family == EnumFamily::Compilation
                    && matches!(value.member.as_str(), "fastbuild" | "dbg" | "opt") =>
            {
                value.member.as_str()
            }
            _ => {
                return Err(SlugConfigurationError::UnexpectedMakeVariableOption {
                    option: "compilation_mode",
                });
            }
        };
        let defines = match self.option_value(CORE_OPTIONS, "define")? {
            OptionValue::Native(NativeOccurrence::Absent) => &[][..],
            OptionValue::Native(NativeOccurrence::List(values)) => values.0.as_ref(),
            _ => {
                return Err(SlugConfigurationError::UnexpectedMakeVariableOption {
                    option: "define",
                });
            }
        };
        if !defines
            .iter()
            .all(|entry| matches!(entry, NativeValue::Entry(_, _)))
        {
            return Err(SlugConfigurationError::UnexpectedMakeVariableOption { option: "define" });
        }
        Ok(MakeVariableConfigurationInputs {
            compilation_mode,
            defines,
        })
    }
}

/// Normalize the sole retained define option before configuration publication.
/// Index scratch borrows keys; only final surviving entries enter the existing
/// native option slice. No parallel semantic dictionary is retained.
pub(super) fn normalize_defines(options: &mut [OptionRecord]) {
    let Some(record) = options
        .iter_mut()
        .find(|record| record.class_name == CORE_OPTIONS && record.canonical_name == "define")
    else {
        return;
    };
    let OptionValue::Native(NativeOccurrence::List(values)) = &record.value else {
        return;
    };
    if values.len() < 2
        || !values
            .iter()
            .all(|value| matches!(value, NativeValue::Entry(_, _)))
    {
        return;
    }
    let key = |index: usize| match &values[index] {
        NativeValue::Entry(key, _) => key.as_str(),
        _ => unreachable!(),
    };
    let mut indices = (0..values.len()).collect::<Vec<_>>();
    // Equal keys put the last command occurrence first; dedup keeps that entry.
    indices.sort_unstable_by(|a, b| {
        key(*a)
            .encode_utf16()
            .cmp(key(*b).encode_utf16())
            .then_with(|| b.cmp(a))
    });
    indices.dedup_by(|a, b| key(*a) == key(*b));
    if indices.iter().copied().eq(0..values.len()) {
        return;
    }
    let normalized = indices
        .into_iter()
        .map(|index| values[index].clone())
        .collect::<Vec<_>>();
    record.value = OptionValue::Native(NativeOccurrence::List(NativeValues(normalized.into())));
}
