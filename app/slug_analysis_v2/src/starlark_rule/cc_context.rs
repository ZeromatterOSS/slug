//! Phase-local projections over DICE-prepared package/configuration/platform facts.

use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::ActionEnvironmentHostOs;
use slug_loading_v2::cc_context::CcExecutionContext;
use slug_loading_v2::package_features::EffectiveFeatures;
use starlark::values::list::AllocList;

use super::*;

#[derive(Debug, Clone, Allocative)]
pub(super) struct CcContextProjection {
    configuration: Option<SlugConfiguration>,
    features: Option<EffectiveFeatures>,
    default_execution: Arc<ConfiguredActionOwnerContext>,
}

impl CcContextProjection {
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
        Ok(heap.alloc_simple(ConfigurationValue {
            coverage_enabled: configuration.coverage_enabled()?,
        }))
    }

    fn execution_os(&self) -> anyhow::Result<&'static str> {
        if self.default_execution.execution_platform().is_none() {
            anyhow::bail!("exec_os requires a resolved default execution platform");
        }
        let standard = self
            .default_execution
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
        let host = self
            .configuration
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!("exec_os Host fallback requires structural configuration")
            })?
            .execution_host_os()?;
        Ok(match host {
            ActionEnvironmentHostOs::Linux => "LINUX",
            ActionEnvironmentHostOs::Windows => "WINDOWS",
            ActionEnvironmentHostOs::Macos => "DARWIN",
            ActionEnvironmentHostOs::Freebsd => "FREEBSD",
            ActionEnvironmentHostOs::Openbsd => "OPENBSD",
            ActionEnvironmentHostOs::Unknown => "UNKNOWN",
        })
    }
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

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ConfigurationValue {
    coverage_enabled: bool,
}

impl fmt::Display for ConfigurationValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<configuration>")
    }
}

starlark::starlark_simple_value!(ConfigurationValue);

#[starlark_value(type = "configuration")]
impl<'v> StarlarkValue<'v> for ConfigurationValue {
    fn get_attr(&self, name: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "coverage_enabled" => Some(Value::new_bool(self.coverage_enabled)),
            _ => None,
        }
    }
    fn dir_attr(&self) -> Vec<String> {
        vec!["coverage_enabled".to_owned()]
    }
}

#[cfg(test)]
mod tests;
