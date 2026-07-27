/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::DiceProjectionComputations;
use dice::DiceTransactionUpdater;
use dice::InjectedKey;
use dice::Key;
use dice::ProjectionKey;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_identity_v2::PackageIdentifier;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark_map::small_set::SmallSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub enum RootRepoFileUtf8Mode {
    Off,
    Warning,
    Error,
}

impl RootRepoFileUtf8Mode {
    pub fn from_bazel_flag_value(value: &str) -> Result<Self, String> {
        if value.eq_ignore_ascii_case("off")
            || matches!(value, "0")
            || value.eq_ignore_ascii_case("false")
            || value.eq_ignore_ascii_case("no")
            || value.eq_ignore_ascii_case("f")
            || value.eq_ignore_ascii_case("n")
        {
            Ok(Self::Off)
        } else if value.eq_ignore_ascii_case("warning") {
            Ok(Self::Warning)
        } else if value.eq_ignore_ascii_case("error")
            || matches!(value, "1")
            || value.eq_ignore_ascii_case("true")
            || value.eq_ignore_ascii_case("yes")
            || value.eq_ignore_ascii_case("t")
            || value.eq_ignore_ascii_case("y")
        {
            Ok(Self::Error)
        } else {
            Err(format!(
                "invalid --incompatible_enforce_starlark_utf8 value {value:?}: expected off, warning, error, or a boolean"
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RootRepoFileSemantics {
    pub utf8_mode: RootRepoFileUtf8Mode,
}

impl RootRepoFileSemantics {
    pub fn from_bazel_flag_value(value: Option<&str>) -> Result<Self, String> {
        Ok(Self {
            utf8_mode: match value {
                Some(value) => RootRepoFileUtf8Mode::from_bazel_flag_value(value)?,
                None => RootRepoFileUtf8Mode::Warning,
            },
        })
    }
}

impl Default for RootRepoFileSemantics {
    fn default() -> Self {
        Self {
            utf8_mode: RootRepoFileUtf8Mode::Warning,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RootPackagePolicyNormalizationError {
    InvalidDeletedPackage { value: String, message: String },
    InvalidRepoFileUtf8Mode { value: String, message: String },
}

impl fmt::Display for RootPackagePolicyNormalizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDeletedPackage { value, message } => {
                write!(f, "invalid deleted package {value:?}: {message}")
            }
            Self::InvalidRepoFileUtf8Mode { value, message } => {
                write!(f, "invalid repo file UTF-8 mode {value:?}: {message}")
            }
        }
    }
}

impl std::error::Error for RootPackagePolicyNormalizationError {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootPackagePolicyInputs {
    workspace: NormalizedAbsolutePath,
    package_roots: Arc<[NormalizedAbsolutePath]>,
    deleted_packages: SmallSet<PackageIdentifier>,
    vendor_directory: Option<NormalizedAbsolutePath>,
    repo_file_semantics: RootRepoFileSemantics,
}

impl RootPackagePolicyInputs {
    pub fn new<I, S>(
        workspace: NormalizedAbsolutePath,
        package_roots: impl Into<Arc<[NormalizedAbsolutePath]>>,
        deleted_package_occurrences: I,
        vendor_directory: Option<NormalizedAbsolutePath>,
        repo_file_utf8_flag_value: Option<&str>,
    ) -> Result<Self, RootPackagePolicyNormalizationError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut deleted_packages = SmallSet::new();
        for occurrence in deleted_package_occurrences {
            let occurrence = occurrence.as_ref();
            if occurrence.is_empty() {
                continue;
            }
            for value in occurrence.split(',') {
                let package = PackageIdentifier::parse_bazel_package_identifier(value).map_err(
                    |message| RootPackagePolicyNormalizationError::InvalidDeletedPackage {
                        value: value.to_owned(),
                        message,
                    },
                )?;
                deleted_packages.insert(package);
            }
        }
        let repo_file_semantics =
            RootRepoFileSemantics::from_bazel_flag_value(repo_file_utf8_flag_value).map_err(
                |message| RootPackagePolicyNormalizationError::InvalidRepoFileUtf8Mode {
                    value: repo_file_utf8_flag_value.unwrap_or_default().to_owned(),
                    message,
                },
            )?;
        Ok(Self {
            workspace,
            package_roots: package_roots.into(),
            deleted_packages,
            vendor_directory,
            repo_file_semantics,
        })
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn package_roots(&self) -> &[NormalizedAbsolutePath] {
        &self.package_roots
    }

    pub fn deleted_packages(&self) -> &SmallSet<PackageIdentifier> {
        &self.deleted_packages
    }

    pub fn vendor_directory(&self) -> Option<&NormalizedAbsolutePath> {
        self.vendor_directory.as_ref()
    }

    pub fn repo_file_semantics(&self) -> RootRepoFileSemantics {
        self.repo_file_semantics
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootRepositoryIgnoreInputs {
    package_roots: Arc<[NormalizedAbsolutePath]>,
    vendor_directory: Option<NormalizedAbsolutePath>,
}

impl RootRepositoryIgnoreInputs {
    pub fn package_roots(&self) -> &[NormalizedAbsolutePath] {
        &self.package_roots
    }

    pub fn vendor_directory(&self) -> Option<&NormalizedAbsolutePath> {
        self.vendor_directory.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootPackageLookupInputs {
    package_roots: Arc<[NormalizedAbsolutePath]>,
    deleted_packages: SmallSet<PackageIdentifier>,
}

impl RootPackageLookupInputs {
    pub fn package_roots(&self) -> &[NormalizedAbsolutePath] {
        &self.package_roots
    }

    pub fn deleted_packages(&self) -> &SmallSet<PackageIdentifier> {
        &self.deleted_packages
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum RootPackagePolicyProjectionError {
    MissingInput { workspace: NormalizedAbsolutePath },
    ProjectionFailed { workspace: NormalizedAbsolutePath },
}

impl fmt::Display for RootPackagePolicyProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let workspace = match self {
            Self::MissingInput { workspace } | Self::ProjectionFailed { workspace } => workspace,
        };
        match self {
            Self::MissingInput { .. } => {
                write!(f, "missing root package policy inputs for {workspace}")
            }
            Self::ProjectionFailed { .. } => {
                write!(
                    f,
                    "failed to project root package policy inputs for {workspace}"
                )
            }
        }
    }
}

impl std::error::Error for RootPackagePolicyProjectionError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RootPackagePolicyInputsKey {
    workspace: NormalizedAbsolutePath,
}

impl fmt::Display for RootPackagePolicyInputsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-package-policy-inputs:{}", self.workspace)
    }
}

impl InjectedKey for RootPackagePolicyInputsKey {
    type Value = Arc<RootPackagePolicyInputs>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
struct RootRepoFileSemanticsProjection;

impl fmt::Display for RootRepoFileSemanticsProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("root-repo-file-semantics-projection")
    }
}

impl ProjectionKey for RootRepoFileSemanticsProjection {
    type DeriveFromKey = RootPackagePolicyInputsKey;
    type Value = RootRepoFileSemantics;

    fn compute(
        &self,
        inputs: &Arc<RootPackagePolicyInputs>,
        _ctx: &DiceProjectionComputations,
    ) -> Self::Value {
        inputs.repo_file_semantics
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
struct RootRepositoryIgnoreInputsProjection;

impl fmt::Display for RootRepositoryIgnoreInputsProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("root-repository-ignore-inputs-projection")
    }
}

impl ProjectionKey for RootRepositoryIgnoreInputsProjection {
    type DeriveFromKey = RootPackagePolicyInputsKey;
    type Value = RootRepositoryIgnoreInputs;

    fn compute(
        &self,
        inputs: &Arc<RootPackagePolicyInputs>,
        _ctx: &DiceProjectionComputations,
    ) -> Self::Value {
        RootRepositoryIgnoreInputs {
            package_roots: inputs.package_roots.dupe(),
            vendor_directory: inputs.vendor_directory.dupe(),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
struct RootPackageLookupInputsProjection;

impl fmt::Display for RootPackageLookupInputsProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("root-package-lookup-inputs-projection")
    }
}

impl ProjectionKey for RootPackageLookupInputsProjection {
    type DeriveFromKey = RootPackagePolicyInputsKey;
    type Value = Arc<RootPackageLookupInputs>;

    fn compute(
        &self,
        inputs: &Arc<RootPackagePolicyInputs>,
        _ctx: &DiceProjectionComputations,
    ) -> Self::Value {
        Arc::new(RootPackageLookupInputs {
            package_roots: inputs.package_roots.dupe(),
            deleted_packages: inputs.deleted_packages.clone(),
        })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
#[allow(dead_code)]
struct RootVendorDirectoryProjection;

impl fmt::Display for RootVendorDirectoryProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("root-vendor-directory-projection")
    }
}

impl ProjectionKey for RootVendorDirectoryProjection {
    type DeriveFromKey = RootPackagePolicyInputsKey;
    type Value = Option<NormalizedAbsolutePath>;

    fn compute(
        &self,
        inputs: &Arc<RootPackagePolicyInputs>,
        _ctx: &DiceProjectionComputations,
    ) -> Self::Value {
        inputs.vendor_directory.dupe()
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

macro_rules! root_package_policy_projection_key {
    (
        $visibility:vis $key:ident,
        $value:ty,
        $projection:expr,
        $display:literal
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
        #[allow(dead_code)]
        $visibility struct $key {
            workspace: NormalizedAbsolutePath,
        }

        #[allow(dead_code)]
        impl $key {
            $visibility fn new(workspace: NormalizedAbsolutePath) -> Self {
                Self { workspace }
            }

            $visibility fn workspace(&self) -> &NormalizedAbsolutePath {
                &self.workspace
            }
        }

        impl fmt::Display for $key {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", $display, self.workspace())
            }
        }

        #[async_trait]
        impl Key for $key {
            type Value = Result<$value, RootPackagePolicyProjectionError>;

            async fn compute(
                &self,
                ctx: &mut DiceComputations,
                _cancellations: &CancellationContext,
            ) -> Self::Value {
                let opaque = ctx
                    .compute_opaque(&RootPackagePolicyInputsKey {
                        workspace: self.workspace.dupe(),
                    })
                    .await
                    .map_err(|_| RootPackagePolicyProjectionError::MissingInput {
                        workspace: self.workspace.dupe(),
                    })?;
                ctx.projection(&opaque, &$projection).map_err(|_| {
                    RootPackagePolicyProjectionError::ProjectionFailed {
                        workspace: self.workspace.dupe(),
                    }
                })
            }

            fn equality(x: &Self::Value, y: &Self::Value) -> bool {
                x == y
            }
        }
    };
}

root_package_policy_projection_key!(
    pub RootRepoFileSemanticsProjectionKey,
    RootRepoFileSemantics,
    RootRepoFileSemanticsProjection,
    "root-repo-file-semantics"
);
root_package_policy_projection_key!(
    pub RootRepositoryIgnoreInputsProjectionKey,
    RootRepositoryIgnoreInputs,
    RootRepositoryIgnoreInputsProjection,
    "root-repository-ignore-inputs"
);
root_package_policy_projection_key!(
    pub RootPackageLookupInputsProjectionKey,
    Arc<RootPackageLookupInputs>,
    RootPackageLookupInputsProjection,
    "root-package-lookup-inputs"
);
root_package_policy_projection_key!(
    pub(crate) RootVendorDirectoryProjectionKey,
    Option<NormalizedAbsolutePath>,
    RootVendorDirectoryProjection,
    "root-vendor-directory"
);

pub fn inject_root_package_policy_inputs(
    updater: &mut DiceTransactionUpdater,
    inputs: RootPackagePolicyInputs,
) -> anyhow::Result<()> {
    let workspace = inputs.workspace.dupe();
    updater.changed_to(vec![(
        RootPackagePolicyInputsKey { workspace },
        Arc::new(inputs),
    )])?;
    Ok(())
}
