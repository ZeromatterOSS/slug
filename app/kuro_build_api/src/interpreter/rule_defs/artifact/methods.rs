/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use allocative::Allocative;
use dupe::Dupe;
use kuro_core::configuration::data::ConfigurationData;
use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::provider::label::ProvidersName;
use kuro_core::target::label::label::TargetLabel;
use kuro_core::target::name::TargetNameRef;
use kuro_fs::paths::forward_rel_path::ForwardRelativePath;
use kuro_interpreter::types::configured_providers_label::StarlarkConfiguredProvidersLabel;
use starlark::any::ProvidesStaticType;
use starlark::environment::MethodsBuilder;
use starlark::values::AllocValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::StringValue;
use starlark::values::Value;
use starlark::values::ValueOf;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneOr;
use starlark::values::type_repr::StarlarkTypeRepr;

use crate::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkArtifactLike;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkInputArtifactLike;
use crate::interpreter::rule_defs::artifact::starlark_artifact_like::ValueAsInputArtifactLike;
use crate::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;
use crate::interpreter::rule_defs::artifact::starlark_output_artifact::StarlarkOutputArtifact;
use crate::interpreter::rule_defs::artifact::starlark_promise_artifact::StarlarkPromiseArtifact;

/// A stub for artifact root (Bazel compatibility).
/// Provides `path` attribute for output root path.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ArtifactRootStub {
    path: String,
}

impl std::fmt::Display for ArtifactRootStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<root {}>", self.path)
    }
}

starlark::starlark_simple_value!(ArtifactRootStub);

#[starlark::values::starlark_value(type = "root")]
impl<'v> StarlarkValue<'v> for ArtifactRootStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        attribute == "path"
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            _ => None,
        }
    }
}

#[derive(StarlarkTypeRepr, AllocValue)]
pub enum EitherStarlarkInputArtifact<'v> {
    Artifact(StarlarkArtifact),
    DeclaredArtifact(StarlarkDeclaredArtifact<'v>),
    PromiseArtifact(StarlarkPromiseArtifact),
}

