/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use async_trait::async_trait;
use dice::DiceComputations;
use slug_core::fs::project_rel_path::ProjectRelativePath;
use slug_core::global_cfg_options::GlobalCfgOptions;
use slug_node::configured_universe::CqueryUniverse;
use slug_node::nodes::configured::ConfiguredTargetNode;
use slug_node::nodes::unconfigured::TargetNode;
use slug_query::query::syntax::simple::eval::values::QueryEvaluationResult;
use slug_util::late_binding::LateBinding;

use crate::actions::query::ActionQueryNode;

#[async_trait]
pub trait QueryFrontend: Send + Sync + 'static {
    async fn eval_uquery(
        &self,
        ctx: &mut DiceComputations<'_>,
        working_dir: &ProjectRelativePath,
        query: &str,
        query_args: &[String],
    ) -> slug_error::Result<QueryEvaluationResult<TargetNode>>;

    async fn eval_cquery(
        &self,
        ctx: &mut DiceComputations<'_>,
        working_dir: &ProjectRelativePath,
        query: &str,
        query_args: &[String],
        global_cfg_options: GlobalCfgOptions,
        target_universe: Option<&[String]>,
        collect_universes: bool,
    ) -> slug_error::Result<(
        QueryEvaluationResult<ConfiguredTargetNode>,
        Option<Vec<Arc<CqueryUniverse>>>,
    )>;

    async fn eval_aquery(
        &self,
        ctx: &mut DiceComputations<'_>,
        working_dir: &ProjectRelativePath,
        query: &str,
        query_args: &[String],
        global_cfg_options: GlobalCfgOptions,
    ) -> slug_error::Result<QueryEvaluationResult<ActionQueryNode>>;
}

pub static QUERY_FRONTEND: LateBinding<&'static dyn QueryFrontend> =
    LateBinding::new("QUERY_FRONTEND");
