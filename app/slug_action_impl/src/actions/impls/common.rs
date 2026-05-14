/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_artifact::artifact::build_artifact::BuildArtifact;
use slug_build_api::actions::box_slice_set::BoxSliceSet;
use slug_build_api::artifact_groups::ArtifactGroup;

pub(crate) fn first_output_artifact(outputs: &BoxSliceSet<BuildArtifact>) -> &BuildArtifact {
    outputs
        .iter()
        .next()
        .expect("a single artifact by construction")
}

pub(crate) fn first_output_from_slice(outputs: &[BuildArtifact]) -> &BuildArtifact {
    outputs
        .iter()
        .next()
        .expect("a single artifact by construction")
}

pub(crate) fn first_input_artifact(inputs: &BoxSliceSet<ArtifactGroup>) -> &ArtifactGroup {
    inputs
        .iter()
        .next()
        .expect("a single input by construction")
}