#[starlark_module]
pub(crate) fn any_artifact_methods(builder: &mut MethodsBuilder) {
    /// The base name of this artifact. e.g. for an artifact at `foo/bar`, this is `bar`
    #[starlark(attribute)]
    fn basename<'v>(
        this: &'v dyn StarlarkArtifactLike<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<StringValue<'v>> {
        Ok(this.with_filename(&|filename| heap.alloc_str(filename.as_str()))?)
    }

    /// The file extension of this artifact. e.g. for an artifact at foo/bar.sh,
    /// this is `.sh` (with the leading dot). If no extension is present, `""` is returned.
    #[starlark(attribute)]
    fn extension<'v>(
        this: &'v dyn StarlarkArtifactLike<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<StringValue<'v>> {
        Ok(this.with_filename(&|filename| match filename.extension() {
            None => heap.alloc_str(""),
            Some(x) => heap.alloc_str(&format!(".{}", x)),
        })?)
    }

    /// Whether the artifact represents a source file
    #[starlark(attribute)]
    fn is_source<'v>(this: &'v dyn StarlarkArtifactLike<'v>) -> starlark::Result<bool> {
        Ok(this.is_source()?)
    }

    /// Whether the artifact represents a directory (Bazel-compatible).
    ///
    /// Returns true for tree artifacts (declared via ctx.actions.declare_directory),
    /// false for regular file artifacts.
    #[starlark(attribute)]
    fn is_directory<'v>(this: &'v dyn StarlarkArtifactLike<'v>) -> starlark::Result<bool> {
        Ok(this.is_directory())
    }

    /// The `Label` of the rule that originally created this artifact. May also be None in
    /// the case of source files, or if the artifact has not been used in an action, or if the
    /// action was not created by a rule.
    #[starlark(attribute)]
    fn owner<'v>(
        this: &'v dyn StarlarkArtifactLike<'v>,
    ) -> starlark::Result<NoneOr<StarlarkConfiguredProvidersLabel>> {
        match this.owner()? {
            Some(BaseDeferredKey::TargetLabel(target)) => {
                Ok(NoneOr::Other(StarlarkConfiguredProvidersLabel::new(
                    ConfiguredProvidersLabel::new(target.dupe(), ProvidersName::Default),
                )))
            }
            Some(BaseDeferredKey::Aspect(key)) => {
                // Aspect deferred key wraps a target - return the target's label
                if let Some(label) = key.configured_label() {
                    Ok(NoneOr::Other(StarlarkConfiguredProvidersLabel::new(
                        ConfiguredProvidersLabel::new(label, ProvidersName::Default),
                    )))
                } else {
                    Ok(NoneOr::None)
                }
            }
            None | Some(BaseDeferredKey::AnonTarget(_) | BaseDeferredKey::BxlLabel(_)) => {
                // For source files, construct a label from the source path info.
                // This is needed so that artifact.owner.workspace_root works in rules_cc
                // (cc_compilation_helper.bzl checks artifact.owner.workspace_root for
                // external cell source files).
                if let Some((package, name_str)) = this.source_path_info() {
                    if let Ok(target_name) = TargetNameRef::new(&name_str) {
                        let target_label = TargetLabel::new(package, target_name);
                        let configured = target_label.configure(ConfigurationData::unbound());
                        let providers_label =
                            ConfiguredProvidersLabel::new(configured, ProvidersName::Default);
                        return Ok(NoneOr::Other(StarlarkConfiguredProvidersLabel::new(
                            providers_label,
                        )));
                    }
                }
                Ok(NoneOr::None)
            }
        }
    }

    /// The Label of this artifact (Bazel-compatible).
    ///
    /// For generated files, this is the owner's label.
    /// For source files, this returns the short path as a string-like representation.
    /// This provides compatibility with Bazel's Target.label interface.
    #[starlark(attribute)]
    fn label<'v>(
        this: &'v dyn StarlarkArtifactLike<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // For Bazel compatibility, return the owner label for generated files,
        // or a path-based string for source files
        match this.owner()? {
            Some(BaseDeferredKey::TargetLabel(target)) => {
                Ok(heap.alloc(StarlarkConfiguredProvidersLabel::new(
                    ConfiguredProvidersLabel::new(target.dupe(), ProvidersName::Default),
                )))
            }
            Some(BaseDeferredKey::Aspect(key)) => {
                // Aspect deferred key wraps a target - return the target's label
                if let Some(label) = key.configured_label() {
                    Ok(heap.alloc(StarlarkConfiguredProvidersLabel::new(
                        ConfiguredProvidersLabel::new(label, ProvidersName::Default),
                    )))
                } else {
                    let path_str = this.with_short_path(&|p| heap.alloc_str(p.as_str()))?;
                    Ok(path_str.to_value())
                }
            }
            Some(BaseDeferredKey::AnonTarget(_) | BaseDeferredKey::BxlLabel(_)) | None => {
                // For source files, construct a proper label from the source path
                if let Some((package, name_str)) = this.source_path_info() {
                    if let Ok(target_name) = TargetNameRef::new(&name_str) {
                        let target_label = TargetLabel::new(package, target_name);
                        let configured = target_label.configure(ConfigurationData::unbound());
                        let providers_label =
                            ConfiguredProvidersLabel::new(configured, ProvidersName::Default);
                        return Ok(
                            heap.alloc(StarlarkConfiguredProvidersLabel::new(providers_label))
                        );
                    }
                }
                // Fallback: return path string
                let path_str = this.with_short_path(&|p| heap.alloc_str(p.as_str()))?;
                Ok(path_str.to_value())
            }
        }
    }

    /// The interesting part of the path, relative to somewhere in the output directory.
    /// For an artifact declared as `foo/bar`, this is `foo/bar`.
    #[starlark(attribute)]
    fn short_path<'v>(
        this: &'v dyn StarlarkArtifactLike<'v>,
        heap: Heap<'_>,
    ) -> starlark::Result<StringValue<'v>> {
        Ok(this.with_short_path(&|short_path| heap.alloc_str(short_path.as_str()))?)
    }

    /// The full execution path of this artifact (Bazel-compatible).
    /// For build artifacts, returns the complete buck-out path that can be used in commands.
    /// For source files, returns the cell-relative path.
    #[starlark(attribute)]
    fn path<'v>(
        this: &'v dyn StarlarkArtifactLike<'v>,
        heap: Heap<'_>,
    ) -> starlark::Result<StringValue<'v>> {
        Ok(this.with_full_path(&|path| heap.alloc_str(path.as_str()))?)
    }

    /// The directory part of the artifact's execution path (Bazel-compatible).
    ///
    /// For an artifact with path `foo/bar/baz.o`, this returns `foo/bar`.
    /// For an artifact at the root, returns an empty string.
    #[starlark(attribute)]
    fn dirname<'v>(
        this: &'v dyn StarlarkArtifactLike<'v>,
        heap: Heap<'_>,
    ) -> starlark::Result<StringValue<'v>> {
        Ok(this.with_full_path(&|path| {
            let path_str = path.as_str();
            match path_str.rfind('/') {
                Some(pos) => heap.alloc_str(&path_str[..pos]),
                None => heap.alloc_str(""),
            }
        })?)
    }

    /// The root directory of this artifact (Bazel-compatible).
    /// Returns a struct with a `path` attribute containing the output root.
    /// For generated files, root.path is the buck-out prefix before the short_path.
    #[starlark(attribute)]
    fn root<'v>(
        this: &'v dyn StarlarkArtifactLike<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        if this.is_source()? {
            return Ok(heap.alloc(ArtifactRootStub {
                path: String::new(),
            }));
        }
        // Compute root = full_path with short_path stripped from the end
        let full = this.with_full_path(&|p| heap.alloc_str(p.as_str()))?;
        let short = this.with_short_path(&|p| heap.alloc_str(p.as_str()))?;
        let full_str = full.as_str();
        let short_str = short.as_str();
        let root_path = if let Some(prefix) = full_str.strip_suffix(short_str) {
            prefix.trim_end_matches('/').to_owned()
        } else {
            match full_str.rfind('/') {
                Some(pos) => full_str[..pos].to_owned(),
                None => String::new(),
            }
        };
        Ok(heap.alloc(ArtifactRootStub { path: root_path }))
    }

    /// The executable file (Bazel FilesToRunProvider compatibility).
    /// For artifacts, this just returns the artifact itself.
    #[starlark(attribute)]
    fn executable<'v>(this: Value<'v>) -> starlark::Result<Value<'v>> {
        Ok(this)
    }
}

