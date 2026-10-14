/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

mod args_expansion;
mod build_info_transform;
mod cpp_compile;
pub mod ctx_actions;
pub mod reapi_projection;
pub mod registry;
mod resource_set;
pub mod runfiles_support;
mod solib_symlink;
mod spawn_command_line;
pub mod spec;

pub use build_info_transform::BuildInfoCallbackRecipe;
pub use build_info_transform::BuildInfoTransformKind;
pub use build_info_transform::BuildInfoTransformSpec;
pub use cpp_compile::CppCompileConstruction;
pub use cpp_compile::CppCompileData;
pub use cpp_compile::CppCompileDependencies;
pub use cpp_compile::CppCompileInputPreparation;
pub use cpp_compile::CppCompileInputSets;
pub use cpp_compile::CppCompileSpec;
pub use ctx_actions::CtxActions;
pub use reapi_projection::ReapiCommandProjection;
pub use registry::ActionError;
pub use registry::ActionRegistry;
pub use registry::RegisterableAction;
pub use resource_set::RetainedSpawnResourceSet;
pub use runfiles_support::RunfilesSupportActionSpec;
pub use runfiles_support::RunfilesSymlinkMode;
pub use solib_symlink::SolibSymlinkError;
pub use solib_symlink::SolibSymlinkSpec;
pub use spawn_command_line::ExpandedSpawnCommandLine;
pub use spawn_command_line::SpawnCommandLineError;
pub use spawn_command_line::VirtualParamFile;
pub use spec::ActionInput;
pub use spec::ActionKind;
pub use spec::ActionOutput;
pub use spec::ActionOutputKind;
pub use spec::ActionOutputMetadata;
pub use spec::ActionSpec;
pub use spec::ArgsWriteSpec;
pub use spec::ArtifactInputSource;
pub use spec::ArtifactInputs;
pub use spec::ParamFile;
pub use spec::ParamFileFormat;
pub use spec::RetainedArgCall;
pub use spec::RetainedArgsDepset;
pub use spec::RetainedArgsDepsetError;
pub use spec::RetainedArgsRecipe;
pub use spec::RetainedArtifactInputs;
pub use spec::RetainedArtifactInputsError;
pub use spec::RetainedCommandLine;
pub use spec::RetainedCommandLineSegment;
pub use spec::RetainedParamFileFormat;
pub use spec::RetainedScalarArg;
pub use spec::RetainedScalarValue;
pub use spec::RetainedSpawnArgsSnapshot;
pub use spec::RetainedSpawnInvocation;
pub use spec::RetainedSpawnParamFilePolicy;
pub use spec::RetainedVectorArg;
pub use spec::RetainedVectorOptions;
pub use spec::RetainedVectorSource;
pub use spec::SpawnExecutable;
pub use spec::SpawnSpec;
pub use spec::SymlinkSpec;
pub use spec::SymlinkTarget;
