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
use kuro_build_api_derive::internal_provider;
use kuro_core::configuration::data::ConfigurationData;
use kuro_core::provider::label::ProvidersLabel;
use kuro_core::target::label::label::TargetLabel;
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

use crate as kuro_build_api;
use crate::interpreter::rule_defs::provider::builtin::configuration_info::ConfigurationInfo;
use crate::interpreter::rule_defs::provider::builtin::configuration_info::FrozenConfigurationInfo;

#[internal_provider(platform_info_creator)]
#[derive(Clone, Debug, Trace, Coerce, Freeze, ProvidesStaticType, Allocative)]
#[repr(C)]
pub struct PlatformInfoGen<V: ValueLifetimeless> {
    label: ValueOfUncheckedGeneric<V, String>,
    configuration: ValueOfUncheckedGeneric<V, FrozenConfigurationInfo>,
}

impl<'v, V: ValueLike<'v>> PlatformInfoGen<V> {
    pub fn to_configuration(&self) -> kuro_error::Result<ConfigurationData> {
        ConfigurationData::from_platform(
            self.label
                .to_value()
                .get()
                .unpack_str()
                .expect("type checked during construction")
                .to_owned(),
            ConfigurationInfo::from_value(self.configuration.get().to_value())
                .expect("type checked during construction")
                .to_configuration_data()?,
        )
    }
}

impl<'v> PlatformInfo<'v> {
    pub fn from_configuration(
        cfg: &ConfigurationData,
        heap: Heap<'v>,
    ) -> kuro_error::Result<PlatformInfo<'v>> {
        let label = heap.alloc_str(cfg.label()?);
        let configuration = heap.alloc(ConfigurationInfo::from_configuration_data(
            cfg.data()?,
            heap,
        ));
        Ok(PlatformInfoGen {
            label: label.to_value_of_unchecked().cast(),
            configuration: ValueOfUnchecked::<FrozenConfigurationInfo>::new(configuration),
        })
    }
}

impl FrozenPlatformInfo {
    /// Create a frozen PlatformInfo for a native `platform()` rule.
    ///
    /// Takes the platform label string and constraint pairs collected from
    /// `constraint_values` deps (and parent platforms), and produces a frozen
    /// `PlatformInfo` value ready for inclusion in a provider collection.
    pub fn for_native_platform(
        label_str: &str,
        constraint_pairs: &[(TargetLabel, ProvidersLabel)],
        heap: &FrozenHeap,
    ) -> FrozenValue {
        let label_frozen = heap.alloc_str(label_str).to_frozen_value();
        let config_info =
            FrozenConfigurationInfo::for_native_config_setting(constraint_pairs, heap);
        heap.alloc(PlatformInfoGen::<FrozenValue> {
            label: FrozenValueOfUnchecked::new(label_frozen),
            configuration: FrozenValueOfUnchecked::new(config_info),
        })
    }
}

#[starlark_module]
fn platform_info_creator(globals: &mut GlobalsBuilder) {
    #[starlark(as_type = FrozenPlatformInfo)]
    fn PlatformInfo<'v>(
        #[starlark(require = named)] label: StringValue<'v>,
        #[starlark(require = named)] configuration: ValueOf<'v, &'v ConfigurationInfo<'v>>,
    ) -> starlark::Result<PlatformInfo<'v>> {
        Ok(PlatformInfo {
            label: label.to_value_of_unchecked().cast(),
            configuration: ValueOfUnchecked::<FrozenConfigurationInfo>::new(configuration.value),
        })
    }
}
