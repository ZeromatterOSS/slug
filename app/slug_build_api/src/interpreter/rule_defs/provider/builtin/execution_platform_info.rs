/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt::Debug;

use allocative::Allocative;
use dupe::Dupe;
use slug_build_api_derive::internal_provider;
use slug_core::configuration::build_setting::BuildSettingLabel;
use slug_core::configuration::build_setting::BuildSettingValue;
use slug_core::configuration::constraints::ConstraintKey;
use slug_core::configuration::constraints::ConstraintValue;
use slug_core::configuration::data::ConfigurationData;
use slug_core::execution_types::execution::ExecutionPlatform;
use slug_core::target::label::label::TargetLabel;
use slug_interpreter::types::target_label::StarlarkTargetLabel;
use starlark::any::ProvidesStaticType;
use starlark::coerce::Coerce;
use starlark::environment::GlobalsBuilder;
use starlark::values::Freeze;
use starlark::values::Trace;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::ValueOfUnchecked;
use starlark::values::ValueOfUncheckedGeneric;
use starlark::values::ValueTyped;
use starlark::values::ValueTypedComplex;
use starlark::values::dict::AllocDict;
use starlark::values::dict::DictRef;
use starlark::values::dict::DictType;

use crate as slug_build_api;
use crate::interpreter::rule_defs::command_executor_config::StarlarkCommandExecutorConfig;
use crate::interpreter::rule_defs::provider::builtin::configuration_info::ConfigurationInfo;
use crate::interpreter::rule_defs::provider::builtin::configuration_info::FrozenConfigurationInfo;

#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
enum ExecutionPlatformProviderErrors {
    #[error("expected a ConfigurationInfo, got `{0}` (type `{1}`)")]
    ExpectedConfigurationInfo(String, String),
    #[error("expected a CommandExecutorConfig, got `{0}` (type `{1}`)")]
    ExpectedCommandExecutorConfig(String, String),
}

/// Provider that signals that a target represents an execution platform.
#[internal_provider(info_creator)]
#[derive(Clone, Debug, Trace, Coerce, Freeze, ProvidesStaticType, Allocative)]
#[repr(C)]
pub struct ExecutionPlatformInfoGen<V: ValueLifetimeless> {
    /// label of the defining rule, used in informative messages
    label: ValueOfUncheckedGeneric<V, StarlarkTargetLabel>,
    /// The configuration of the execution platform
    configuration: ValueOfUncheckedGeneric<V, FrozenConfigurationInfo>,
    /// The executor config
    executor_config: ValueOfUncheckedGeneric<V, StarlarkCommandExecutorConfig>,
    /// `platform(exec_properties={...})` entries forwarded from the
    /// `platform` dep. Applied to the produced `ConfigurationData`'s
    /// `build_settings` (as `BuildSettingValue::String`) so rule-authored
    /// defaults like `@bazel_tools//tools/cpp:compilation_mode = "opt"`
    /// flow to exec-configured targets without slug hardcoding anything.
    exec_properties: ValueOfUncheckedGeneric<V, DictType<String, String>>,
}

