//! Phase-local projections over DICE-prepared package/configuration/platform facts.

use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_loading_v2::cc_context::CcExecutionContext;
use slug_loading_v2::package_features::EffectiveFeatures;
use starlark::values::list::AllocList;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct CcContextProjection {
    configuration: Option<SlugConfiguration>,
    features: Option<EffectiveFeatures>,
    default_execution: Arc<ConfiguredActionOwnerContext>,
}

impl CcContextProjection {
    pub(super) fn owner_configuration(&self) -> Option<&SlugConfiguration> {
        self.configuration.as_ref()
    }
    pub(super) fn owner_features(&self) -> Option<&EffectiveFeatures> {
        self.features.as_ref()
    }
    pub(super) fn owner_default_execution(&self) -> &Arc<ConfiguredActionOwnerContext> {
        &self.default_execution
    }

    pub(super) fn coverage_instrumented(&self) -> anyhow::Result<bool> {
        self.require_disabled_coverage("ctx.coverage_instrumented")?;
        Ok(false)
    }

    pub(super) fn require_disabled_coverage(&self, operation: &str) -> anyhow::Result<()> {
        let configuration = self
            .configuration
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("{operation} requires structural configuration"))?;
        anyhow::ensure!(
            !configuration.coverage_enabled()?,
            "{operation} with coverage enabled is unsupported/deferred"
        );
        Ok(())
    }

    pub(super) fn merged_genfiles(&self) -> anyhow::Result<bool> {
        Ok(self
            .configuration
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ctx.genfiles_dir requires structural configuration"))?
            .merge_genfiles_directory()?)
    }
    #[cfg(test)]
    pub(super) fn unconfigured(default_execution: Arc<ConfiguredActionOwnerContext>) -> Self {
        Self {
            configuration: None,
            features: None,
            default_execution,
        }
    }

    pub(super) fn new(
        package: &LoadedPackage,
        configuration: Option<&SlugConfiguration>,
        attributes: &[ResolvedRuleAttribute],
        default_execution: Arc<ConfiguredActionOwnerContext>,
    ) -> Result<Self, String> {
        let features = configuration
            .map(|configuration| {
                let rule = match attributes
                    .iter()
                    .find(|attr| attr.declaration_name == "features")
                {
                    Some(ResolvedRuleAttribute {
                        value: CoercedAttributeValue::StringList(values),
                        ..
                    }) => values.as_ref(),
                    None => &[],
                    _ => {
                        return Err(
                            "ctx.features requires resolved string-list attributes".to_owned()
                        );
                    }
                };
                let global = configuration
                    .native_string_list_option(
                        "com.google.devtools.build.lib.analysis.config.CoreOptions",
                        "features",
                    )
                    .map_err(|error| error.to_string())?
                    .iter()
                    .map(CompactString::from)
                    .collect::<Vec<_>>();
                Ok(package.feature_defaults.resolve(rule, &global))
            })
            .transpose()?;
        Ok(Self {
            configuration: configuration.cloned(),
            features,
            default_execution,
        })
    }

    pub(super) fn features<'v>(&self, disabled: bool, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
        let features = self
            .features
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ctx.features requires structural configuration"))?;
        let values = if disabled {
            features.disabled()
        } else {
            features.enabled()
        };
        // Bazel converts its immutable Java lists to new mutable Starlark lists.
        Ok(heap.alloc(AllocList(values.iter().map(CompactString::as_str))))
    }

    pub(super) fn configuration<'v>(&self, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
        let configuration = self.configuration.as_ref().ok_or_else(|| {
            anyhow::anyhow!("ctx.configuration requires structural configuration")
        })?;
        slug_loading_v2::cc_compile_outputs::alloc_configuration(heap, configuration.clone())
    }

    fn execution_os(&self) -> anyhow::Result<&'static str> {
        selected_execution_os(&self.default_execution, self.configuration.as_ref())
            .map_err(anyhow::Error::msg)
    }
}

/// Bazel's selected-platform OS constraint, falling back to the retained Host
/// only when that platform has no standard OS constraint.
pub(crate) fn selected_execution_os(
    context: &ConfiguredActionOwnerContext,
    configuration: Option<&SlugConfiguration>,
) -> Result<&'static str, String> {
    if context.execution_platform().is_none() {
        return Err("exec_os requires a resolved default execution platform".to_owned());
    }
    let standard = context
        .platform_constraints()
        .iter()
        .find(|constraint| standard_os_label(constraint.constraint_setting().label(), "os"));
    if let Some(constraint) = standard {
        let value = constraint.constraint_value().label();
        for (name, os) in [
            ("linux", "LINUX"),
            ("osx", "DARWIN"),
            ("macos", "DARWIN"),
            ("windows", "WINDOWS"),
            ("freebsd", "FREEBSD"),
            ("openbsd", "OPENBSD"),
        ] {
            if standard_os_label(value, name) {
                return Ok(os);
            }
        }
        return Ok("UNKNOWN");
    }
    let host = configuration
        .ok_or_else(|| "exec_os Host fallback requires structural configuration".to_owned())?
        .execution_host_os()
        .map_err(|error| error.to_string())?;
    Ok(match host {
        ActionEnvironmentHostOs::Linux => "LINUX",
        ActionEnvironmentHostOs::Windows => "WINDOWS",
        ActionEnvironmentHostOs::Macos => "DARWIN",
        ActionEnvironmentHostOs::Freebsd => "FREEBSD",
        ActionEnvironmentHostOs::Openbsd => "OPENBSD",
        ActionEnvironmentHostOs::Unknown => "UNKNOWN",
    })
}

fn standard_os_label(label: &slug_identity_v2::CanonicalLabel, target: &str) -> bool {
    label.package().repo().as_str() == "platforms"
        && label.package().package().as_str() == "os"
        && label.target().as_str() == target
}

impl<'v, V: ValueLike<'v>> CcExecutionContext for AnalysisContextGen<V> {
    fn execution_os(&self) -> anyhow::Result<&'static str> {
        self.token.require_active("exec_os", "rule context")?;
        self.cc_context.execution_os()
    }
}

impl<'v, V: ValueLike<'v>> slug_loading_v2::cc_context::CcCompileOutputContext
    for AnalysisContextGen<V>
{
    fn declare_compile_output(
        &self,
        label: &slug_identity_v2::CanonicalLabel,
        output_name: &str,
        configuration: &SlugConfiguration,
    ) -> anyhow::Result<AnalysisArtifactValue> {
        self.token
            .require_active("declare_compile_output_file", "rule context")?;
        anyhow::ensure!(
            configuration == self.output_root.configuration(),
            "compile output configuration must equal the active ctx output root configuration"
        );
        let path = slug_loading_v2::cc_compile_outputs::compile_output_path(
            &self.target_label,
            label,
            output_name,
        )?;
        self.action_sink.declare_compile_output(path)
    }
}

#[cfg(test)]
mod tests;
