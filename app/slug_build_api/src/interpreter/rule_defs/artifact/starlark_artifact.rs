/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt::Display;

use allocative::Allocative;
use dupe::Dupe;
use dupe::OptionDupedExt;
use slug_artifact::artifact::artifact_type::Artifact;
use slug_artifact::artifact::artifact_type::BaseArtifactKind;
use slug_core::deferred::base_deferred_key::BaseDeferredKey;
use slug_execute::execute::request::OutputType;
use slug_execute::path::artifact_path::ArtifactPath;
use slug_fs::paths::file_name::FileName;
use slug_fs::paths::forward_rel_path::ForwardRelativePath;
use slug_interpreter::types::provider::callable::ValueAsProviderCallableLike;
use serde::Serialize;
use serde::Serializer;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::environment::Methods;
use starlark::environment::MethodsStatic;
use starlark::values::Demand;
use starlark::values::Heap;
use starlark::values::StarlarkValue;
use starlark::values::StringValue;
use starlark::values::Value;
use starlark::values::list::UnpackList;
use starlark::values::starlark_value;
use starlark::values::type_repr::StarlarkTypeRepr;

use crate::artifact_groups::ArtifactGroup;
use crate::interpreter::rule_defs::artifact::ArtifactError;
use crate::interpreter::rule_defs::artifact::associated::AssociatedArtifacts;
use crate::interpreter::rule_defs::artifact::methods::EitherStarlarkInputArtifact;
use crate::interpreter::rule_defs::artifact::methods::artifact_methods;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::ArtifactFingerprint;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkArtifactLike;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkInputArtifactLike;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::ValueAsInputArtifactLike;
use crate::interpreter::rule_defs::artifact::starlark_output_artifact::StarlarkOutputArtifact;
use crate::interpreter::rule_defs::cmd_args::ArtifactPathMapper;
use crate::interpreter::rule_defs::cmd_args::CommandLineArgLike;
use crate::interpreter::rule_defs::cmd_args::CommandLineArtifactVisitor;
use crate::interpreter::rule_defs::cmd_args::CommandLineBuilder;
use crate::interpreter::rule_defs::cmd_args::CommandLineContext;
use crate::interpreter::rule_defs::cmd_args::WriteToFileMacroVisitor;
use crate::interpreter::rule_defs::cmd_args::command_line_arg_like_type::command_line_arg_like_impl;
use crate::interpreter::rule_defs::provider::builtin::default_info::DefaultInfo;
use crate::interpreter::rule_defs::provider::builtin::default_info::DefaultInfoCallable;

/// Error types for artifact provider access.
///
/// In Bazel, source files (artifacts) implicitly have `DefaultInfo` with the file
/// in the `files` field. This allows patterns like `DefaultInfo in artifact` and
/// `artifact[DefaultInfo]` to work in rule implementations.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
enum ArtifactProviderError {
    #[error(
        "artifact[<provider>] operation requires a provider type, got `{0}`. \
         Artifacts only support DefaultInfo provider access."
    )]
    IndexTypeNotProvider(&'static str),
    #[error("artifact does not have provider `{provider}`. Artifacts only have DefaultInfo.")]
    ProviderNotFound { provider: String },
}

/// A wrapper for an `Artifact` that is guaranteed to be bound, such as outputs
/// from dependencies, or source files.
#[derive(Debug, Dupe, Clone, PartialEq, ProvidesStaticType, Allocative)]
pub struct StarlarkArtifact {
    pub(crate) artifact: Artifact,
    // A set of ArtifactGroups that should be materialized along with the main artifact
    pub(crate) associated_artifacts: AssociatedArtifacts,
}

starlark_simple_value!(StarlarkArtifact);

impl StarlarkArtifact {
    pub fn new(artifact: Artifact) -> Self {
        StarlarkArtifact {
            artifact,
            associated_artifacts: AssociatedArtifacts::new(),
        }
    }

    /// Constructor for `StarlarkArtifact` with an existing set of associated artifacts.
    /// Used by `SYNTHESIZE_RUNFILES_TREE` to attach a runfiles symlink tree to an
    /// executable without going through `with_associated_artifacts`'s `UnpackList`
    /// argument shape.
    pub fn new_with_associated_artifacts(
        artifact: Artifact,
        associated_artifacts: AssociatedArtifacts,
    ) -> Self {
        StarlarkArtifact {
            artifact,
            associated_artifacts,
        }
    }

    pub fn artifact(&self) -> Artifact {
        self.artifact.dupe()
    }

    pub fn get_artifact_path(&self) -> ArtifactPath<'_> {
        self.artifact.get_path()
    }
}

