/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_identity_v2::ApparentLabel;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoadLabel {
    label: ApparentLabel,
}

impl LoadLabel {
    pub fn parse(value: &str) -> Result<Self, String> {
        let label = ApparentLabel::parse(value)?;
        if !label.target().as_str().ends_with(".bzl") {
            return Err(format!("load label must point to a .bzl file: {value}"));
        }
        Ok(Self { label })
    }

    pub fn label(&self) -> &ApparentLabel {
        &self.label
    }
}
