/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select either.
 */

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use starlark_map::small_set::SmallSet;

/// One normalized, repository-relative file authored by the admitted
/// `repository_ctx.file` subset.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct GeneratedRepositoryFileEffect {
    path: CompactString,
    content: Arc<[u8]>,
    executable: bool,
}

impl GeneratedRepositoryFileEffect {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn content(&self) -> &[u8] {
        &self.content
    }

    pub fn executable(&self) -> bool {
        self.executable
    }
}

/// Ordered, structural output of one selected repository-rule invocation.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct GeneratedRepositoryFileEffectPlan(Arc<[GeneratedRepositoryFileEffect]>);

impl GeneratedRepositoryFileEffectPlan {
    pub fn effects(&self) -> &[GeneratedRepositoryFileEffect] {
        &self.0
    }

    pub fn builder() -> GeneratedRepositoryFileEffectPlanBuilder {
        GeneratedRepositoryFileEffectPlanBuilder {
            seen: SmallSet::new(),
            effects: Vec::new(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum GeneratedRepositoryFileEffectPlanError {
    InvalidPath(CompactString),
    RepeatedPath(CompactString),
}

impl GeneratedRepositoryFileEffectPlan {
    pub fn build(
        effects: impl IntoIterator<Item = (CompactString, Arc<[u8]>, bool)>,
    ) -> Result<Self, GeneratedRepositoryFileEffectPlanError> {
        let mut builder = Self::builder();
        for (path, content, executable) in effects {
            builder.push(path, content, executable)?;
        }
        Ok(builder.finish())
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct GeneratedRepositoryFileEffectPlanBuilder {
    seen: SmallSet<CompactString>,
    effects: Vec<GeneratedRepositoryFileEffect>,
}

impl GeneratedRepositoryFileEffectPlanBuilder {
    pub fn push(
        &mut self,
        path: CompactString,
        content: Arc<[u8]>,
        executable: bool,
    ) -> Result<(), GeneratedRepositoryFileEffectPlanError> {
        if !valid_path(&path) {
            return Err(GeneratedRepositoryFileEffectPlanError::InvalidPath(path));
        }
        if !self.seen.insert(path.clone()) {
            return Err(GeneratedRepositoryFileEffectPlanError::RepeatedPath(path));
        }
        self.effects.push(GeneratedRepositoryFileEffect {
            path,
            content,
            executable,
        });
        Ok(())
    }

    pub fn finish(self) -> GeneratedRepositoryFileEffectPlan {
        GeneratedRepositoryFileEffectPlan(self.effects.into())
    }
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && !path.ends_with('/')
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Arc;

    use compact_str::CompactString;

    use super::GeneratedRepositoryFileEffectPlan;
    use super::GeneratedRepositoryFileEffectPlanError;

    #[test]
    fn plan_preserves_order_bytes_executable_and_structural_identity() {
        let plan = GeneratedRepositoryFileEffectPlan::build([
            (
                CompactString::new("BUILD.bazel"),
                Arc::from(&b"exports_files([])\n"[..]),
                true,
            ),
            (
                CompactString::new("generated.txt"),
                Arc::from(&b"hello\n"[..]),
                false,
            ),
        ])
        .unwrap();
        assert_eq!(plan.effects().len(), 2);
        assert_eq!(plan.effects()[0].path(), "BUILD.bazel");
        assert_eq!(plan.effects()[0].content(), b"exports_files([])\n");
        assert!(plan.effects()[0].executable());
        assert_eq!(plan.effects()[1].path(), "generated.txt");
        assert!(!plan.effects()[1].executable());
        let mut left = DefaultHasher::new();
        plan.hash(&mut left);
        let swapped = GeneratedRepositoryFileEffectPlan::build([
            (
                CompactString::new("generated.txt"),
                Arc::from(&b"hello\n"[..]),
                false,
            ),
            (
                CompactString::new("BUILD.bazel"),
                Arc::from(&b"exports_files([])\n"[..]),
                true,
            ),
        ])
        .unwrap();
        assert_ne!(plan, swapped);
        let mut right = DefaultHasher::new();
        swapped.hash(&mut right);
        assert_ne!(left.finish(), right.finish());
    }

    #[test]
    fn plan_rejects_non_normal_and_repeated_paths_at_first_occurrence() {
        for path in ["", "/absolute", "a/../b", "a//b", "a\\b", "a/"] {
            assert!(matches!(
                GeneratedRepositoryFileEffectPlan::build([(
                    CompactString::new(path),
                    Arc::from(&b"x"[..]),
                    true,
                )]),
                Err(GeneratedRepositoryFileEffectPlanError::InvalidPath(_))
            ));
        }
        assert!(matches!(
            GeneratedRepositoryFileEffectPlan::build([
                (CompactString::new("a"), Arc::from(&b"one"[..]), true),
                (CompactString::new("a"), Arc::from(&b"two"[..]), false),
            ]),
            Err(GeneratedRepositoryFileEffectPlanError::RepeatedPath(path)) if path == "a"
        ));
    }
}
