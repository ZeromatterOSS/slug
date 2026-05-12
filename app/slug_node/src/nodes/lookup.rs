/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use async_trait::async_trait;
use dice::LinearRecomputeDiceComputations;
use slug_core::target::configured_target_label::ConfiguredTargetLabel;
use slug_core::target::label::label::TargetLabel;
use slug_query::query::traversal::AsyncNodeLookup;

use crate::nodes::configured::ConfiguredTargetNode;
use crate::nodes::configured_frontend::ConfiguredTargetNodeCalculation;
use crate::nodes::frontend::TargetGraphCalculation;
use crate::nodes::unconfigured::TargetNode;

pub struct TargetNodeLookup<'c, 'd>(pub &'c LinearRecomputeDiceComputations<'d>);

#[async_trait]
impl AsyncNodeLookup<TargetNode> for TargetNodeLookup<'_, '_> {
    async fn get(&self, label: &TargetLabel) -> slug_error::Result<TargetNode> {
        Ok(self.0.get().get_target_node(label).await?)
    }
}

pub struct ConfiguredTargetNodeLookup<'c, 'd>(pub &'c LinearRecomputeDiceComputations<'d>);

#[async_trait]
impl AsyncNodeLookup<ConfiguredTargetNode> for ConfiguredTargetNodeLookup<'_, '_> {
    async fn get(&self, label: &ConfiguredTargetLabel) -> slug_error::Result<ConfiguredTargetNode> {
        Ok(self
            .0
            .get()
            .get_configured_target_node(label)
            .await?
            .require_compatible()?)
    }
}
