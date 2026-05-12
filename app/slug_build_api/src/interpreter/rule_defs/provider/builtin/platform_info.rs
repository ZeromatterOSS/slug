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
use slug_build_api_derive::internal_provider;
use slug_core::configuration::build_setting::BuildSettingLabel;
use slug_core::configuration::build_setting::BuildSettingValue;
use slug_core::configuration::data::ConfigurationData;
use slug_core::provider::label::ProvidersLabel;
use slug_core::target::label::label::TargetLabel;
use starlark::any::ProvidesStaticType;
use starlark::coerce::Coerce;
use starlark::environment::GlobalsBuilder;
use starlark::values::Freeze;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::FrozenValueOfUnchecked;
use starlark::values::Heap;
use starlark::values::StringValue;
use starlark::values::Trace;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::ValueOf;
use starlark::values::ValueOfUnchecked;
use starlark::values::ValueOfUncheckedGeneric;
use starlark::values::dict::AllocDict;
use starlark::values::dict::DictRef;
use starlark::values::dict::DictType;

use crate as slug_build_api;
use crate::interpreter::rule_defs::provider::builtin::configuration_info::ConfigurationInfo;
use crate::interpreter::rule_defs::provider::builtin::configuration_info::FrozenConfigurationInfo;

#[internal_provider(platform_info_creator)]
#[derive(Clone, Debug, Trace, Coerce, Freeze, ProvidesStaticType, Allocative)]
#[repr(C)]
pub struct PlatformInfoGen<V: ValueLifetimeless> {
    label: ValueOfUncheckedGeneric<V, String>,
    configuration: ValueOfUncheckedGeneric<V, FrozenConfigurationInfo>,
    /// Bazel `platform(exec_properties={...})`. When this platform is used as
    /// an execution platform, each entry is applied to the resulting
    /// `ConfigurationData.build_settings` — this is how rules_cc declares its
    /// opt-mode default for exec-config tool builds without slug hardcoding it.
    exec_properties: ValueOfUncheckedGeneric<V, DictType<String, String>>,
}

impl<'v, V: ValueLike<'v>> PlatformInfoGen<V> {
    pub fn to_configuration(&self) -> slug_error::Result<ConfigurationData> {
        let label = self
            .label
            .to_value()
            .get()
            .unpack_str()
            .expect("type checked during construction")
            .to_owned();
        let mut data = ConfigurationInfo::from_value(self.configuration.get().to_value())
            .expect("type checked during construction")
            .to_configuration_data()?;
        // `exec_properties` carries opaque-key remote-execution metadata
        // (e.g. `OSFamily`, `Arch`, `container-image`); the keys are
        // arbitrary strings, NOT Bazel labels. Bazel passes these
        // through to the executor unchanged. Conflating them with
        // build settings here errors on every platform that uses
        // RBE-style exec_properties, which `toolchains_buildbuddy`
        // does. Only entries whose key parses as a real label belong
        // in build_settings; everything else is exec metadata and is
        // applied via the platform's `exec_properties` field, not its
        // configuration.
        for (k, v) in self.exec_properties_entries() {
            if let Ok(key) = BuildSettingLabel::from_bazel_label(&k) {
                data.build_settings
                    .insert(key, BuildSettingValue::String(v));
            }
        }
        ConfigurationData::from_platform(label, data)
    }

    /// Returns the exec_properties entries as (label, value) pairs. Empty for
    /// `PlatformInfo` instances that did not set exec_properties.
    pub fn exec_properties_entries(&self) -> Vec<(String, String)> {
        match DictRef::from_value(self.exec_properties.get().to_value()) {
            Some(dict) => dict
                .iter()
                .filter_map(|(k, v)| Some((k.unpack_str()?.to_owned(), v.unpack_str()?.to_owned())))
                .collect(),
            None => Vec::new(),
        }
    }
}

impl<'v> PlatformInfo<'v> {
    pub fn from_configuration(
        cfg: &ConfigurationData,
        heap: Heap<'v>,
    ) -> slug_error::Result<PlatformInfo<'v>> {
        let label = heap.alloc_str(cfg.label()?);
        let configuration = heap.alloc(ConfigurationInfo::from_configuration_data(
            cfg.data()?,
            heap,
        ));
        // Round-tripping a ConfigurationData through `PlatformInfo` is lossy
        // for exec_properties: the dict is not stored on ConfigurationInfo.
        // The cfg's build_settings remain authoritative for consumers that
        // read `ConfigurationData.build_settings` directly (e.g. `ctx.var`
        // in a later phase). This is acceptable because `from_configuration`
        // is used by transition machinery that already carries the cfg.
        let exec_properties = heap.alloc(AllocDict(Vec::<(&str, &str)>::new()));
        Ok(PlatformInfoGen {
            label: label.to_value_of_unchecked().cast(),
            configuration: ValueOfUnchecked::<FrozenConfigurationInfo>::new(configuration),
            exec_properties: ValueOfUnchecked::<DictType<String, String>>::new(exec_properties),
        })
    }
}

impl FrozenPlatformInfo {
    /// Create a frozen PlatformInfo for a native `platform()` rule.
    ///
    /// `exec_properties` is applied to the produced `ConfigurationData`'s
    /// `build_settings` when the platform is used as an execution platform.
    pub fn for_native_platform(
        label_str: &str,
        constraint_pairs: &[(TargetLabel, ProvidersLabel)],
        exec_properties: &[(String, String)],
        heap: &FrozenHeap,
    ) -> FrozenValue {
        let label_frozen = heap.alloc_str(label_str).to_frozen_value();
        let config_info =
            FrozenConfigurationInfo::for_native_config_setting(constraint_pairs, heap);
        let exec_properties_frozen = heap.alloc(AllocDict(
            exec_properties
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str())),
        ));
        heap.alloc(PlatformInfoGen::<FrozenValue> {
            label: FrozenValueOfUnchecked::new(label_frozen),
            configuration: FrozenValueOfUnchecked::new(config_info),
            exec_properties: FrozenValueOfUnchecked::new(exec_properties_frozen),
        })
    }
}

#[starlark_module]
fn platform_info_creator(globals: &mut GlobalsBuilder) {
    #[starlark(as_type = FrozenPlatformInfo)]
    fn PlatformInfo<'v>(
        #[starlark(require = named)] label: StringValue<'v>,
        #[starlark(require = named)] configuration: ValueOf<'v, &'v ConfigurationInfo<'v>>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        exec_properties: starlark::values::Value<'v>,
        eval: &mut starlark::eval::Evaluator<'v, '_, '_>,
    ) -> starlark::Result<PlatformInfo<'v>> {
        let exec_properties_value = if exec_properties.is_none() {
            eval.heap().alloc(AllocDict(Vec::<(&str, &str)>::new()))
        } else {
            exec_properties
        };
        Ok(PlatformInfo {
            label: label.to_value_of_unchecked().cast(),
            configuration: ValueOfUnchecked::<FrozenConfigurationInfo>::new(configuration.value),
            exec_properties: ValueOfUnchecked::<DictType<String, String>>::new(
                exec_properties_value,
            ),
        })
    }
}