impl FrozenExecutionPlatformInfo {
    /// Create a FrozenExecutionPlatformInfo for use in native execution_platform analysis.
    ///
    /// Builds an ExecutionPlatformInfo from a target label, constraint pairs,
    /// exec_properties (see the field docs), and a local executor.
    pub fn for_native_execution_platform(
        label: slug_core::target::label::label::TargetLabel,
        constraint_pairs: &[(
            slug_core::target::label::label::TargetLabel,
            slug_core::provider::label::ProvidersLabel,
        )],
        exec_properties: &[(String, String)],
        heap: &starlark::values::FrozenHeap,
    ) -> starlark::values::FrozenValue {
        use std::sync::Arc;

        use slug_core::execution_types::executor_config::CacheUploadBehavior;
        use slug_core::execution_types::executor_config::CommandExecutorConfig;
        use slug_core::execution_types::executor_config::CommandGenerationOptions;
        use slug_core::execution_types::executor_config::Executor;
        use slug_core::execution_types::executor_config::HybridExecutionLevel;
        use slug_core::execution_types::executor_config::LocalExecutorOptions;
        use slug_core::execution_types::executor_config::MetaInternalExtraParams;
        use slug_core::execution_types::executor_config::PathSeparatorKind;
        use slug_core::execution_types::executor_config::RePlatformFields;
        use slug_core::execution_types::executor_config::RemoteEnabledExecutor;
        use slug_core::execution_types::executor_config::RemoteEnabledExecutorOptions;
        use slug_core::execution_types::executor_config::RemoteExecutorOptions;
        use slug_core::execution_types::executor_config::RemoteExecutorUseCase;
        use starlark::values::FrozenValueOfUnchecked;

        use crate::interpreter::rule_defs::provider::builtin::configuration_info::FrozenConfigurationInfo;

        let label_value = heap.alloc(StarlarkTargetLabel::new(label));
        let config_value =
            FrozenConfigurationInfo::for_native_config_setting(constraint_pairs, heap);

        // When the platform carries `exec_properties` the user has
        // signaled that this is a remote-execution platform — Bazel's
        // convention is `OSFamily=…`, `Arch=…`, `container-image=…`,
        // etc. Materialize them into the platform's
        // `CommandExecutorConfig.re_properties` so an action configured
        // under this exec platform sends the right RE `Platform`
        // message at upload time. The executor shape is `Hybrid`
        // (Limited) so daemon-side wiring still falls back to local for
        // workspaces without a configured remote backend; the daemon's
        // executor factory checks whether RE is configured before
        // dispatching to remote.
        //
        // Without `exec_properties`, fall back to `testing_local()` to
        // preserve the previous behavior for `:host` / generic
        // platforms that don't intend remote execution.
        let exec_config = if exec_properties.is_empty() {
            CommandExecutorConfig::testing_local()
        } else {
            let re_properties = RePlatformFields {
                properties: Arc::new(
                    exec_properties
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                ),
            };
            Arc::new(CommandExecutorConfig {
                executor: Executor::RemoteEnabled(RemoteEnabledExecutorOptions {
                    executor: RemoteEnabledExecutor::Hybrid {
                        local: LocalExecutorOptions::default(),
                        remote: RemoteExecutorOptions::default(),
                        level: HybridExecutionLevel::Limited,
                    },
                    re_properties,
                    re_use_case: RemoteExecutorUseCase::slug_default(),
                    re_action_key: None,
                    cache_upload_behavior: CacheUploadBehavior::Disabled,
                    remote_cache_enabled: true,
                    remote_dep_file_cache_enabled: false,
                    dependencies: Vec::new(),
                    custom_image: None,
                    meta_internal_extra_params: MetaInternalExtraParams::default(),
                }),
                options: CommandGenerationOptions {
                    path_separator: PathSeparatorKind::system_default(),
                    output_paths_behavior: Default::default(),
                    use_bazel_protocol_remote_persistent_workers: false,
                },
            })
        };
        let exec_config_value = heap.alloc(StarlarkCommandExecutorConfig(exec_config));
        let exec_properties_value = heap.alloc(AllocDict(
            exec_properties
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str())),
        ));

        heap.alloc(ExecutionPlatformInfoGen::<starlark::values::FrozenValue> {
            label: FrozenValueOfUnchecked::new(label_value),
            configuration: FrozenValueOfUnchecked::new(config_value),
            executor_config: FrozenValueOfUnchecked::new(exec_config_value),
            exec_properties: FrozenValueOfUnchecked::new(exec_properties_value),
        })
    }
}

impl<'v, V: ValueLike<'v>> ExecutionPlatformInfoGen<V> {
    pub fn to_execution_platform(&self) -> slug_error::Result<ExecutionPlatform> {
        self.to_execution_platform_with_marker(None)
    }

    /// Convert to an ExecutionPlatform, optionally adding a marker constraint to the configuration.
    pub fn to_execution_platform_with_marker(
        &self,
        marker_constraint: Option<&(ConstraintKey, ConstraintValue)>,
    ) -> slug_error::Result<ExecutionPlatform> {
        let target = self.label.cast::<&StarlarkTargetLabel>().unpack()?.label();
        let mut cfg = ConfigurationInfo::from_value(self.configuration.get().to_value())
            .ok_or_else(|| {
                ExecutionPlatformProviderErrors::ExpectedConfigurationInfo(
                    self.configuration.to_value().get().to_repr(),
                    self.configuration.to_value().get().get_type().to_owned(),
                )
            })?
            .to_configuration_data()?;

        // Apply exec_properties as build_settings on the exec platform's cfg.
        if let Some(dict) = DictRef::from_value(self.exec_properties.get().to_value()) {
            for (k, v) in dict.iter() {
                if let (Some(k_str), Some(v_str)) = (k.unpack_str(), v.unpack_str()) {
                    let label = BuildSettingLabel::from_bazel_label(k_str)?;
                    cfg.build_settings
                        .insert(label, BuildSettingValue::String(v_str.to_owned()));
                }
            }
        }

        // Add the marker constraint if provided
        if let Some((key, value)) = marker_constraint {
            cfg.constraints.insert(key.clone(), value.clone());
        }

        let cfg = ConfigurationData::from_platform(TargetLabel::to_string(target), cfg)?;
        let executor_config =
            StarlarkCommandExecutorConfig::from_value(self.executor_config.get().to_value())
                .ok_or_else(|| {
                    ExecutionPlatformProviderErrors::ExpectedCommandExecutorConfig(
                        self.configuration.get().to_value().to_repr(),
                        self.configuration.get().to_value().get_type().to_owned(),
                    )
                })?
                .0
                .dupe();
        Ok(ExecutionPlatform::platform(
            target.dupe(),
            cfg,
            executor_config,
        ))
    }
}

