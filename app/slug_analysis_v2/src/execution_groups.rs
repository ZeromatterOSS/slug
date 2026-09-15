/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_loading_v2::CoercedAttributeValue;
use slug_loading_v2::package::DeclaredExecGroup;
use slug_loading_v2::package::ToolchainTypeRequirement;

use crate::configured_attribute::ResolvedRuleAttribute;
use crate::exec_group::ConfiguredExecGroup;
use crate::result::ConfiguredActionOwnerContext;
use crate::result::RunfilesPackageClosureRow;
use crate::result::ToolchainTopology;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct NormalizedExecGroupRow {
    identity: ConfiguredExecGroup,
    requirements: Arc<[ToolchainTypeRequirement]>,
    constraints: Arc<[CanonicalLabel]>,
    target_exec_properties: BTreeMap<String, String>,
}

impl NormalizedExecGroupRow {
    pub(crate) fn identity(&self) -> &ConfiguredExecGroup {
        &self.identity
    }

    pub(crate) fn requirements(&self) -> &Arc<[ToolchainTypeRequirement]> {
        &self.requirements
    }

    pub(crate) fn constraints(&self) -> &Arc<[CanonicalLabel]> {
        &self.constraints
    }

    pub(crate) fn target_exec_properties(&self) -> &BTreeMap<String, String> {
        &self.target_exec_properties
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct NormalizedExecGroupCollection {
    rows: Arc<[NormalizedExecGroupRow]>,
    default_declared_requirements: Arc<[ToolchainTypeRequirement]>,
    target_default_exec_properties: BTreeMap<String, String>,
    use_auto_exec_groups: bool,
}

impl NormalizedExecGroupCollection {
    pub(crate) fn rows(&self) -> &[NormalizedExecGroupRow] {
        &self.rows
    }

    pub(crate) fn target_default_exec_properties(&self) -> &BTreeMap<String, String> {
        &self.target_default_exec_properties
    }
}

fn resolved_value<'a>(
    attributes: &'a [ResolvedRuleAttribute],
    name: &str,
) -> Option<&'a CoercedAttributeValue> {
    attributes
        .iter()
        .find(|attribute| attribute.declaration_name == name)
        .map(|attribute| &attribute.value)
}

fn normalize_constraints(
    values: impl IntoIterator<Item = CanonicalLabel>,
) -> Arc<[CanonicalLabel]> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    values.into()
}

const DEFAULT_EXEC_GROUP_NAME: &str = "default-exec-group";

fn automatic_group_identity(
    spelling: &str,
    target: &CanonicalLabel,
    default_requirements: &[ToolchainTypeRequirement],
) -> Option<ConfiguredExecGroup> {
    let label =
        CanonicalLabel::parse_with_package_context(spelling, target.package(), |requested| {
            if requested.is_empty() {
                Ok(CanonicalRepoName::root())
            } else {
                Err(format!("unknown apparent repository '@{requested}'"))
            }
        })
        .ok()?;
    default_requirements
        .iter()
        .any(|requirement| requirement.label() == &label)
        .then(|| ConfiguredExecGroup::automatic(label))
}

fn qualified_group_identity(
    spelling: &str,
    target: &CanonicalLabel,
    declared_names: &BTreeSet<&str>,
    default_requirements: &[ToolchainTypeRequirement],
    use_auto_exec_groups: bool,
) -> Option<ConfiguredExecGroup> {
    if spelling == DEFAULT_EXEC_GROUP_NAME {
        return Some(ConfiguredExecGroup::Default);
    }
    if declared_names.contains(spelling) {
        return Some(ConfiguredExecGroup::Named(CompactString::from(spelling)));
    }
    use_auto_exec_groups
        .then(|| automatic_group_identity(spelling, target, default_requirements))
        .flatten()
}

