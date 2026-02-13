/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed licenses.
 */

//! Deferred key for aspect-registered actions.
//!
//! When an aspect registers actions via `ctx.actions`, those actions need to be
//! looked up separately from the target's own actions. `AspectDeferredKey`
//! implements `BaseDeferredKeyDyn` and routes action lookups through
//! `EVAL_ASPECT_DEFERRED` to the aspect's DICE-cached `AnalysisResult`.
//!
//! The path generation (`make_hashed_path`) produces identical paths to
//! `BaseDeferredKey::TargetLabel`, since aspect outputs should appear in the
//! same directory as the target's outputs (matching Bazel semantics).

use std::any::Any;
use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use cmp_any::PartialEqAny;
use kuro_core::content_hash::ContentBasedPathHash;
use kuro_core::deferred::base_deferred_key::BaseDeferredKeyDyn;
use kuro_core::deferred::base_deferred_key::PathResolutionError;
use kuro_core::fs::buck_out_path::BuckOutPathKind;
use kuro_core::fs::project_rel_path::ProjectRelativePath;
use kuro_core::fs::project_rel_path::ProjectRelativePathBuf;
use kuro_core::global_cfg_options::GlobalCfgOptions;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_core::target::name::EQ_SIGN_SUBST;
use kuro_data::ToProtoMessage;
use kuro_data::action_key_owner::BaseDeferredKeyProto;
use kuro_fs::paths::forward_rel_path::ForwardRelativePath;
use kuro_node::aspect_type::StarlarkAspectType;

/// Deferred key for aspect-registered actions.
///
/// Stored inside `BaseDeferredKey::Aspect(Arc<AspectDeferredKey>)`.
/// When the build system needs to look up actions from this key, it dispatches
/// through `EVAL_ASPECT_DEFERRED` which computes the `AspectKey` via DICE.
#[derive(Debug, Clone, Allocative)]
pub struct AspectDeferredKey {
    /// The target this aspect is applied to.
    pub target: ConfiguredTargetLabel,
    /// The aspect type (module path + name) for DICE lookup.
    pub aspect_type: Arc<StarlarkAspectType>,
}

impl fmt::Display for AspectDeferredKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AspectDeferred({}, {})",
            self.target, self.aspect_type.name
        )
    }
}

impl PartialEq for AspectDeferredKey {
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target && self.aspect_type == other.aspect_type
    }
}

impl Eq for AspectDeferredKey {}

impl Hash for AspectDeferredKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.aspect_type.hash(state);
    }
}

impl AspectDeferredKey {
    fn escape_target_name(target_name: &str) -> Cow<'_, str> {
        if target_name.contains('=') {
            Cow::Owned(target_name.replace('=', EQ_SIGN_SUBST))
        } else {
            Cow::Borrowed(target_name)
        }
    }

    fn compute_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        Hash::hash(self, &mut hasher);
        hasher.finish()
    }
}

impl BaseDeferredKeyDyn for AspectDeferredKey {
    fn eq_token(&self) -> PartialEqAny<'_> {
        PartialEqAny::new(self)
    }

    fn hash(&self) -> u64 {
        self.compute_hash()
    }

    fn strong_hash(&self) -> u64 {
        self.compute_hash()
    }

    /// Produce the same output paths as `BaseDeferredKey::TargetLabel(target)`.
    ///
    /// Aspect outputs appear in the same directory as target outputs,
    /// matching Bazel semantics where aspect-generated files are siblings
    /// of the target's own outputs.
    fn make_hashed_path(
        &self,
        base: &ProjectRelativePath,
        prefix: &ForwardRelativePath,
        action_key: Option<&str>,
        path: &ForwardRelativePath,
        path_resolution_method: BuckOutPathKind,
        content_hash: Option<&ContentBasedPathHash>,
    ) -> kuro_error::Result<ProjectRelativePathBuf> {
        let target = &self.target;
        let cell_relative_path = target.pkg().cell_relative_path().as_str();
        let escaped_target_name = Self::escape_target_name(target.name().as_str());

        let path_identifier = match path_resolution_method {
            BuckOutPathKind::Configuration => [
                target.cfg().output_hash().as_str(),
                if target.exec_cfg().is_some() { "-" } else { "" },
                target
                    .exec_cfg()
                    .as_ref()
                    .map_or("", |x| x.output_hash().as_str()),
                "/",
                cell_relative_path,
                if cell_relative_path.is_empty() {
                    ""
                } else {
                    "/"
                },
                "__",
                escaped_target_name.as_ref(),
                "__",
                "/",
                if action_key.is_none() {
                    ""
                } else {
                    "__action__"
                },
                action_key.unwrap_or_default(),
                if action_key.is_none() { "" } else { "__/" },
            ],
            BuckOutPathKind::ContentHash => {
                let content_hash = content_hash.as_ref().map(|x| x.as_str());
                if let Some(content_hash) = content_hash {
                    [
                        cell_relative_path,
                        if cell_relative_path.is_empty() {
                            ""
                        } else {
                            "/"
                        },
                        "__",
                        escaped_target_name.as_ref(),
                        "__",
                        "/",
                        if action_key.is_none() {
                            ""
                        } else {
                            "__action__"
                        },
                        action_key.unwrap_or_default(),
                        if action_key.is_none() { "" } else { "__/" },
                        content_hash,
                        "/",
                        "",
                        "",
                    ]
                } else {
                    return Err(PathResolutionError::ContentBasedPathWithNoContentHash(
                        path.to_buf(),
                    ))?;
                }
            }
        };

        let path_str = path_identifier.concat();

        let hashed_path = [
            base.as_str(),
            "/",
            prefix.as_str(),
            "/",
            target.pkg().cell_name().as_str(),
            "/",
            path_str.as_str(),
            path.as_str(),
        ];

        Ok(ProjectRelativePathBuf::unchecked_new(hashed_path.concat()))
    }

    fn configured_label(&self) -> Option<ConfiguredTargetLabel> {
        Some(self.target.clone())
    }

    fn to_proto(&self) -> BaseDeferredKeyProto {
        BaseDeferredKeyProto::TargetLabel(self.target.as_proto())
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn global_cfg_options(&self) -> Option<GlobalCfgOptions> {
        None
    }
}
