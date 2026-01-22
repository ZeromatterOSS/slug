/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::RefMut;
use std::collections::HashMap;
use std::sync::Arc;

use allocative::Allocative;
use kuro_analysis::analysis::calculation::get_dep_analysis;
use kuro_analysis::analysis::calculation::resolve_queries;
use kuro_analysis::analysis::env::get_dep;
use kuro_analysis::analysis::env::get_deps_from_analysis_results;
use kuro_analysis::analysis::env::resolve_query;
use kuro_analysis::analysis::env::resolve_unkeyed_placeholder;
use kuro_analysis::attrs::resolve::ctx::AnalysisQueryResult;
use kuro_analysis::attrs::resolve::ctx::AttrResolutionContext;
use kuro_build_api::interpreter::rule_defs::cmd_args::value::FrozenCommandLineArg;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollection;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;
use kuro_core::execution_types::execution::ExecutionPlatformResolution;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_node::nodes::configured::ConfiguredTargetNode;
use futures::FutureExt;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::FrozenValueTyped;

use crate::bxl::starlark_defs::context::BxlContext;

#[derive(Allocative)]
pub(crate) struct LazyAttrResolutionCache {
    pub(super) dep_analysis_results:
        Option<HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>>,
    pub(super) query_results: Option<HashMap<String, Arc<AnalysisQueryResult>>>,
}

// Contains a `module` that things must live on, and various `FrozenProviderCollectionValue`s
// that are NOT tied to that module. Must claim ownership of them via `add_reference` before returning them.
pub(crate) struct LazyAttrResolutionContext<'v, 'a, 'e, 'c> {
    pub(crate) eval: &'c mut Evaluator<'v, 'a, 'e>,
    pub(super) configured_node: &'v ConfiguredTargetNode,
    pub(super) ctx: &'v BxlContext<'v>,
    pub(crate) cache: RefMut<'c, LazyAttrResolutionCache>,
}

fn get_or_try_init<T>(
    o: &mut Option<T>,
    f: impl FnOnce() -> kuro_error::Result<T>,
) -> kuro_error::Result<&T> {
    if o.is_none() {
        *o = Some(f()?);
    }

    Ok(o.as_ref().unwrap())
}

impl LazyAttrResolutionCache {
    fn dep_analysis_results<'v>(
        &mut self,
        ctx: &'v BxlContext<'v>,
        configured_node: &'v ConfiguredTargetNode,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<&HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>> {
        get_or_try_init(&mut self.dep_analysis_results, || {
            get_deps_from_analysis_results(ctx.via_dice(eval, |ctx| {
                ctx.via(|dice_ctx| {
                    get_dep_analysis(configured_node.as_ref(), dice_ctx).boxed_local()
                })
            })?)
        })
    }

    fn query_results<'v>(
        &mut self,
        ctx: &'v BxlContext<'v>,
        configured_node: &'v ConfiguredTargetNode,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<&HashMap<String, Arc<AnalysisQueryResult>>> {
        get_or_try_init(&mut self.query_results, || {
            ctx.via_dice(eval, |ctx| {
                ctx.via(|dice_ctx| {
                    resolve_queries(dice_ctx, configured_node.as_ref()).boxed_local()
                })
            })
        })
    }
}

impl<'v, 'a, 'e, 'c> AttrResolutionContext<'v> for LazyAttrResolutionContext<'v, 'a, 'e, 'c> {
    fn starlark_module(&self) -> &'v Module {
        self.eval.module()
    }

    fn get_dep(
        &mut self,
        target: &ConfiguredProvidersLabel,
    ) -> kuro_error::Result<FrozenValueTyped<'v, FrozenProviderCollection>> {
        let module = self.eval.module();
        match self
            .cache
            .dep_analysis_results(self.ctx, self.configured_node, self.eval)
        {
            Ok(deps) => Ok(get_dep(deps, target, module)?),
            Err(e) => Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Bxl,
                "Error getting deps from analysis: `{}`",
                e
            )),
        }
    }

    fn resolve_unkeyed_placeholder(
        &mut self,
        name: &str,
    ) -> kuro_error::Result<Option<FrozenCommandLineArg>> {
        let module = self.eval.module();
        match self
            .cache
            .dep_analysis_results(self.ctx, self.configured_node, self.eval)
        {
            Ok(deps) => Ok(resolve_unkeyed_placeholder(deps, name, module)),
            Err(e) => Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Bxl,
                "Error resolving unkeyed placeholder: `{}`",
                e
            )),
        }
    }

    fn resolve_query(&mut self, query: &str) -> kuro_error::Result<Arc<AnalysisQueryResult>> {
        let module = self.eval.module();
        match self
            .cache
            .query_results(self.ctx, self.configured_node, self.eval)
        {
            Ok(res) => resolve_query(res, query, module),
            Err(e) => Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Bxl,
                "Error resolving query: `{}`",
                e
            ))
            .map_err(kuro_error::Error::from),
        }
    }

    fn execution_platform_resolution(&self) -> &ExecutionPlatformResolution {
        self.configured_node.execution_platform_resolution()
    }
}