fn platform_group_prefix_matches(prefix: &str, group: &ConfiguredExecGroup) -> bool {
    match group {
        ConfiguredExecGroup::Default => prefix == DEFAULT_EXEC_GROUP_NAME,
        ConfiguredExecGroup::Named(name) => prefix == name,
        ConfiguredExecGroup::Automatic(label) => {
            if prefix == label.to_string() {
                return true;
            }
            let package = label.package().package().as_str();
            let short = format!("//{package}:{}", label.target());
            prefix == short || (label.package().repo().is_root() && prefix == format!("@{short}"))
        }
    }
}

pub(crate) fn normalize_execution_groups(
    target: &CanonicalLabel,
    default_requirements: &[ToolchainTypeRequirement],
    declarations: &[(CompactString, DeclaredExecGroup)],
    attributes: &[ResolvedRuleAttribute],
    native_auto_exec_groups: bool,
) -> Result<NormalizedExecGroupCollection, String> {
    let declared_names = declarations
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>();
    let explicit_auto = resolved_value(attributes, "_use_auto_exec_groups").map(|value| {
        if let CoercedAttributeValue::Boolean(value) = value {
            Ok(*value)
        } else {
            Err("resolved _use_auto_exec_groups is not boolean".to_owned())
        }
    });
    let use_auto_exec_groups = explicit_auto
        .transpose()?
        .unwrap_or(native_auto_exec_groups);
    let default_constraints = match resolved_value(attributes, "exec_compatible_with") {
        Some(CoercedAttributeValue::LabelList(values)) => values.clone(),
        Some(_) => return Err("resolved exec_compatible_with is not a label list".to_owned()),
        None => Arc::from([]),
    };
    let target_group_constraints = match resolved_value(attributes, "exec_group_compatible_with") {
        Some(CoercedAttributeValue::LabelListDict(values)) => values.clone(),
        Some(_) => {
            return Err("resolved exec_group_compatible_with is not a label-list dict".to_owned());
        }
        None => Arc::from([]),
    };
    let mut constraints_by_group = BTreeMap::new();
    for (name, constraints) in target_group_constraints.iter() {
        let Some(identity) = qualified_group_identity(
            name,
            target,
            &declared_names,
            default_requirements,
            use_auto_exec_groups,
        ) else {
            return Err(format!(
                "exec_group_compatible_with names unknown execution group '{name}'"
            ));
        };
        constraints_by_group.insert(identity, constraints);
    }

    let raw_properties = match resolved_value(attributes, "exec_properties") {
        Some(CoercedAttributeValue::StringDict(values)) => values.clone(),
        Some(_) => return Err("resolved exec_properties is not a string dict".to_owned()),
        None => Arc::from([]),
    };
    let mut default_properties = BTreeMap::new();
    let mut group_properties: BTreeMap<ConfiguredExecGroup, BTreeMap<String, String>> =
        BTreeMap::new();
    for (key, value) in raw_properties.iter() {
        if let Some((group, property)) = key.split_once('.') {
            let Some(identity) = qualified_group_identity(
                group,
                target,
                &declared_names,
                default_requirements,
                use_auto_exec_groups,
            ) else {
                return Err(format!(
                    "exec_properties names unknown execution group '{group}'"
                ));
            };
            if property.is_empty() {
                return Err(format!(
                    "exec_properties has empty property for group '{group}'"
                ));
            }
            group_properties
                .entry(identity)
                .or_default()
                .insert(property.to_owned(), value.to_string());
        } else {
            default_properties.insert(key.to_string(), value.to_string());
        }
    }

    let default_declared_requirements: Arc<[ToolchainTypeRequirement]> =
        Arc::from(default_requirements);
    let mut rows = Vec::with_capacity(
        1 + declarations.len() + usize::from(use_auto_exec_groups) * default_requirements.len(),
    );
    rows.push(NormalizedExecGroupRow {
        identity: ConfiguredExecGroup::Default,
        requirements: if use_auto_exec_groups {
            Arc::from([])
        } else {
            default_declared_requirements.clone()
        },
        constraints: normalize_constraints(
            default_constraints.iter().cloned().chain(
                constraints_by_group
                    .remove(&ConfiguredExecGroup::Default)
                    .map(|constraints| constraints.as_ref().to_vec())
                    .unwrap_or_default(),
            ),
        ),
        target_exec_properties: group_properties
            .remove(&ConfiguredExecGroup::Default)
            .unwrap_or_default(),
    });
    for (name, declaration) in declarations {
        let identity = ConfiguredExecGroup::Named(name.clone());
        let target_constraints = constraints_by_group
            .remove(&identity)
            .map(|constraints| constraints.as_ref().to_vec())
            .unwrap_or_default();
        rows.push(NormalizedExecGroupRow {
            identity: identity.clone(),
            requirements: Arc::from(declaration.toolchains()),
            constraints: normalize_constraints(
                declaration
                    .exec_compatible_with()
                    .iter()
                    .cloned()
                    .chain(target_constraints),
            ),
            target_exec_properties: group_properties.remove(&identity).unwrap_or_default(),
        });
    }
    if use_auto_exec_groups {
        rows.extend(default_requirements.iter().cloned().map(|requirement| {
            let identity = ConfiguredExecGroup::automatic(requirement.label().clone());
            let target_constraints = constraints_by_group
                .remove(&identity)
                .map(|constraints| constraints.as_ref().to_vec())
                .unwrap_or_default();
            NormalizedExecGroupRow {
                identity: identity.clone(),
                requirements: Arc::from([requirement]),
                constraints: normalize_constraints(
                    default_constraints
                        .iter()
                        .cloned()
                        .chain(target_constraints),
                ),
                target_exec_properties: group_properties.remove(&identity).unwrap_or_default(),
            }
        }));
    }
    Ok(NormalizedExecGroupCollection {
        rows: rows.into(),
        default_declared_requirements,
        target_default_exec_properties: default_properties,
        use_auto_exec_groups,
    })
}