#[starlark_module]
fn input_artifact_methods(builder: &mut MethodsBuilder) {
    /// Returns a `StarlarkOutputArtifact` instance, or fails if the artifact is
    /// either an `Artifact`, or is a bound `Artifact` (You cannot bind twice)
    fn as_output<'v>(
        this: ValueOf<'v, &'v dyn StarlarkInputArtifactLike<'v>>,
    ) -> starlark::Result<StarlarkOutputArtifact<'v>> {
        Ok(this.typed.as_output(this.value)?)
    }

    /// Create an artifact that lives at path relative from this artifact.
    ///
    /// For example, if artifact foo is a directory containing a file bar, then `foo.project("bar")`
    /// yields the file bar. It is possible for projected artifacts to hide the prefix in order to
    /// have the short name of the resulting artifact only contain the projected path, by passing
    /// `hide_prefix = True` to `project()`.
    fn project<'v>(
        this: &'v dyn StarlarkInputArtifactLike<'v>,
        #[starlark(require = pos)] path: &str,
        #[starlark(require = named, default = false)] hide_prefix: bool,
    ) -> starlark::Result<EitherStarlarkInputArtifact<'v>> {
        let path = ForwardRelativePath::new(path)?;
        Ok(this.project(path, hide_prefix)?)
    }

    /// Returns a `StarlarkArtifact` instance which is identical to the original artifact, except
    /// with no associated artifacts
    fn without_associated_artifacts<'v>(
        this: &'v dyn StarlarkInputArtifactLike<'v>,
    ) -> starlark::Result<EitherStarlarkInputArtifact<'v>> {
        Ok(this.without_associated_artifacts()?)
    }

    /// Returns a `StarlarkArtifact` instance which is identical to the original artifact, but with
    /// potentially additional artifacts. The artifacts must be bound.
    fn with_associated_artifacts<'v>(
        this: &'v dyn StarlarkInputArtifactLike<'v>,
        artifacts: UnpackList<ValueAsInputArtifactLike<'v>>,
    ) -> starlark::Result<EitherStarlarkInputArtifact<'v>> {
        Ok(this.with_associated_artifacts(artifacts)?)
    }
}

/// A single input or output file for an action.
///
/// There is no `.parent` method on `artifact`, but in most cases
/// `cmd_args(my_artifact, parent = 1)` can be used to similar effect.
pub(crate) fn artifact_methods(builder: &mut MethodsBuilder) {
    any_artifact_methods(builder);
    input_artifact_methods(builder);
}