impl Display for StarlarkArtifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // FIXME(ndmitchell): This display is not the same as the underlying Artifact, but they should probably be the same
        write!(
            f,
            "<{} ",
            if self.artifact.is_source() {
                "source"
            } else {
                "build artifact"
            }
        )?;

        // Use display path in repr: cell-relative for source files, short_path for build artifacts.
        // e.g., source file "artifacts/DATA" -> "<source artifacts/DATA>"
        self.artifact
            .get_path()
            .with_display_path(|p| write!(f, "{p}"))?;

        if let Some(owner) = self.artifact.owner() {
            write!(f, " bound to {owner}")?;
        }

        write!(f, ">")?;

        Ok(())
    }
}

impl Serialize for StarlarkArtifact {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.collect_str(self)
    }
}

impl<'v> StarlarkArtifactLike<'v> for StarlarkArtifact {
    fn with_filename(
        &self,
        f: &dyn for<'b> Fn(&'b FileName) -> StringValue<'v>,
    ) -> slug_error::Result<StringValue<'v>> {
        self.artifact.get_path().with_filename(f)
    }

    fn is_source(&'v self) -> slug_error::Result<bool> {
        Ok(self.artifact.is_source())
    }

    fn is_directory(&self) -> bool {
        match self.artifact.as_parts().0 {
            BaseArtifactKind::Build(ba) => matches!(ba.output_type(), OutputType::Directory),
            BaseArtifactKind::Source(_) => false,
        }
    }

    fn owner(&'v self) -> slug_error::Result<Option<BaseDeferredKey>> {
        Ok(self.artifact.owner().duped())
    }

    fn with_short_path(
        &self,
        f: &dyn for<'b> Fn(&'b ForwardRelativePath) -> StringValue<'v>,
    ) -> slug_error::Result<StringValue<'v>> {
        Ok(self.artifact.get_path().with_short_path(f))
    }

    fn with_display_path(
        &self,
        f: &dyn for<'b> Fn(&'b ForwardRelativePath) -> StringValue<'v>,
    ) -> slug_error::Result<StringValue<'v>> {
        Ok(self.artifact.get_path().with_display_path(f))
    }

    fn with_full_path(
        &self,
        f: &dyn for<'b> Fn(&'b ForwardRelativePath) -> StringValue<'v>,
    ) -> slug_error::Result<StringValue<'v>> {
        Ok(self.artifact.get_path().with_full_path(f))
    }

    fn with_root_path(
        &self,
        f: &dyn for<'b> Fn(&'b ForwardRelativePath) -> StringValue<'v>,
    ) -> slug_error::Result<StringValue<'v>> {
        Ok(self.artifact.get_path().with_root_path(f))
    }

    fn source_path_info(&self) -> Option<(slug_core::package::PackageLabel, String)> {
        let source = self.artifact.get_source()?;
        let source_path = source.get_path();
        Some((
            source_path.package(),
            source_path.path().as_str().to_owned(),
        ))
    }

    fn fingerprint<'s>(&'s self) -> ArtifactFingerprint<'s>
    where
        'v: 's,
    {
        let path = self.artifact.get_path();
        let associated_artifacts = self.get_associated_artifacts();
        ArtifactFingerprint::Normal {
            path,
            associated_artifacts,
            is_output: false,
        }
    }
}

impl<'v> StarlarkInputArtifactLike<'v> for StarlarkArtifact {
    fn as_output_error(&self) -> slug_error::Error {
        match self.artifact.as_parts().0 {
            BaseArtifactKind::Source(_) => ArtifactError::SourceArtifactAsOutput {
                repr: self.to_string(),
            }
            .into(),
            BaseArtifactKind::Build(b) => ArtifactError::BoundArtifactAsOutput {
                artifact_repr: self.to_string(),
                existing_owner: b.get_path().owner().owner().dupe(),
            }
            .into(),
        }
    }

    fn get_bound_artifact(&self) -> slug_error::Result<Artifact> {
        Ok(self.artifact.dupe())
    }

    fn get_associated_artifacts(&self) -> Option<&AssociatedArtifacts> {
        Some(&self.associated_artifacts)
    }

