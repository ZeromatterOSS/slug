/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use allocative::Allocative;
use dupe::Dupe;
use slug_bzlmod_v2::ROOT_MODULE_BOOTSTRAP_REMINDER_BYTES;
use slug_bzlmod_v2::RootModuleBootstrapApplyResult;
use slug_bzlmod_v2::RootModuleBootstrapCreateError;
use slug_bzlmod_v2::RootModuleBootstrapRequest;
use slug_bzlmod_v2::RootModuleBootstrapWarning;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathIoErrorKind;

#[allow(dead_code)] // Dormant until root-module bootstrap activation.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) struct RootModuleBootstrapOwner {
    workspace: NormalizedAbsolutePath,
}

#[allow(dead_code)] // Dormant until root-module bootstrap activation.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(super) enum RootModuleBootstrapApplyError {
    WorkspaceMismatch {
        owner_workspace: NormalizedAbsolutePath,
        request_workspace: NormalizedAbsolutePath,
    },
    Create(RootModuleBootstrapCreateError),
}

#[allow(dead_code)] // Dormant until root-module bootstrap activation.
impl RootModuleBootstrapOwner {
    pub(super) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }

    pub(super) fn apply(
        &self,
        request: &RootModuleBootstrapRequest,
    ) -> Result<RootModuleBootstrapApplyResult, RootModuleBootstrapApplyError> {
        if request.workspace != self.workspace {
            return Err(RootModuleBootstrapApplyError::WorkspaceMismatch {
                owner_workspace: self.workspace.dupe(),
                request_workspace: request.workspace.dupe(),
            });
        }

        let module_path = request.module_path();
        if module_path.as_path().exists() {
            return Ok(RootModuleBootstrapApplyResult::AlreadyPresent);
        }

        std::fs::write(module_path.as_path(), ROOT_MODULE_BOOTSTRAP_REMINDER_BYTES).map_err(
            |error| {
                RootModuleBootstrapApplyError::Create(RootModuleBootstrapCreateError {
                    module_path: module_path.dupe(),
                    kind: PathIoErrorKind::from(error.kind()),
                    raw_os_error: error.raw_os_error(),
                })
            },
        )?;
        Ok(RootModuleBootstrapApplyResult::Created(
            RootModuleBootstrapWarning,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn normalized(path: &Path) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(path.to_path_buf()).unwrap()
    }

    fn request(workspace: &NormalizedAbsolutePath) -> RootModuleBootstrapRequest {
        RootModuleBootstrapRequest {
            workspace: workspace.dupe(),
        }
    }

    #[test]
    fn creates_exact_reminder_then_preserves_warm_and_edited_files() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = normalized(temp.path());
        let owner = RootModuleBootstrapOwner::new(workspace.dupe());
        let request = request(&workspace);
        let module_path = request.module_path();

        assert_eq!(
            owner.apply(&request),
            Ok(RootModuleBootstrapApplyResult::Created(
                RootModuleBootstrapWarning
            ))
        );
        assert_eq!(
            std::fs::read(module_path.as_path()).unwrap().as_slice(),
            ROOT_MODULE_BOOTSTRAP_REMINDER_BYTES
        );
        assert_eq!(
            owner.apply(&request),
            Ok(RootModuleBootstrapApplyResult::AlreadyPresent)
        );
        assert_eq!(
            std::fs::read(module_path.as_path()).unwrap().as_slice(),
            ROOT_MODULE_BOOTSTRAP_REMINDER_BYTES
        );

        std::fs::write(module_path.as_path(), b"module(name = \"edited\")\n").unwrap();
        assert_eq!(
            owner.apply(&request),
            Ok(RootModuleBootstrapApplyResult::AlreadyPresent)
        );
        assert_eq!(
            std::fs::read(module_path.as_path()).unwrap().as_slice(),
            b"module(name = \"edited\")\n"
        );
    }

    #[test]
    fn delete_recreates_the_exact_reminder() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = normalized(temp.path());
        let owner = RootModuleBootstrapOwner::new(workspace.dupe());
        let request = request(&workspace);
        let module_path = request.module_path();

        assert!(owner.apply(&request).is_ok());
        std::fs::remove_file(module_path.as_path()).unwrap();
        assert_eq!(
            owner.apply(&request),
            Ok(RootModuleBootstrapApplyResult::Created(
                RootModuleBootstrapWarning
            ))
        );
        assert_eq!(
            std::fs::read(module_path.as_path()).unwrap().as_slice(),
            ROOT_MODULE_BOOTSTRAP_REMINDER_BYTES
        );
    }

    #[test]
    fn file_as_workspace_reports_typed_create_failure() {
        let temp = tempfile::tempdir().unwrap();
        let workspace_file = temp.path().join("workspace-file");
        std::fs::write(&workspace_file, b"not a directory").unwrap();
        let workspace = normalized(&workspace_file);
        let owner = RootModuleBootstrapOwner::new(workspace.dupe());
        let request = request(&workspace);
        let expected_module_path = request.module_path();

        let Err(RootModuleBootstrapApplyError::Create(error)) = owner.apply(&request) else {
            panic!("file-as-workspace must produce a typed create failure");
        };
        assert_eq!(error.module_path, expected_module_path);
        assert_eq!(error.kind, PathIoErrorKind::NotADirectory);
        assert!(error.raw_os_error.is_some());
    }

    #[test]
    fn rejects_a_foreign_workspace_without_touching_it() {
        let temp = tempfile::tempdir().unwrap();
        let owner_path = temp.path().join("owner");
        let foreign_path = temp.path().join("foreign");
        std::fs::create_dir(&owner_path).unwrap();
        std::fs::create_dir(&foreign_path).unwrap();
        let owner_workspace = normalized(&owner_path);
        let foreign_workspace = normalized(&foreign_path);
        let owner = RootModuleBootstrapOwner::new(owner_workspace.dupe());
        let request = request(&foreign_workspace);

        assert_eq!(
            owner.apply(&request),
            Err(RootModuleBootstrapApplyError::WorkspaceMismatch {
                owner_workspace,
                request_workspace: foreign_workspace,
            })
        );
        assert!(!owner_path.join("MODULE.bazel").exists());
        assert!(!foreign_path.join("MODULE.bazel").exists());
    }

    #[cfg(unix)]
    #[test]
    fn existing_symlink_is_preserved_without_overwriting_its_target() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let workspace = normalized(temp.path());
        let owner = RootModuleBootstrapOwner::new(workspace.dupe());
        let request = request(&workspace);
        let module_path = request.module_path();
        let target = temp.path().join("target.MODULE.bazel");
        std::fs::write(&target, b"module(name = \"target\")\n").unwrap();
        symlink("target.MODULE.bazel", module_path.as_path()).unwrap();

        assert_eq!(
            owner.apply(&request),
            Ok(RootModuleBootstrapApplyResult::AlreadyPresent)
        );
        assert!(
            std::fs::symlink_metadata(module_path.as_path())
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::read(target).unwrap().as_slice(),
            b"module(name = \"target\")\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_followed_and_its_target_is_created() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let workspace = normalized(temp.path());
        let owner = RootModuleBootstrapOwner::new(workspace.dupe());
        let request = request(&workspace);
        let module_path = request.module_path();
        let target = temp.path().join("target.MODULE.bazel");
        symlink("target.MODULE.bazel", module_path.as_path()).unwrap();

        assert_eq!(
            owner.apply(&request),
            Ok(RootModuleBootstrapApplyResult::Created(
                RootModuleBootstrapWarning
            ))
        );
        assert!(
            std::fs::symlink_metadata(module_path.as_path())
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::read(target).unwrap().as_slice(),
            ROOT_MODULE_BOOTSTRAP_REMINDER_BYTES
        );
    }
}
