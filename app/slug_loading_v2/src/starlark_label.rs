/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::hash::Hash;

use allocative::Allocative;
use slug_identity_v2::CanonicalLabel;
use starlark::any::ProvidesStaticType;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;
use starlark_map::StarlarkHasher;

use crate::provider::BzlEvaluationContext;

/// One canonical Bazel Label value shared by loading and module-extension
/// evaluation. Repository/package/target identity lives only in this owner.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct StarlarkLabel(CanonicalLabel);

impl StarlarkLabel {
    pub(crate) fn new(label: CanonicalLabel) -> Self {
        Self(label)
    }

    pub(crate) fn canonical(&self) -> &CanonicalLabel {
        &self.0
    }
}

impl fmt::Display for StarlarkLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

starlark::starlark_simple_value!(StarlarkLabel);

#[starlark_value(type = "Label")]
impl<'v> StarlarkValue<'v> for StarlarkLabel {
    fn collect_str(&self, collector: &mut String) {
        collector.push_str(&self.0.to_string());
    }

    fn collect_repr(&self, collector: &mut String) {
        collector.push_str("Label(\"");
        collector.push_str(&self.0.to_string());
        collector.push_str("\")");
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.0.hash(hasher);
        Ok(())
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.0 == other.0))
    }

    fn get_attr(&self, name: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match name {
            "name" => Some(heap.alloc_str(self.0.target().as_str()).to_value()),
            "package" => Some(
                heap.alloc_str(self.0.package().package().as_str())
                    .to_value(),
            ),
            "repo_name" | "workspace_name" => {
                Some(heap.alloc_str(self.0.package().repo().as_str()).to_value())
            }
            _ => None,
        }
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(starlark_label_methods)
    }
}

#[starlark_module]
fn starlark_label_methods(builder: &mut MethodsBuilder) {
    fn same_package_label(this: Value, target_name: &str) -> anyhow::Result<StarlarkLabel> {
        let this = StarlarkLabel::from_value(this)
            .ok_or_else(|| anyhow::anyhow!("invalid Label receiver"))?;
        let label = CanonicalLabel::parse(&format!("{}:{}", this.0.package(), target_name))
            .map_err(anyhow::Error::msg)?;
        Ok(StarlarkLabel(label))
    }
}

fn resolve_label(raw: &str, defining_source: &CanonicalLabel) -> anyhow::Result<CanonicalLabel> {
    if raw.starts_with(':') {
        return CanonicalLabel::parse(&format!("{}{}", defining_source.package(), raw))
            .map_err(anyhow::Error::msg);
    }
    if !raw.starts_with("//") {
        anyhow::bail!("Label input must begin with '//' or ':' in the admitted loading slice");
    }
    let provisional = CanonicalLabel::parse(&format!("@@{raw}")).map_err(anyhow::Error::msg)?;
    let repo = defining_source.package().repo();
    if repo.is_root() {
        Ok(provisional)
    } else {
        provisional
            .rebind_provisional_root_repository(repo)
            .map_err(anyhow::Error::msg)
    }
}

#[starlark_module]
pub(crate) fn label_globals(builder: &mut GlobalsBuilder) {
    #[allow(non_snake_case)]
    fn Label<'v>(input: Value<'v>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<Value<'v>> {
        if StarlarkLabel::from_value(input).is_some() {
            return Ok(input);
        }
        let raw = input
            .unpack_str()
            .ok_or_else(|| anyhow::anyhow!("Label input must be a string or Label"))?;
        let source = BzlEvaluationContext::from_evaluator(eval)
            .map_err(|_| anyhow::anyhow!("Label() may only be called in a .bzl module"))?
            .source_label_for_call(eval)?;
        Ok(eval
            .heap()
            .alloc_simple(StarlarkLabel::new(resolve_label(raw, &source)?)))
    }
}
