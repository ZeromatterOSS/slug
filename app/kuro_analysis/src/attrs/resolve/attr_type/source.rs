/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::Dupe;
use kuro_artifact::artifact::source_artifact::SourceArtifact;
use kuro_build_api::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use kuro_build_api::interpreter::rule_defs::provider::dependency::SourceFileTarget;
use kuro_core::configuration::pair::Configuration;
use kuro_core::package::source_path::SourcePath;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::provider::label::ProvidersName;
use kuro_core::target::label::label::TargetLabel;
use kuro_core::target::name::TargetNameRef;
use kuro_node::attrs::attr_type::source::SourceAttrType;
use starlark::values::Value;
use starlark::values::list::ListRef;

use crate::attrs::resolve::ctx::AttrResolutionContext;

#[derive(kuro_error::Error, Debug)]
#[kuro(tag = Input)]
enum SourceLabelResolutionError {
    #[error("Expected a single artifact from {0}, but it returned {1} artifacts")]
    ExpectedSingleValue(String, usize),
}

pub(crate) trait SourceAttrTypeExt {
    fn resolve_single_file<'v>(
        ctx: &mut dyn AttrResolutionContext<'v>,
        path: SourcePath,
    ) -> Value<'v> {
        ctx.heap()
            .alloc(StarlarkArtifact::new(SourceArtifact::new(path).into()))
    }

    fn resolve_single_file_target<'v>(
        ctx: &mut dyn AttrResolutionContext<'v>,
        path: SourcePath,
        cfg_pair: &Configuration,
    ) -> kuro_error::Result<Value<'v>> {
        let target_name = TargetNameRef::new(path.path().as_str())?;
        let target = TargetLabel::new(path.package(), target_name).configure_pair(cfg_pair.dupe());
        let label = ConfiguredProvidersLabel::new(target, ProvidersName::Default);
        let artifact = StarlarkArtifact::new(SourceArtifact::new(path).into());
        Ok(ctx.heap().alloc(SourceFileTarget::new(label, artifact)))
    }

    fn resolve_label<'v>(
        ctx: &mut dyn AttrResolutionContext<'v>,
        label: &ConfiguredProvidersLabel,
    ) -> kuro_error::Result<Vec<Value<'v>>> {
        let dep = ctx.get_dep(label)?;
        let default_outputs = dep.default_info()?.default_outputs_raw();
        let res = ListRef::from_frozen_value(default_outputs)
            .unwrap()
            .iter()
            .collect();
        Ok(res)
    }

    fn resolve_single_label<'v>(
        ctx: &mut dyn AttrResolutionContext<'v>,
        value: &ConfiguredProvidersLabel,
    ) -> kuro_error::Result<Value<'v>> {
        let mut resolved = Self::resolve_label(ctx, value)?;
        if resolved.len() == 1 {
            Ok(resolved.pop().unwrap())
        } else {
            Err(
                SourceLabelResolutionError::ExpectedSingleValue(value.to_string(), resolved.len())
                    .into(),
            )
        }
    }
}

impl SourceAttrTypeExt for SourceAttrType {}