#[starlark_module]
fn info_creator(globals: &mut GlobalsBuilder) {
    fn ExecutionPlatformInfo<'v>(
        #[starlark(require = named)] label: ValueTyped<'v, StarlarkTargetLabel>,
        #[starlark(require = named)] configuration: ValueTypedComplex<'v, ConfigurationInfo<'v>>,
        #[starlark(require = named)] executor_config: ValueTyped<'v, StarlarkCommandExecutorConfig>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        exec_properties: starlark::values::Value<'v>,
        eval: &mut starlark::eval::Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ExecutionPlatformInfo<'v>> {
        let exec_properties_value = if exec_properties.is_none() {
            eval.heap().alloc(AllocDict(Vec::<(&str, &str)>::new()))
        } else {
            exec_properties
        };
        let info = ExecutionPlatformInfo {
            label: label.to_value_of_unchecked(),
            configuration: ValueOfUnchecked::new(configuration.to_value()),
            executor_config: executor_config.to_value_of_unchecked(),
            exec_properties: ValueOfUnchecked::<DictType<String, String>>::new(
                exec_properties_value,
            ),
        };
        // This checks that the values are valid.
        info.to_execution_platform()?;
        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use slug_core::configuration::build_setting::BuildSettingLabel;
    use slug_core::configuration::build_setting::BuildSettingValue;
    use slug_core::target::label::label::TargetLabel;
    use starlark::values::FrozenHeap;

    use super::*;

    /// `platform(exec_properties=...)` declared defaults flow into the
    /// resulting `ExecutionPlatform`'s `ConfigurationData.build_settings`.
    #[test]
    fn exec_properties_land_in_build_settings() -> slug_error::Result<()> {
        let heap = FrozenHeap::new();
        let label = TargetLabel::testing_parse("@root//:exec");
        let exec_properties = vec![
            (
                "@bazel_tools//tools/cpp:compilation_mode".to_owned(),
                "opt".to_owned(),
            ),
            ("//:my_flag".to_owned(), "baz".to_owned()),
        ];
        let frozen_value = FrozenExecutionPlatformInfo::for_native_execution_platform(
            label.dupe(),
            &[],
            &exec_properties,
            &heap,
        );
        let info = frozen_value
            .downcast_frozen_ref::<FrozenExecutionPlatformInfo>()
            .expect("downcast to FrozenExecutionPlatformInfo");

        let platform = info.to_execution_platform()?;
        let cfg = platform.cfg();

        let compile_mode_label =
            BuildSettingLabel::from_bazel_label("@bazel_tools//tools/cpp:compilation_mode")?;
        assert_eq!(
            cfg.get_build_setting(&compile_mode_label)?,
            Some(&BuildSettingValue::String("opt".to_owned()))
        );
        let my_flag_label = BuildSettingLabel::from_bazel_label("//:my_flag")?;
        assert_eq!(
            cfg.get_build_setting(&my_flag_label)?,
            Some(&BuildSettingValue::String("baz".to_owned()))
        );
        Ok(())
    }

    /// Two exec platforms that share constraints but differ only in
    /// `exec_properties` must produce distinct `ConfigurationData`s (different
    /// build-settings → different output hash).
    #[test]
    fn differing_exec_properties_produce_distinct_cfgs() -> slug_error::Result<()> {
        fn build(value: &str) -> slug_error::Result<ConfigurationData> {
            let heap = FrozenHeap::new();
            let label = TargetLabel::testing_parse("@root//:exec");
            let frozen = FrozenExecutionPlatformInfo::for_native_execution_platform(
                label,
                &[],
                &[(
                    "@bazel_tools//tools/cpp:compilation_mode".to_owned(),
                    value.to_owned(),
                )],
                &heap,
            );
            let info = frozen
                .downcast_frozen_ref::<FrozenExecutionPlatformInfo>()
                .expect("downcast");
            Ok(info.to_execution_platform()?.cfg().dupe())
        }

        let opt = build("opt")?;
        let dbg = build("dbg")?;
        assert_ne!(opt.output_hash(), dbg.output_hash());
        Ok(())
    }
}
