/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 */

use std::sync::Arc;

use allocative::Allocative;
use slug_analysis_v2::ConfiguredAction;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutputKind;

/// The producer-owned, root-set-specific result of action output validation.
#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub(super) struct ValidatedActionClosure {
    owners: Arc<[Arc<ConfiguredNodeResult>]>,
    duplicate_execution_actions: Arc<[ActionCoordinate]>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Allocative)]
pub(super) struct ActionCoordinate {
    owner: u32,
    action: u32,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub(super) enum ConfiguredActionClosureError {
    OutputConflict {
        path: Arc<str>,
        first_owner: ConfiguredTargetKey,
        second_owner: ConfiguredTargetKey,
    },
    OutputPrefixConflict {
        path: Arc<str>,
        first_owner: ConfiguredTargetKey,
        second_owner: ConfiguredTargetKey,
    },
    UnsupportedEquivalence {
        path: Arc<str>,
        first_owner: ConfiguredTargetKey,
        second_owner: ConfiguredTargetKey,
    },
    InvalidOutputRoot,
}

struct OutputRow<'a> {
    coordinate: ActionCoordinate,
    output: &'a slug_build_api_v2::ActionOutput,
    action: &'a ConfiguredAction,
    root: &'a [u8],
    ordinal: u64,
}

impl ValidatedActionClosure {
    pub(super) fn empty() -> Self {
        Self {
            owners: Arc::from([]),
            duplicate_execution_actions: Arc::from([]),
        }
    }

    pub(super) fn new(
        owners: Arc<[Arc<ConfiguredNodeResult>]>,
    ) -> Result<Self, ConfiguredActionClosureError> {
        let mut rows = collect_output_rows(&owners)?;
        rows.sort_by(|left, right| {
            left.root
                .cmp(right.root)
                .then_with(|| {
                    left.output
                        .path()
                        .split('/')
                        .cmp(right.output.path().split('/'))
                })
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        });

        let mut duplicate_execution_actions = validate_exact_outputs(&rows)?;
        validate_prefix_outputs(&rows)?;
        duplicate_execution_actions.sort_unstable();
        duplicate_execution_actions.dedup();
        Ok(Self {
            owners,
            duplicate_execution_actions: duplicate_execution_actions.into(),
        })
    }

    pub(super) fn owners(&self) -> &[Arc<ConfiguredNodeResult>] {
        &self.owners
    }

    pub(super) fn execution_representative(&self, owner: usize, action: usize) -> bool {
        let coordinate = ActionCoordinate {
            owner: u32::try_from(owner).expect("closure owner index fits u32"),
            action: u32::try_from(action).expect("closure action index fits u32"),
        };
        self.duplicate_execution_actions
            .binary_search(&coordinate)
            .is_err()
    }
}

impl std::ops::Deref for ValidatedActionClosure {
    type Target = [Arc<ConfiguredNodeResult>];

    fn deref(&self) -> &Self::Target {
        &self.owners
    }
}

fn collect_output_rows<'a>(
    owners: &'a [Arc<ConfiguredNodeResult>],
) -> Result<Vec<OutputRow<'a>>, ConfiguredActionClosureError> {
    let mut rows = Vec::new();
    let mut ordinal = 0u64;
    for (owner_index, owner) in owners.iter().enumerate() {
        let root = owner
            .configured_target_key()
            .and_then(|key| key.configuration().slug_configuration())
            .map(|configuration| configuration.canonical_bytes())
            .ok_or(ConfiguredActionClosureError::InvalidOutputRoot)?;
        for (action_index, action) in owner.actions().iter().enumerate() {
            let coordinate = ActionCoordinate {
                owner: u32::try_from(owner_index).expect("closure owner count fits u32"),
                action: u32::try_from(action_index).expect("closure action count fits u32"),
            };
            for output in action.outputs() {
                rows.push(OutputRow {
                    coordinate,
                    output,
                    action,
                    root,
                    ordinal,
                });
                ordinal += 1;
            }
        }
    }
    Ok(rows)
}

