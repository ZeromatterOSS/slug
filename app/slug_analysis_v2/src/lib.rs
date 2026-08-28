/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

mod analysis_value;
mod build_setting;
mod command_configuration;
mod configured_analysis_cycle_detector;
mod configured_attribute;
pub mod configured_target;
pub mod dice;
pub mod key;
pub mod result;
pub mod starlark_rule;
pub mod toolchains;

pub use command_configuration::CommandConfigurationPreparationKey;
pub use command_configuration::CommandConfigurationPreparationObservationKey;
pub use command_configuration::CommandConfigurationPreparationOutcome;
pub use command_configuration::CommandConfigurationPreparationOuterError;
pub use command_configuration::ObservedCommandConfiguration;
pub use command_configuration::ObservedCommandConfigurationPreparationOutcome;
pub use configured_analysis_cycle_detector::analysis_cycle_detector;
pub use configured_target::ConfiguredEdge;
pub use configured_target::ConfiguredEdgeKind;
pub use dice::AnalysisError;
pub use dice::AnalysisErrorKind;
pub use dice::ConfiguredConditionKey;
pub use dice::ConfiguredConditionMatch;
pub use dice::ConfiguredConditionOutcome;
pub use dice::ConfiguredNodeAnalysisKey;
#[doc(hidden)]
pub use dice::ConfiguredNodeAnalysisObservationKey;
pub use dice::ConfiguredPlatformKey;
pub use dice::ConfiguredPlatformOutcome;
pub use dice::ConfiguredTargetPlatformKey;
pub use dice::ConfiguredToolchainResolutionKey;
#[doc(hidden)]
pub use dice::ConfiguredToolchainResolutionObservationKey;
#[doc(hidden)]
pub use dice::ObservedConfiguredNodeAnalysisPreparationOutcome;
pub use dice::prepare_configured_node_analysis;
#[doc(hidden)]
pub use dice::prepare_configured_node_analysis_observed;
pub use key::ConfigurationChecksum;
pub use key::ConfigurationKey;
pub use key::ConfigurationKind;
pub use key::ConfiguredNodeKey;
pub use key::ConfiguredTargetKey;
pub use result::AnalysisDiagnostic;
pub use result::ConfiguredAction;
pub use result::ConfiguredActionAspectProvenance;
pub use result::ConfiguredActionExecGroup;
pub use result::ConfiguredActionExecutionState;
pub use result::ConfiguredActionOwnerContext;
pub use result::ConfiguredActionPlatformConstraint;
pub use result::ConfiguredActionToolchainContext;
pub use result::ConfiguredActionView;
pub use result::ConfiguredNodeKind;
pub use result::ConfiguredNodeResult;
pub use result::ConfiguredPlatform;
pub use result::ConfiguredToolchainResolution;
pub use result::ConfiguredToolchainResolutionRow;
pub use result::DiagnosticSeverity;
pub use result::PlatformSemanticFact;
pub use result::ToolchainSelection;
pub use result::ToolchainTopology;
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
