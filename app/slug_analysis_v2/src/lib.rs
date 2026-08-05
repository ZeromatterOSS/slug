/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod configured_target;
pub mod dice;
pub mod key;
pub mod result;
pub mod starlark_rule;
pub mod toolchains;

pub use configured_target::ConfiguredDependency;
pub use configured_target::TransitionEdge;
pub use configured_target::TransitionKind;
pub use dice::AnalysisError;
pub use dice::AnalysisErrorKind;
pub use dice::ConfiguredTargetAnalysisKey;
pub use dice::RootConfiguredTargetAnalysisKey;
pub use key::ConfigurationChecksum;
pub use key::ConfigurationKey;
pub use key::ConfigurationKind;
pub use key::ConfiguredTargetKey;
pub use result::AnalysisDiagnostic;
pub use result::AnalysisResult;
pub use result::DiagnosticSeverity;
pub use slug_loading_v2::LoadingPreparationNeeds as AnalysisPreparationNeeds;
pub use slug_loading_v2::LoadingPreparationOutcome as AnalysisPreparationOutcome;
pub use toolchains::ConstraintSet;
pub use toolchains::ConstraintSetting;
pub use toolchains::ConstraintValue;
pub use toolchains::ExecGroup;
pub use toolchains::ExecGroupCollection;
pub use toolchains::ExecutionPlatform;
pub use toolchains::RegisteredToolchains;
pub use toolchains::RegisteredToolchainsKey;
pub use toolchains::ResolvedToolchainContext;
pub use toolchains::ToolchainResolution;
pub use toolchains::ToolchainResolutionError;
pub use toolchains::ToolchainResolutionRequest;
pub use toolchains::ToolchainTarget;
pub use toolchains::ToolchainType;