fn validate_exact_outputs(
    rows: &[OutputRow<'_>],
) -> Result<Vec<ActionCoordinate>, ConfiguredActionClosureError> {
    let mut duplicates = Vec::new();
    let mut group_start = 0usize;
    while group_start < rows.len() {
        let mut group_end = group_start + 1;
        while group_end < rows.len()
            && rows[group_start].root == rows[group_end].root
            && rows[group_start].output.path() == rows[group_end].output.path()
        {
            group_end += 1;
        }
        let first = &rows[group_start];
        for row in &rows[group_start + 1..group_end] {
            let error = |unsupported| {
                if unsupported {
                    ConfiguredActionClosureError::UnsupportedEquivalence {
                        path: Arc::from(row.output.path()),
                        first_owner: first.action.context().owner().clone(),
                        second_owner: row.action.context().owner().clone(),
                    }
                } else {
                    ConfiguredActionClosureError::OutputConflict {
                        path: Arc::from(row.output.path()),
                        first_owner: first.action.context().owner().clone(),
                        second_owner: row.action.context().owner().clone(),
                    }
                }
            };
            if !scalar_file_write(first.action) || !scalar_file_write(row.action) {
                return Err(error(true));
            }
            if !equivalent_file_write(first.action, row.action) {
                return Err(error(false));
            }
            if row.coordinate != first.coordinate {
                duplicates.push(row.coordinate);
            }
        }
        group_start = group_end;
    }
    Ok(duplicates)
}

fn validate_prefix_outputs(rows: &[OutputRow<'_>]) -> Result<(), ConfiguredActionClosureError> {
    let mut ancestors: Vec<usize> = Vec::new();
    let mut previous_path: Option<(&[u8], &str)> = None;
    for (index, row) in rows.iter().enumerate() {
        if previous_path.is_some_and(|(root, path)| root == row.root && path == row.output.path()) {
            continue;
        }
        previous_path = Some((row.root, row.output.path()));
        while ancestors.last().is_some_and(|ancestor| {
            let ancestor = &rows[*ancestor];
            ancestor.root != row.root
                || !strict_path_prefix(ancestor.output.path(), row.output.path())
        }) {
            ancestors.pop();
        }
        if let Some(ancestor) = ancestors.last().map(|index| &rows[*index])
            && !runfiles_manifest_exemption(ancestor.output, row.output)
        {
            return Err(ConfiguredActionClosureError::OutputPrefixConflict {
                path: Arc::from(row.output.path()),
                first_owner: ancestor.action.context().owner().clone(),
                second_owner: row.action.context().owner().clone(),
            });
        }
        ancestors.push(index);
    }
    Ok(())
}

fn scalar_file_write(action: &ConfiguredAction) -> bool {
    matches!(action.kind(), ActionKind::Write { .. })
        && matches!(action.outputs(), [output] if output.kind() == ActionOutputKind::File)
        && action.argv().is_empty()
        && action.inputs().is_empty()
        && action.tools().is_empty()
        && action.param_files().is_empty()
        && action.env().is_empty()
        && action.execution_requirements().is_empty()
        && action.exec_properties().is_empty()
        && action.progress_message().is_none()
        && (action.context().execution_platform().is_some()
            == action.context().raw_platform_fact().is_some())
}

fn equivalent_file_write(left: &ConfiguredAction, right: &ConfiguredAction) -> bool {
    matches!(
        (left.kind(), right.kind()),
        (
            ActionKind::Write { content: left_content, is_executable: left_executable },
            ActionKind::Write { content: right_content, is_executable: right_executable },
        ) if left.mnemonic() == right.mnemonic()
            && left_content == right_content
            && left_executable == right_executable
            && left.context().execution_platform().map(|platform| platform.label())
                == right.context().execution_platform().map(|platform| platform.label())
            && left.context().raw_platform_fact() == right.context().raw_platform_fact()
            && same_constraint_labels(left, right)
    )
}

fn same_constraint_labels(left: &ConfiguredAction, right: &ConfiguredAction) -> bool {
    left.context()
        .platform_constraints()
        .iter()
        .map(|constraint| {
            (
                constraint.constraint_value().label(),
                constraint.constraint_setting().label(),
            )
        })
        .eq(right
            .context()
            .platform_constraints()
            .iter()
            .map(|constraint| {
                (
                    constraint.constraint_value().label(),
                    constraint.constraint_setting().label(),
                )
            }))
}

fn strict_path_prefix(parent: &str, child: &str) -> bool {
    child.len() > parent.len()
        && child.starts_with(parent)
        && child.as_bytes().get(parent.len()) == Some(&b'/')
}

fn runfiles_manifest_exemption(
    parent: &slug_build_api_v2::ActionOutput,
    child: &slug_build_api_v2::ActionOutput,
) -> bool {
    parent.kind() == ActionOutputKind::RunfilesTree
        && child.path() == format!("{}/MANIFEST", parent.path())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use slug_analysis_v2::ConfigurationKey;
    use slug_analysis_v2::ConfiguredActionOwnerContext;
    use slug_analysis_v2::ConfiguredNodeResult;
    use slug_analysis_v2::ConfiguredTargetKey;
    use slug_build_api_v2::ActionKind;
    use slug_build_api_v2::ActionOutput;
    use slug_build_api_v2::ActionSpec;
    use slug_build_api_v2::ProviderCollection;
    use slug_build_api_v2::RunfilesPackageDepset;
    use slug_configuration_v2::SlugConfiguration;
    use slug_configuration_v2::native::host::AutoCpuToken;
    use slug_configuration_v2::native::host::HostConversionInputs;
    use slug_configuration_v2::native::host::HostPathFlavor;
    use slug_identity_v2::CanonicalLabel;

    use super::ActionOutputKind;
    use super::ConfiguredActionClosureError;
    use super::ValidatedActionClosure;
    use super::runfiles_manifest_exemption;
    use super::strict_path_prefix;

    fn node(label: &str, output: ActionOutput, kind: ActionKind) -> Arc<ConfiguredNodeResult> {
        let key = test_key(label, false);
        node_with_context(
            ConfiguredActionOwnerContext::unresolved_default(key).unwrap(),
            output,
            kind,
        )
    }

    fn test_key(label: &str, exec: bool) -> ConfiguredTargetKey {
        let host = HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap();
        ConfiguredTargetKey::new(
            CanonicalLabel::parse(label).unwrap(),
            ConfigurationKey::from_slug(if exec {
                SlugConfiguration::default_exec(&host).unwrap()
            } else {
                SlugConfiguration::default_target(&host).unwrap()
            }),
        )
    }

    fn node_with_context(
        context: ConfiguredActionOwnerContext,
        output: ActionOutput,
        kind: ActionKind,
    ) -> Arc<ConfiguredNodeResult> {
        Arc::new(
            ConfiguredNodeResult::new_rule(
                context.owner().clone(),
                ProviderCollection::from_values(Vec::new(), false).unwrap(),
                None,
                RunfilesPackageDepset::empty(),
            )
            .with_action_specs(
                vec![ActionSpec::new(kind, "test", vec![output])],
                vec![Arc::new(context)],
            )
            .unwrap(),
        )
    }

    #[test]
    fn configured_action_conflicts_prefixes_are_segment_strict() {
        assert!(strict_path_prefix("tree", "tree/child"));
        assert!(!strict_path_prefix("tree", "tree"));
        assert!(!strict_path_prefix("tree", "treehouse"));
    }

    #[test]
    fn configured_action_conflicts_only_exempt_runfiles_manifest() {
        let runfiles = ActionOutput::new("tree", ActionOutputKind::RunfilesTree);
        let directory = ActionOutput::new("tree", ActionOutputKind::Directory);
        let manifest = ActionOutput::new("tree/MANIFEST", ActionOutputKind::File);
        let nested = ActionOutput::new("tree/sub/MANIFEST", ActionOutputKind::File);
        assert!(runfiles_manifest_exemption(&runfiles, &manifest));
        assert!(!runfiles_manifest_exemption(&directory, &manifest));
        assert!(!runfiles_manifest_exemption(&runfiles, &nested));
    }

    #[test]
    fn configured_action_conflicts_inventory_prefix_and_typed_terminals() {
        let file = |path| ActionOutput::new(path, ActionOutputKind::File);
        let write = |content: &str| ActionKind::Write {
            content: content.into(),
            is_executable: false,
        };
        assert!(matches!(
            ValidatedActionClosure::new(Arc::from([
                node(
                    "@@//:directory",
                    ActionOutput::new("tree", ActionOutputKind::Directory),
                    write("a")
                ),
                node("@@//:file", file("tree/child"), write("b")),
            ])),
            Err(ConfiguredActionClosureError::OutputPrefixConflict { .. })
        ));
        assert!(matches!(
            ValidatedActionClosure::new(Arc::from([
                node("@@//:one", file("same"), ActionKind::Run),
                node("@@//:two", file("same"), ActionKind::Run),
            ])),
            Err(ConfiguredActionClosureError::UnsupportedEquivalence { .. })
        ));
        assert!(
            ValidatedActionClosure::new(Arc::from([
                node("@@//:one", file("one"), ActionKind::Run),
                node("@@//:two", file("two"), ActionKind::Run),
            ]))
            .is_ok()
        );
    }

    // Pinned Actions.java: exact registration precedes segment-prefix checks;
    // every output kind participates, with only RunfilesTree/MANIFEST exempt.
    #[test]
    fn configured_action_conflicts_complete_prefix_inventory_and_exact_order() {
        let file = |label, path| {
            node(
                label,
                ActionOutput::new(path, ActionOutputKind::File),
                ActionKind::Run,
            )
        };
        for kind in [
            ActionOutputKind::File,
            ActionOutputKind::Directory,
            ActionOutputKind::Symlink,
            ActionOutputKind::RunfilesTree,
        ] {
            let parent = node("@@//:outer", ActionOutput::new("a", kind), ActionKind::Run);
            assert!(matches!(
                ValidatedActionClosure::new(Arc::from([
                    parent.clone(),
                    file("@@//:nested", "a/child"),
                ])),
                Err(ConfiguredActionClosureError::OutputPrefixConflict { .. })
            ));
            let manifest = ValidatedActionClosure::new(Arc::from([
                parent.clone(),
                file("@@//:manifest", "a/MANIFEST"),
            ]));
            assert_eq!(manifest.is_ok(), kind == ActionOutputKind::RunfilesTree);
            let exact = ValidatedActionClosure::new(Arc::from([
                parent,
                file("@@//:nested", "a/child"),
                file("@@//:first", "z"),
                file("@@//:second", "z"),
            ]))
            .unwrap_err();
            assert!(
                matches!(exact, ConfiguredActionClosureError::UnsupportedEquivalence { path, first_owner, second_owner }
                if path.as_ref() == "z" && first_owner.label().target().as_str() == "first" && second_owner.label().target().as_str() == "second")
            );
        }
        let lexical = ValidatedActionClosure::new(Arc::from([
            file("@@//:z1", "z"),
            file("@@//:z2", "z"),
            file("@@//:b1", "b"),
            file("@@//:b2", "b"),
        ]))
        .unwrap_err();
        assert!(
            matches!(lexical, ConfiguredActionClosureError::UnsupportedEquivalence { path, .. } if path.as_ref() == "b")
        );
    }

    fn resolved_context(
        owner: &str,
        platform: &str,
        raw: &str,
        merged: &str,
        message: Option<&str>,
        constraints: &[(&str, &str)],
    ) -> ConfiguredActionOwnerContext {
        ConfiguredActionOwnerContext::new(
            test_key(owner, false),
            slug_analysis_v2::ConfiguredExecGroup::Default,
            test_key(platform, true),
            slug_analysis_v2::PlatformSemanticFact {
                exec_properties: Arc::from([("property".into(), raw.into())]),
                missing_toolchain_error: message.map(Arc::from),
            },
            &[("property".to_owned(), merged.to_owned())]
                .into_iter()
                .collect(),
            &Default::default(),
            constraints
                .iter()
                .map(|(value, setting)| {
                    slug_analysis_v2::ConfiguredActionPlatformConstraint::new(
                        test_key(value, true),
                        test_key(setting, true),
                    )
                })
                .collect(),
            None,
            slug_analysis_v2::ConfiguredActionAspectProvenance::Absent,
        )
        .unwrap()
    }

    fn write_node(
        context: ConfiguredActionOwnerContext,
        content: &str,
    ) -> Arc<ConfiguredNodeResult> {
        node_with_context(
            context,
            ActionOutput::new("shared", ActionOutputKind::File),
            ActionKind::Write {
                content: content.into(),
                is_executable: false,
            },
        )
    }

    #[test]
    fn configured_action_conflicts_compare_raw_platform_not_merged_owner_properties() {
        let make = |owner, platform, raw, merged, message, constraints: &[(&str, &str)]| {
            write_node(
                resolved_context(owner, platform, raw, merged, message, constraints),
                "same",
            )
        };
        let constraint = [("@@//:value", "@@//:setting")];
        let left = make(
            "@@//:left",
            "@@//:platform",
            "raw",
            "override",
            Some("message"),
            &constraint,
        );
        let equivalent = make(
            "@@//:right",
            "@@//:platform",
            "raw",
            "different override",
            Some("message"),
            &constraint,
        );
        assert_ne!(
            left.actions()[0].context().platform_fact(),
            equivalent.actions()[0].context().platform_fact()
        );
        let shared = ValidatedActionClosure::new(Arc::from([left.clone(), equivalent])).unwrap();
        assert!(shared.execution_representative(0, 0));
        assert!(!shared.execution_representative(1, 0));
        let changed_raw = make(
            "@@//:right",
            "@@//:platform",
            "changed",
            "override",
            Some("message"),
            &constraint,
        );
        assert_eq!(
            left.actions()[0].context().platform_fact(),
            changed_raw.actions()[0].context().platform_fact()
        );
        for right in [
            changed_raw,
            make(
                "@@//:right",
                "@@//:other",
                "raw",
                "override",
                Some("message"),
                &constraint,
            ),
            make(
                "@@//:right",
                "@@//:platform",
                "raw",
                "override",
                None,
                &constraint,
            ),
            make(
                "@@//:right",
                "@@//:platform",
                "raw",
                "override",
                Some("custom"),
                &constraint,
            ),
            make(
                "@@//:right",
                "@@//:platform",
                "raw",
                "override",
                Some("message"),
                &[("@@//:other_value", "@@//:setting")],
            ),
            make(
                "@@//:right",
                "@@//:platform",
                "raw",
                "override",
                Some("message"),
                &[("@@//:value", "@@//:other_setting")],
            ),
            write_node(
                ConfiguredActionOwnerContext::unresolved_default(test_key("@@//:right", false))
                    .unwrap(),
                "same",
            ),
        ] {
            assert!(matches!(
                ValidatedActionClosure::new(Arc::from([left.clone(), right])),
                Err(ConfiguredActionClosureError::OutputConflict { .. })
            ));
        }
        let ordered = [("@@//:v1", "@@//:s1"), ("@@//:v2", "@@//:s2")];
        let reverse = [ordered[1], ordered[0]];
        assert!(matches!(
            ValidatedActionClosure::new(Arc::from([
                make(
                    "@@//:left",
                    "@@//:platform",
                    "raw",
                    "override",
                    None,
                    &ordered
                ),
                make(
                    "@@//:right",
                    "@@//:platform",
                    "raw",
                    "override",
                    None,
                    &reverse
                ),
            ])),
            Err(ConfiguredActionClosureError::OutputConflict { .. })
        ));
    }

    #[test]
    fn configured_action_conflicts_unresolved_long_unicode_and_distinct_configuration() {
        let content = "éλ😀".repeat(300);
        let unresolved = |label, exec| {
            ConfiguredActionOwnerContext::unresolved_default(test_key(label, exec)).unwrap()
        };
        let left = write_node(unresolved("@@//:left", false), &content);
        let right = write_node(unresolved("@@//:right", false), &content);
        let shared = ValidatedActionClosure::new(Arc::from([right.clone(), left.clone()])).unwrap();
        assert!(shared.execution_representative(0, 0));
        assert!(!shared.execution_representative(1, 0));
        assert!(
            right.configured_file_write_actions().is_err(),
            "analysis sharing must not admit unresolved execution"
        );
        let distinct = ValidatedActionClosure::new(Arc::from([
            left,
            write_node(unresolved("@@//:right", true), "different"),
        ]))
        .unwrap();
        assert!(distinct.execution_representative(0, 0));
        assert!(distinct.execution_representative(1, 0));
    }
}
