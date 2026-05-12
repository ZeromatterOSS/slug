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

use dice::DetectCycles;
use dice::Dice;
use slug_common::dice::cells::SetCellResolver;
use slug_common::dice::data::SetIoProvider;
use slug_common::io::IoProvider;
use slug_common::legacy_configs::configs::LegacyBuckConfig;
use slug_common::legacy_configs::dice::SetLegacyConfigs;
use slug_execute::digest_config::DigestConfig;
use slug_execute::digest_config::SetDigestConfig;

use crate::actions::execute::dice_data::SetInvalidationTrackingConfig;
use crate::build::detailed_aggregated_metrics::dice::SetDetailedAggregatedMetricsEventHandler;
use crate::build::detailed_aggregated_metrics::events::start_detailed_aggregated_metrics_state_tracker;

/// Utility to configure the dice globals.
/// One place to not forget to initialize something in all places.
pub async fn configure_dice_for_buck(
    io: Arc<dyn IoProvider>,
    digest_config: DigestConfig,
    _root_config: Option<&LegacyBuckConfig>,
    detect_cycles: Option<DetectCycles>,
) -> slug_error::Result<Arc<Dice>> {
    let detect_cycles = detect_cycles.unwrap_or(DetectCycles::Enabled);

    let mut dice = Dice::builder();
    dice.set_io_provider(io);
    dice.set_digest_config(digest_config);
    dice.set_invalidation_tracking_config(false);

    dice.set_detailed_aggregated_metrics_event_handler(Some(
        start_detailed_aggregated_metrics_state_tracker(),
    ));

    let dice = dice.build(detect_cycles);
    let mut dice_ctx = dice.updater();
    dice_ctx.set_none_cell_resolver()?;
    dice_ctx.set_none_legacy_config_external_data()?;
    dice_ctx.commit().await;

    Ok(dice)
}
