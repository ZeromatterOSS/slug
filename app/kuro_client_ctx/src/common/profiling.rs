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

#[derive(
    clap::ValueEnum,
    Dupe,
    Clone,
    Copy,
    Debug,
    serde::Serialize,
    serde::Deserialize
)]
pub enum BuckProfileMode {
    TimeFlame,
    HeapAllocated,
    HeapRetained,
    HeapFlameAllocated,
    HeapFlameRetained,
    HeapSummaryAllocated,
    HeapSummaryRetained,
    Statement,
    Bytecode,
    BytecodePairs,
    Typecheck,
    Coverage,
    None,
}

impl BuckProfileMode {
    pub fn to_proto(&self) -> kuro_cli_proto::ProfileMode {
        match self {
            BuckProfileMode::TimeFlame => kuro_cli_proto::ProfileMode::TimeFlame,
            BuckProfileMode::HeapAllocated => kuro_cli_proto::ProfileMode::HeapAllocated,
            BuckProfileMode::HeapRetained => kuro_cli_proto::ProfileMode::HeapRetained,
            BuckProfileMode::HeapFlameAllocated => kuro_cli_proto::ProfileMode::HeapFlameAllocated,
            BuckProfileMode::HeapFlameRetained => kuro_cli_proto::ProfileMode::HeapFlameRetained,
            BuckProfileMode::HeapSummaryAllocated => {
                kuro_cli_proto::ProfileMode::HeapSummaryAllocated
            }
            BuckProfileMode::HeapSummaryRetained => {
                kuro_cli_proto::ProfileMode::HeapSummaryRetained
            }
            BuckProfileMode::Statement => kuro_cli_proto::ProfileMode::Statement,
            BuckProfileMode::Bytecode => kuro_cli_proto::ProfileMode::Bytecode,
            BuckProfileMode::BytecodePairs => kuro_cli_proto::ProfileMode::BytecodePairs,
            BuckProfileMode::Typecheck => kuro_cli_proto::ProfileMode::Typecheck,
            BuckProfileMode::Coverage => kuro_cli_proto::ProfileMode::Coverage,
            BuckProfileMode::None => kuro_cli_proto::ProfileMode::None,
        }
    }
}
