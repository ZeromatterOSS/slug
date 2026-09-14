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

pub(crate) fn normalize_execution_groups(
    default_requirements: &[ToolchainTypeRequirement],
    declarations: &[(CompactString, DeclaredExecGroup)],
    attributes: &[ResolvedRuleAttribute],
    native_auto_exec_groups: bool,
) -> Result<NormalizedExecGroupCollection, String> {
    let declared_names = declarations
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>();
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
    for (name, _) in target_group_constraints.iter() {
        if !declared_names.contains(name.as_str()) {
            return Err(format!(
                "exec_group_compatible_with names unknown execution group '{name}'"
            ));
        }
    }

    let raw_properties = match resolved_value(attributes, "exec_properties") {
        Some(CoercedAttributeValue::StringDict(values)) => values.clone(),
        Some(_) => return Err("resolved exec_properties is not a string dict".to_owned()),
        None => Arc::from([]),
    };
    let mut default_properties = BTreeMap::new();
    let mut group_properties: BTreeMap<&str, BTreeMap<String, String>> = BTreeMap::new();
    for (key, value) in raw_properties.iter() {
        if let Some((group, property)) = key.split_once('.') {
            if !declared_names.contains(group) {
                return Err(format!(
                    "exec_properties names unknown execution group '{group}'"
                ));
            }
            if property.is_empty() {
                return Err(format!(
                    "exec_properties has empty property for group '{group}'"
                ));
            }
            group_properties
                .entry(group)
                .or_default()
                .insert(property.to_owned(), value.to_string());
        } else {
            default_properties.insert(key.to_string(), value.to_string());
        }
    }

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
        constraints: normalize_constraints(default_constraints.iter().cloned()),
        target_exec_properties: BTreeMap::new(),
    });
    for (name, declaration) in declarations {
        let target_constraints = target_group_constraints
            .iter()
            .find_map(|(candidate, constraints)| (candidate == name).then_some(constraints))
            .into_iter()
            .flat_map(|constraints| constraints.iter())
            .cloned();
        rows.push(NormalizedExecGroupRow {
            identity: ConfiguredExecGroup::Named(name.clone()),
            requirements: Arc::from(declaration.toolchains()),
            constraints: normalize_constraints(
                declaration
                    .exec_compatible_with()
                    .iter()
                    .cloned()
                    .chain(target_constraints),
            ),
            target_exec_properties: group_properties.remove(name.as_str()).unwrap_or_default(),
        });
    }
    if use_auto_exec_groups {
        rows.extend(default_requirements.iter().cloned().map(|requirement| {
            NormalizedExecGroupRow {
                identity: ConfiguredExecGroup::automatic(requirement.label().clone()),
                requirements: Arc::from([requirement]),
                constraints: normalize_constraints(default_constraints.iter().cloned()),
                target_exec_properties: BTreeMap::new(),
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
            Some((prefix, property)) if matches!(group, ConfiguredExecGroup::Named(name) if name == prefix) =>
            {
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
        let normalized =
            normalize_execution_groups(&default, &declarations, &attributes, false).unwrap();
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
    fn property_precedence_target_default_beats_platform_group() {
        let platform = Arc::from([
            ("container".into(), "platform-default".into()),
            ("link.container".into(), "platform-group".into()),
        ]);
        let target_default =
            BTreeMap::from([("container".to_owned(), "target-default".to_owned())]);
        let properties = effective_exec_properties(
            &platform,
            &ConfiguredExecGroup::Named("link".into()),
            &target_default,
            &BTreeMap::new(),
        );
        assert_eq!(
            properties.as_ref(),
            &[(
                CompactString::new("container"),
                CompactString::new("target-default")
            )]
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
        assert!(normalize_execution_groups(&[], &declarations, &bad, false).is_err());
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
}