pub(crate) fn effective_exec_properties(
    platform: &Arc<[(CompactString, CompactString)]>,
    group: &ConfiguredExecGroup,
    target_default: &BTreeMap<String, String>,
    target_group: &BTreeMap<String, String>,
) -> Arc<[(CompactString, CompactString)]> {
    let mut merged = BTreeMap::new();
    let mut platform_group = BTreeMap::new();
    for (key, value) in platform.iter() {
        match key.split_once('.') {
            None => {
                merged.insert(key.clone(), value.clone());
            }
            Some((prefix, property)) if platform_group_prefix_matches(prefix, group) => {
                platform_group.insert(CompactString::from(property), value.clone());
            }
            Some(_) => {}
        }
    }
    merged.extend(platform_group);
    merged.extend(
        target_default
            .iter()
            .chain(target_group)
            .map(|(key, value)| (CompactString::from(key), CompactString::from(value))),
    );
    merged.into_iter().collect::<Vec<_>>().into()
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredExecGroupRow {
    identity: ConfiguredExecGroup,
    requirements: Arc<[ToolchainTypeRequirement]>,
    constraints: Arc<[CanonicalLabel]>,
    action_context: Arc<ConfiguredActionOwnerContext>,
    runfiles_packages: Arc<[RunfilesPackageClosureRow]>,
}

impl ConfiguredExecGroupRow {
    pub fn new(
        identity: ConfiguredExecGroup,
        requirements: Arc<[ToolchainTypeRequirement]>,
        constraints: Arc<[CanonicalLabel]>,
        action_context: Arc<ConfiguredActionOwnerContext>,
    ) -> Result<Self, String> {
        if action_context.exec_group() != &identity {
            return Err("configured execution group has mismatched action context".to_owned());
        }
        Ok(Self {
            identity,
            requirements,
            constraints,
            action_context,
            runfiles_packages: Arc::from([]),
        })
    }

    pub(crate) fn from_normalized(
        normalized: &NormalizedExecGroupRow,
        action_context: Arc<ConfiguredActionOwnerContext>,
        runfiles_packages: Arc<[RunfilesPackageClosureRow]>,
    ) -> Result<Self, String> {
        let mut row = Self::new(
            normalized.identity.clone(),
            normalized.requirements.clone(),
            normalized.constraints.clone(),
            action_context,
        )?;
        row.runfiles_packages = runfiles_packages;
        Ok(row)
    }

    pub fn identity(&self) -> &ConfiguredExecGroup {
        &self.identity
    }

    pub fn requirements(&self) -> &[ToolchainTypeRequirement] {
        &self.requirements
    }

    pub fn constraints(&self) -> &[CanonicalLabel] {
        &self.constraints
    }

    pub fn action_context(&self) -> &Arc<ConfiguredActionOwnerContext> {
        &self.action_context
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ConfiguredExecGroupCollection {
    rows: Arc<[ConfiguredExecGroupRow]>,
    default_declared_requirements: Arc<[ToolchainTypeRequirement]>,
    toolchain_topology: ToolchainTopology,
    use_auto_exec_groups: bool,
}

impl ConfiguredExecGroupCollection {
    pub fn new(
        rows: Vec<ConfiguredExecGroupRow>,
        default_declared_requirements: Arc<[ToolchainTypeRequirement]>,
        toolchain_topology: ToolchainTopology,
        use_auto_exec_groups: bool,
    ) -> Result<Self, String> {
        let identities = rows
            .iter()
            .map(|row| row.identity().clone())
            .collect::<BTreeSet<_>>();
        if identities.len() != rows.len()
            || rows
                .iter()
                .filter(|row| row.identity() == &ConfiguredExecGroup::Default)
                .count()
                != 1
        {
            return Err("configured execution groups require one unique Default row".to_owned());
        }
        Ok(Self {
            rows: rows.into(),
            default_declared_requirements,
            toolchain_topology,
            use_auto_exec_groups,
        })
    }

    pub(crate) fn from_normalized(
        rows: Vec<ConfiguredExecGroupRow>,
        normalized: &NormalizedExecGroupCollection,
        toolchain_topology: ToolchainTopology,
    ) -> Result<Self, String> {
        Self::new(
            rows,
            normalized.default_declared_requirements.clone(),
            toolchain_topology,
            normalized.use_auto_exec_groups,
        )
    }

    pub fn rows(&self) -> &[ConfiguredExecGroupRow] {
        &self.rows
    }

    pub fn row(&self, identity: &ConfiguredExecGroup) -> Option<&ConfiguredExecGroupRow> {
        self.rows.iter().find(|row| row.identity() == identity)
    }

    pub fn named(&self, name: &str) -> Option<&ConfiguredExecGroupRow> {
        self.row(&ConfiguredExecGroup::Named(CompactString::from(name)))
    }

    pub fn default_declared_requirements(&self) -> &[ToolchainTypeRequirement] {
        &self.default_declared_requirements
    }

    pub fn use_auto_exec_groups(&self) -> bool {
        self.use_auto_exec_groups
    }

    pub fn toolchain_topology(&self) -> &ToolchainTopology {
        &self.toolchain_topology
    }
}

#[cfg(test)]
mod tests {
    use slug_loading_v2::AttributeKind;

    use super::*;

    fn label(value: &str) -> CanonicalLabel {
        CanonicalLabel::parse(value).unwrap()
    }

    fn requirement(value: &str) -> ToolchainTypeRequirement {
        ToolchainTypeRequirement::new(label(value), true)
    }

    fn attribute(
        name: &str,
        kind: AttributeKind,
        value: CoercedAttributeValue,
    ) -> ResolvedRuleAttribute {
        ResolvedRuleAttribute {
            declaration_name: name.into(),
            kind,
            sequence: false,
            value,
        }
    }

    #[test]
    fn normalization_preserves_named_constraint_only_and_automatic_groups() {
        let default = [requirement("@@//tc:one"), requirement("@@//tc:two")];
        let declarations = [(
            CompactString::new("link"),
            DeclaredExecGroup::new(Arc::from([]), Arc::from([label("@@//constraints:linux")])),
        )];
        let attributes = [
            attribute(
                "exec_compatible_with",
                AttributeKind::LabelList,
                CoercedAttributeValue::LabelList(Arc::from([label("@@//constraints:x86")])),
            ),
            attribute(
                "_use_auto_exec_groups",
                AttributeKind::Boolean,
                CoercedAttributeValue::Boolean(true),
            ),
        ];
        let normalized = normalize_execution_groups(
            &label("@@//:request"),
            &default,
            &declarations,
            &attributes,
            false,
        )
        .unwrap();
        assert!(normalized.rows()[0].requirements().is_empty());
        assert_eq!(
            normalized.rows()[1].identity(),
            &ConfiguredExecGroup::Named("link".into())
        );
        assert!(normalized.rows()[1].requirements().is_empty());
        assert_eq!(
            normalized.rows()[1].constraints().as_ref(),
            &[label("@@//constraints:linux")]
        );
        assert_eq!(normalized.rows().len(), 4);
        assert_eq!(
            normalized.rows()[2].identity(),
            &ConfiguredExecGroup::automatic(label("@@//tc:one"))
        );
    }

    #[test]
    fn unknown_target_group_prefixes_fail_and_unknown_platform_prefixes_are_unused() {
        let declarations = [(
            CompactString::new("link"),
            DeclaredExecGroup::new(Arc::from([]), Arc::from([])),
        )];
        let bad = [attribute(
            "exec_properties",
            AttributeKind::StringDict,
            CoercedAttributeValue::StringDict(Arc::from([(
                "missing.container".into(),
                "bad".into(),
            )])),
        )];
        assert!(
            normalize_execution_groups(&label("@@//:request"), &[], &declarations, &bad, false,)
                .is_err()
        );
        let platform = Arc::from([
            ("missing.container".into(), "unused".into()),
            ("link.container".into(), "selected".into()),
        ]);
        assert_eq!(
            effective_exec_properties(
                &platform,
                &ConfiguredExecGroup::Named("link".into()),
                &BTreeMap::new(),
                &BTreeMap::new(),
            )
            .as_ref(),
            &[(
                CompactString::new("container"),
                CompactString::new("selected")
            )]
        );
    }

    #[test]
    fn automatic_constraint_keys_are_canonicalized_and_restricted_to_default_toolchains() {
        let default = [
            requirement("@@//rule:first"),
            requirement("@@//rule:second"),
        ];
        let attributes = [
            attribute(
                "_use_auto_exec_groups",
                AttributeKind::Boolean,
                CoercedAttributeValue::Boolean(true),
            ),
            attribute(
                "exec_group_compatible_with",
                AttributeKind::LabelListDict,
                CoercedAttributeValue::LabelListDict(Arc::from([
                    (
                        "//rule:first".into(),
                        Arc::from([label("@@//constraints:first")]),
                    ),
                    (
                        "@//rule:second".into(),
                        Arc::from([label("@@//constraints:second")]),
                    ),
                ])),
            ),
        ];
        let normalized = normalize_execution_groups(
            &label("@@//consumer:request"),
            &default,
            &[],
            &attributes,
            false,
        )
        .unwrap();
        assert_eq!(
            normalized.rows()[1].constraints().as_ref(),
            &[label("@@//constraints:first")]
        );
        assert_eq!(
            normalized.rows()[2].constraints().as_ref(),
            &[label("@@//constraints:second")]
        );

        let unknown = [
            attributes[0].clone(),
            attribute(
                "exec_group_compatible_with",
                AttributeKind::LabelListDict,
                CoercedAttributeValue::LabelListDict(Arc::from([(
                    "//rule:missing".into(),
                    Arc::from([label("@@//constraints:first")]),
                )])),
            ),
        ];
        assert!(
            normalize_execution_groups(
                &label("@@//consumer:request"),
                &default,
                &[],
                &unknown,
                false,
            )
            .unwrap_err()
            .contains("unknown execution group '//rule:missing'")
        );
    }
}