    fn as_command_line_like(&self) -> &dyn CommandLineArgLike<'v> {
        self
    }

    fn get_artifact_group(&self) -> slug_error::Result<ArtifactGroup> {
        Ok(ArtifactGroup::Artifact(self.get_bound_artifact()?))
    }

    fn as_output(&'v self, _this: Value<'v>) -> slug_error::Result<StarlarkOutputArtifact<'v>> {
        match self.artifact.as_parts().0 {
            BaseArtifactKind::Source(_) => Err(ArtifactError::SourceArtifactAsOutput {
                repr: self.to_string(),
            }
            .into()),
            BaseArtifactKind::Build(b) => Err(ArtifactError::BoundArtifactAsOutput {
                artifact_repr: self.to_string(),
                existing_owner: b.get_path().owner().owner().dupe(),
            }
            .into()),
        }
    }

    fn project(
        &'v self,
        path: &ForwardRelativePath,
        hide_prefix: bool,
    ) -> slug_error::Result<EitherStarlarkInputArtifact<'v>> {
        Ok(EitherStarlarkInputArtifact::Artifact(StarlarkArtifact {
            artifact: self.artifact.dupe().project(path, hide_prefix),
            associated_artifacts: self.associated_artifacts.dupe(),
        }))
    }

    fn without_associated_artifacts(
        &'v self,
    ) -> slug_error::Result<EitherStarlarkInputArtifact<'v>> {
        Ok(EitherStarlarkInputArtifact::Artifact(StarlarkArtifact {
            artifact: self.artifact.dupe(),
            associated_artifacts: AssociatedArtifacts::new(),
        }))
    }

    fn with_associated_artifacts(
        &'v self,
        artifacts: UnpackList<ValueAsInputArtifactLike<'v>>,
    ) -> slug_error::Result<EitherStarlarkInputArtifact<'v>> {
        let artifacts = artifacts
            .items
            .iter()
            .map(|a| a.0.get_artifact_group())
            .collect::<Result<Vec<_>, _>>()?;

        let artifacts = AssociatedArtifacts::from(artifacts);

        Ok(EitherStarlarkInputArtifact::Artifact(StarlarkArtifact {
            artifact: self.artifact.dupe(),
            associated_artifacts: self.associated_artifacts.union(artifacts),
        }))
    }
}

impl<'v> CommandLineArgLike<'v> for StarlarkArtifact {
    fn register_me(&self) {
        command_line_arg_like_impl!(StarlarkArtifact::starlark_type_repr());
    }

    fn add_to_command_line(
        &self,
        cli: &mut dyn CommandLineBuilder,
        ctx: &mut dyn CommandLineContext,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<()> {
        cli.push_location(ctx.resolve_artifact(&self.artifact, artifact_path_mapping)?);
        Ok(())
    }

    fn visit_artifacts(
        &self,
        visitor: &mut dyn CommandLineArtifactVisitor<'v>,
    ) -> slug_error::Result<()> {
        visitor.visit_input(ArtifactGroup::Artifact(self.artifact.dupe()), vec![]);
        self.associated_artifacts
            .iter()
            .for_each(|ag| visitor.visit_input(ag.dupe(), vec![]));
        Ok(())
    }

    fn contains_arg_attr(&self) -> bool {
        false
    }

    fn visit_write_to_file_macros(
        &self,
        _visitor: &mut dyn WriteToFileMacroVisitor,
        _artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> slug_error::Result<()> {
        Ok(())
    }
}

// Use "File" type for Bazel compatibility - Bazel calls artifacts "File"
#[starlark_value(type = "File")]
impl<'v> StarlarkValue<'v> for StarlarkArtifact {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(artifact_methods)
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        StarlarkArtifactLike::equals(self, other)
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        StarlarkArtifactLike::write_hash(self, hasher)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn CommandLineArgLike>(self);
    }

    /// Implements `artifact[SomeInfo]` - get a provider value.
    ///
    /// In Bazel, source files (artifacts) implicitly have `DefaultInfo` with the file
    /// in the `files` field. This enables patterns like:
    /// ```python
    /// if DefaultInfo in src:
    ///     files = src[DefaultInfo].files.to_list()
    /// ```
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        match index.as_provider_callable() {
            Some(callable) => {
                let provider_id = callable.id()?;
                // Artifacts only support DefaultInfo
                if provider_id == DefaultInfoCallable::provider_id() {
                    // Allocate the artifact on the heap and create a synthetic DefaultInfo.
                    // The artifact value will be used in DefaultInfo.files.
                    let artifact_value = heap.alloc(self.dupe());
                    let default_info = DefaultInfo::from_artifact_value(heap, artifact_value);
                    Ok(heap.alloc(default_info))
                } else {
                    Err(
                        slug_error::Error::from(ArtifactProviderError::ProviderNotFound {
                            provider: provider_id.name.clone(),
                        })
                        .into(),
                    )
                }
            }
            None => Err(
                slug_error::Error::from(ArtifactProviderError::IndexTypeNotProvider(
                    index.get_type(),
                ))
                .into(),
            ),
        }
    }

    /// Implements `SomeInfo in artifact` - check if provider is present.
    ///
    /// In Bazel, source files (artifacts) implicitly have `DefaultInfo`. This enables
    /// patterns like `if DefaultInfo in src: ...` to work in rule implementations.
    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        match other.as_provider_callable() {
            Some(callable) => {
                let provider_id = callable.id()?;
                // Artifacts only have DefaultInfo
                Ok(provider_id == DefaultInfoCallable::provider_id())
            }
            None => {
                // Not a provider type - this isn't an error in Bazel, just returns false
                // However, to match Bazel behavior we should error for non-provider types
                Err(
                    slug_error::Error::from(ArtifactProviderError::IndexTypeNotProvider(
                        other.get_type(),
                    ))
                    .into(),
                )
            }
        }
    }
}
