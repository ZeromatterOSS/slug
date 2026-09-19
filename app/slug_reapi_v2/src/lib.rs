/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod action_cache;
pub mod cas;
pub mod command;
pub mod config;
pub mod digest;
pub mod evidence;
pub mod executor;
pub mod input_tree;
pub mod proto;

pub use action_cache::ActionCacheEntry;
pub use action_cache::ActionCacheStatus;
pub use action_cache::ActionCacheTable;
pub use action_cache::ActionResult;
pub use cas::CasUploadPlan;
pub use cas::GeneratedDirectory;
pub use cas::GeneratedOutput;
pub use cas::GeneratedOutputReuploadPlan;
pub use command::ReapiActionIdentity;
pub use command::ReapiCommand;
pub use command::is_file_write_action;
pub use config::RemoteConfig;
pub use config::RemoteConfigError;
pub use config::RemoteMode;
pub use digest::ReapiDigest;
pub use evidence::ExecutionEvidence;
pub use executor::FileWriteReapiPlan;
pub use executor::RemoteExecutionError;
pub use executor::RemoteExecutionResult;
pub use executor::execute_action;
pub use executor::execute_file_write;
pub use executor::materialize_outputs;
pub use executor::verify_materialized_run_executable;
pub use input_tree::InputTreeEntryKind;
pub use input_tree::InputTreeError;
pub use input_tree::ReapiBlob;
pub use input_tree::ReapiInputTree;
pub use input_tree::ReapiInputTreeEntry;

pub mod source_staging;
pub use source_staging::SourceInputReapiPlan;

pub mod source_spawn;
pub use source_spawn::SourceSpawnReapiPlan;

mod source_execution;
pub use source_execution::SourceReapiTransport;
pub use source_execution::StagedSourceAction;

mod output_tree;

mod action_chain;
pub use action_chain::ActionChainReapiSession;
pub use action_chain::ActionChainReapiTransport;
pub use action_chain::ActionChainRemoteResult;
pub use action_chain::ActionChainStepResult;
pub use action_chain::LocalManifestResult;
pub use action_chain::StagedChainAction;

mod requested_build;
pub use requested_build::requested_build_success_json;
