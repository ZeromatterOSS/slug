/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Loading-owned, unconfigured attribute metadata.
//!
//! These values deliberately retain configurable structure.  In particular,
//! they are not an alternate spelling of a rule's aggregate dependency list:
//! Stage 8 will project reachable labels from this representation.

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use starlark::values::FrozenValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum AttributeKind {
    Label,
    LabelList,
    StringKeyedLabelDict,
    LabelKeyedStringDict,
    LabelListDict,
    Output,
    OutputList,
    String,
    StringList,
}

impl AttributeKind {
    pub(crate) fn reaches_labels(self) -> bool {
        !matches!(self, Self::String | Self::StringList)
    }

    pub(crate) fn contributes_ordinary_dependencies(self) -> bool {
        matches!(
            self,
            Self::Label
                | Self::LabelList
                | Self::StringKeyedLabelDict
                | Self::LabelKeyedStringDict
                | Self::LabelListDict
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct AttributeSchema {
    declaration_name: CompactString,
    query_name: CompactString,
    kind: AttributeKind,
    mandatory: bool,
    configurable: bool,
    label_reachable: bool,
    default: Option<Arc<CoercedAttributeValue>>,
    transition: Option<TransitionDefinition>,
}

impl AttributeSchema {
    pub(crate) fn new(
        declaration_name: impl Into<CompactString>,
        kind: AttributeKind,
        mandatory: bool,
        configurable: bool,
        default: Option<CoercedAttributeValue>,
        transition: Option<TransitionDefinition>,
    ) -> Self {
        let declaration_name = declaration_name.into();
        let query_name = declaration_name
            .strip_prefix('_')
            .map(|name| CompactString::from(format!("${name}")))
            .unwrap_or_else(|| declaration_name.clone());
        Self {
            declaration_name,
            query_name,
            kind,
            mandatory,
            configurable,
            label_reachable: kind.reaches_labels(),
            default: default.map(Arc::new),
            transition,
        }
    }

    pub fn declaration_name(&self) -> &str {
        &self.declaration_name
    }
    pub fn query_name(&self) -> &str {
        &self.query_name
    }
    pub fn kind(&self) -> AttributeKind {
        self.kind
    }
    pub fn mandatory(&self) -> bool {
        self.mandatory
    }
    pub fn configurable(&self) -> bool {
        self.configurable
    }
    pub fn dependency_reachable(&self) -> bool {
        self.label_reachable
    }
    pub fn default(&self) -> Option<&CoercedAttributeValue> {
        self.default.as_deref()
    }
    pub fn transition(&self) -> Option<&TransitionDefinition> {
        self.transition.as_ref()
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct TransitionDefinition {
    #[allocative(skip)]
    implementation: FrozenValue,
    output: CompactString,
}
impl TransitionDefinition {
    pub fn new(implementation: FrozenValue, output: impl Into<CompactString>) -> Self {
        Self {
            implementation,
            output: output.into(),
        }
    }
    pub fn implementation(&self) -> FrozenValue {
        self.implementation
    }
    pub fn output(&self) -> &str {
        &self.output
    }
}
impl PartialEq for TransitionDefinition {
    fn eq(&self, other: &Self) -> bool {
        self.output == other.output
    }
}
impl Eq for TransitionDefinition {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative)]
pub enum AttributeProvenance {
    Explicit,
    Default,
    Implicit,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct AttributeValue {
    pub declaration_name: CompactString,
    pub provenance: AttributeProvenance,
    pub value: Arc<CoercedAttributeValue>,
}

/// Immutable attribute data that an unconfigured query may inspect.
///
/// This stays owned by loading: query projections retain the already-coerced
/// value rather than creating a string rendering or a second attribute model.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct AttributeQueryValue {
    pub kind: AttributeKind,
    pub provenance: AttributeProvenance,
    pub value: Arc<CoercedAttributeValue>,
}

impl AttributeValue {
    pub fn query_value(&self, schema: &AttributeSchema) -> AttributeQueryValue {
        debug_assert_eq!(self.declaration_name, schema.declaration_name());
        AttributeQueryValue {
            kind: schema.kind(),
            provenance: self.provenance,
            value: self.value.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum CoercedAttributeValue {
    /// Bazel's optional scalar-label default is null; it is not a label.
    None,
    Label(CanonicalLabel),
    LabelList(Arc<[CanonicalLabel]>),
    String(CompactString),
    StringList(Arc<[CompactString]>),
    StringKeyedLabelDict(Arc<[(CompactString, CanonicalLabel)]>),
    LabelKeyedStringDict(Arc<[(CanonicalLabel, CompactString)]>),
    LabelListDict(Arc<[(CompactString, Arc<[CanonicalLabel]>)]>),
    Output(CanonicalLabel),
    OutputList(Arc<[CanonicalLabel]>),
    Selector {
        /// Condition labels deliberately remain separate from branch values;
        /// `getReachableLabels(..., false)` excludes these keys.
        branches: Arc<[(CanonicalLabel, Arc<CoercedAttributeValue>)]>,
        default: Option<Arc<CoercedAttributeValue>>,
    },
    Concatenation(Arc<CoercedAttributeValue>, Arc<CoercedAttributeValue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrCandidateError {
    left: &'static str,
    right: &'static str,
}

impl fmt::Display for AttrCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot concatenate attribute candidate types {} and {}",
            self.left, self.right
        )
    }
}

impl std::error::Error for AttrCandidateError {}

impl CoercedAttributeValue {
    /// Returns every Bazel-visible string candidate for an unconfigured `attr()` query.
    ///
    /// The strings are rendered only for this request. Loading continues to retain the
    /// typed value, including selector structure, rather than a second cached string
    /// representation. `None` is an unset optional scalar and therefore contributes no
    /// candidate.
    pub fn attr_visible_candidates(
        &self,
        render_label: impl Fn(&CanonicalLabel) -> CompactString,
    ) -> Result<Vec<CompactString>, AttrCandidateError> {
        Ok(expand_attr_candidates(self)?
            .into_iter()
            .map(|candidate| candidate.value.render(&render_label))
            .collect())
    }

    pub fn labels(&self, labels: &mut Vec<CanonicalLabel>) {
        match self {
            Self::Label(label) | Self::Output(label) => labels.push(label.clone()),
            Self::LabelList(values) | Self::OutputList(values) => {
                labels.extend(values.iter().cloned())
            }
            Self::StringKeyedLabelDict(values) => {
                labels.extend(values.iter().map(|(_, value)| value.clone()))
            }
            Self::LabelKeyedStringDict(values) => {
                labels.extend(values.iter().map(|(key, _)| key.clone()))
            }
            Self::LabelListDict(values) => {
                labels.extend(values.iter().flat_map(|(_, values)| values.iter().cloned()))
            }
            Self::Selector { branches, default } => {
                for (_, value) in branches.iter() {
                    value.labels(labels);
                }
                if let Some(default) = default {
                    default.labels(labels);
                }
            }
            Self::Concatenation(left, right) => {
                left.labels(labels);
                right.labels(labels);
            }
            Self::String(_) | Self::StringList(_) | Self::None => {}
        }
    }
}

impl AttributeQueryValue {
    /// Request-time candidates for a later ordinary-query `attr()` matcher.
    pub fn attr_visible_candidates(
        &self,
        render_label: impl Fn(&CanonicalLabel) -> CompactString,
    ) -> Result<Vec<CompactString>, AttrCandidateError> {
        self.value.attr_visible_candidates(render_label)
    }
}

/// A temporary typed value is necessary because configurable list and dictionary
/// concatenations are rendered only after their branches have been combined.
/// It deliberately does not escape `attr_visible_candidates`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AttrCandidateAtom<'a> {
    String(CompactString),
    Label(&'a CanonicalLabel),
}

impl AttrCandidateAtom<'_> {
    fn render(&self, render_label: &impl Fn(&CanonicalLabel) -> CompactString) -> CompactString {
        match self {
            Self::String(value) => value.clone(),
            Self::Label(label) => render_label(label),
        }
    }
}

#[derive(Debug, Clone)]
enum AttrCandidateValue<'a> {
    Scalar(AttrCandidateAtom<'a>),
    List(Vec<AttrCandidateAtom<'a>>),
    Dict(Vec<(AttrCandidateAtom<'a>, AttrCandidateValue<'a>)>),
}

impl AttrCandidateValue<'_> {
    fn render(self, render_label: &impl Fn(&CanonicalLabel) -> CompactString) -> CompactString {
        match self {
            Self::Scalar(value) => value.render(render_label),
            Self::List(values) => CompactString::new(format!(
                "[{}]",
                values
                    .iter()
                    .map(|value| value.render(render_label))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Self::Dict(entries) => CompactString::new(format!(
                "{{{}}}",
                entries
                    .into_iter()
                    .map(|(key, value)| {
                        format!(
                            "{}={}",
                            key.render(render_label),
                            value.render(render_label)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    fn shape(&self) -> &'static str {
        match self {
            Self::Scalar(AttrCandidateAtom::String(_)) => "string",
            Self::Scalar(AttrCandidateAtom::Label(_)) => "label",
            Self::List(_) => "list",
            Self::Dict(_) => "dictionary",
        }
    }

    fn concatenate(self, right: Self) -> Result<Self, AttrCandidateError> {
        let left_shape = self.shape();
        let right_shape = right.shape();
        match (self, right) {
            (Self::Scalar(left), Self::Scalar(right)) => match (left, right) {
                (AttrCandidateAtom::String(mut left), AttrCandidateAtom::String(right)) => {
                    left.push_str(&right);
                    Ok(Self::Scalar(AttrCandidateAtom::String(left)))
                }
                _ => Err(AttrCandidateError {
                    left: left_shape,
                    right: right_shape,
                }),
            },
            (Self::List(mut left), Self::List(right)) => {
                left.extend(right);
                Ok(Self::List(left))
            }
            (Self::Dict(mut left), Self::Dict(right)) => {
                // Bazel's dictionary type keeps the last value for a repeated key.
                // Replacing in place retains the original map's observable key order.
                for (key, value) in right {
                    if let Some((_, existing)) =
                        left.iter_mut().find(|(existing, _)| *existing == key)
                    {
                        *existing = value;
                    } else {
                        left.push((key, value));
                    }
                }
                Ok(Self::Dict(left))
            }
            _ => Err(AttrCandidateError {
                left: left_shape,
                right: right_shape,
            }),
        }
    }
}

#[derive(Clone)]
struct AttrCandidate<'a> {
    value: AttrCandidateValue<'a>,
    /// One entry for each selector key set encountered on the path. This is
    /// request-local bookkeeping: equal key sets must select the same condition,
    /// while distinct key sets form a cross product.
    bindings: Vec<SelectorBinding<'a>>,
}

#[derive(Clone)]
struct SelectorBinding<'a> {
    selector: SelectorKeySet<'a>,
    selected: Option<&'a CanonicalLabel>,
}

#[derive(Clone, Copy)]
struct SelectorKeySet<'a> {
    branches: &'a [(CanonicalLabel, Arc<CoercedAttributeValue>)],
}

impl PartialEq for SelectorKeySet<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.branches.len() == other.branches.len()
            && self
                .branches
                .iter()
                .all(|(left, _)| other.branches.iter().any(|(right, _)| left == right))
    }
}

impl Eq for SelectorKeySet<'_> {}

fn expand_attr_candidates(
    value: &CoercedAttributeValue,
) -> Result<Vec<AttrCandidate<'_>>, AttrCandidateError> {
    Ok(match value {
        CoercedAttributeValue::None => Vec::new(),
        CoercedAttributeValue::Label(label) | CoercedAttributeValue::Output(label) => {
            scalar_label_candidate(label)
        }
        CoercedAttributeValue::String(value) => scalar_string_candidate(value.clone()),
        CoercedAttributeValue::LabelList(values) | CoercedAttributeValue::OutputList(values) => {
            list_candidate(values.iter().map(AttrCandidateAtom::Label))
        }
        CoercedAttributeValue::StringList(values) => {
            list_candidate(values.iter().cloned().map(AttrCandidateAtom::String))
        }
        CoercedAttributeValue::StringKeyedLabelDict(values) => {
            dict_candidate(values.iter().map(|(key, value)| {
                (
                    AttrCandidateAtom::String(key.clone()),
                    AttrCandidateValue::Scalar(AttrCandidateAtom::Label(value)),
                )
            }))
        }
        CoercedAttributeValue::LabelKeyedStringDict(values) => {
            dict_candidate(values.iter().map(|(key, value)| {
                (
                    AttrCandidateAtom::Label(key),
                    AttrCandidateValue::Scalar(AttrCandidateAtom::String(value.clone())),
                )
            }))
        }
        CoercedAttributeValue::LabelListDict(values) => {
            dict_candidate(values.iter().map(|(key, values)| {
                (
                    AttrCandidateAtom::String(key.clone()),
                    AttrCandidateValue::List(values.iter().map(AttrCandidateAtom::Label).collect()),
                )
            }))
        }
        CoercedAttributeValue::Selector { branches, default } => {
            let selector = SelectorKeySet { branches };
            let mut candidates = Vec::new();
            for (condition, branch) in branches.iter() {
                candidates.extend(
                    expand_attr_candidates(branch)?
                        .into_iter()
                        .filter_map(|candidate| {
                            bind_selector(candidate, selector, Some(condition))
                        }),
                );
            }
            if let Some(default) = default {
                candidates.extend(
                    expand_attr_candidates(default)?
                        .into_iter()
                        .filter_map(|candidate| bind_selector(candidate, selector, None)),
                );
            }
            candidates
        }
        CoercedAttributeValue::Concatenation(left, right) => combine_attr_candidates(
            expand_attr_candidates(left)?,
            expand_attr_candidates(right)?,
        )?,
    })
}

fn bind_selector<'a>(
    mut candidate: AttrCandidate<'a>,
    selector: SelectorKeySet<'a>,
    selected: Option<&'a CanonicalLabel>,
) -> Option<AttrCandidate<'a>> {
    if let Some(existing) = candidate
        .bindings
        .iter()
        .find(|binding| binding.selector == selector)
    {
        return (existing.selected == selected).then_some(candidate);
    }
    candidate
        .bindings
        .push(SelectorBinding { selector, selected });
    Some(candidate)
}

fn scalar_string_candidate(value: CompactString) -> Vec<AttrCandidate<'static>> {
    vec![AttrCandidate {
        value: AttrCandidateValue::Scalar(AttrCandidateAtom::String(value)),
        bindings: Vec::new(),
    }]
}

fn scalar_label_candidate(label: &CanonicalLabel) -> Vec<AttrCandidate<'_>> {
    vec![AttrCandidate {
        value: AttrCandidateValue::Scalar(AttrCandidateAtom::Label(label)),
        bindings: Vec::new(),
    }]
}

fn list_candidate<'a>(
    values: impl IntoIterator<Item = AttrCandidateAtom<'a>>,
) -> Vec<AttrCandidate<'a>> {
    vec![AttrCandidate {
        value: AttrCandidateValue::List(values.into_iter().collect()),
        bindings: Vec::new(),
    }]
}

fn dict_candidate<'a>(
    entries: impl IntoIterator<Item = (AttrCandidateAtom<'a>, AttrCandidateValue<'a>)>,
) -> Vec<AttrCandidate<'a>> {
    vec![AttrCandidate {
        value: AttrCandidateValue::Dict(entries.into_iter().collect()),
        bindings: Vec::new(),
    }]
}

fn combine_attr_candidates<'a>(
    left: Vec<AttrCandidate<'a>>,
    right: Vec<AttrCandidate<'a>>,
) -> Result<Vec<AttrCandidate<'a>>, AttrCandidateError> {
    let mut combined = Vec::with_capacity(left.len().saturating_mul(right.len()));
    for left_candidate in left {
        for right_candidate in &right {
            let mut bindings = left_candidate.bindings.clone();
            let mut compatible = true;
            for right_binding in &right_candidate.bindings {
                if let Some(left_binding) = bindings
                    .iter()
                    .find(|left_binding| left_binding.selector == right_binding.selector)
                {
                    if left_binding.selected != right_binding.selected {
                        compatible = false;
                        break;
                    }
                } else {
                    bindings.push(right_binding.clone());
                }
            }
            if compatible {
                combined.push(AttrCandidate {
                    value: left_candidate
                        .value
                        .clone()
                        .concatenate(right_candidate.value.clone())?,
                    bindings,
                });
            }
        }
    }
    Ok(combined)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use compact_str::CompactString;
    use slug_identity_v2::CanonicalLabel;

    use super::AttributeKind;
    use super::AttributeProvenance;
    use super::AttributeQueryValue;
    use super::AttributeSchema;
    use super::AttributeValue;
    use super::CoercedAttributeValue;

    fn label(value: &str) -> CanonicalLabel {
        CanonicalLabel::parse(value).unwrap()
    }

    fn render_bazel_label(label: &CanonicalLabel) -> CompactString {
        if label.package().repo().is_root() {
            CompactString::new(format!(
                "//{}:{}",
                label.package().package(),
                label.target()
            ))
        } else {
            CompactString::new(label.to_string())
        }
    }

    fn string_selector(branches: &[(&str, &str)]) -> CoercedAttributeValue {
        CoercedAttributeValue::Selector {
            branches: branches
                .iter()
                .map(|(condition, value)| {
                    (
                        label(condition),
                        Arc::new(CoercedAttributeValue::String(CompactString::new(value))),
                    )
                })
                .collect::<Vec<_>>()
                .into(),
            default: None,
        }
    }

    fn string_value(value: &str) -> Arc<CoercedAttributeValue> {
        Arc::new(CoercedAttributeValue::String(CompactString::new(value)))
    }

    #[test]
    fn query_value_keeps_the_loading_value_structure_order_and_provenance() {
        let schema =
            AttributeSchema::new("chosen", AttributeKind::LabelList, false, true, None, None);
        let before = CanonicalLabel::parse("@@//pkg:before").unwrap();
        let selected = CanonicalLabel::parse("@@//pkg:selected").unwrap();
        let fallback = CanonicalLabel::parse("@@//pkg:fallback").unwrap();
        let retained = Arc::new(CoercedAttributeValue::Concatenation(
            Arc::new(CoercedAttributeValue::LabelList(Arc::from(
                [before.clone()],
            ))),
            Arc::new(CoercedAttributeValue::Selector {
                branches: Arc::from([(
                    CanonicalLabel::parse("@@//conditions:enabled").unwrap(),
                    Arc::new(CoercedAttributeValue::LabelList(Arc::from([
                        selected.clone()
                    ]))),
                )]),
                default: Some(Arc::new(CoercedAttributeValue::LabelList(Arc::from([
                    fallback.clone(),
                ])))),
            }),
        ));
        let value = AttributeValue {
            declaration_name: "chosen".into(),
            provenance: AttributeProvenance::Explicit,
            value: retained.clone(),
        };

        let query_value = value.query_value(&schema);

        assert_eq!(query_value.kind, AttributeKind::LabelList);
        assert_eq!(query_value.provenance, AttributeProvenance::Explicit);
        assert!(Arc::ptr_eq(&query_value.value, &retained));
        let mut labels = Vec::new();
        query_value.value.labels(&mut labels);
        assert_eq!(labels, [before, selected, fallback]);
    }

    #[test]
    fn attr_candidates_preserve_equal_selector_key_correlation() {
        let left = string_selector(&[
            ("@@//conditions:enabled", "left-enabled"),
            ("@@//conditions:disabled", "left-disabled"),
        ]);
        let right = string_selector(&[
            ("@@//conditions:disabled", "-right-disabled"),
            ("@@//conditions:enabled", "-right-enabled"),
        ]);

        let candidates = CoercedAttributeValue::Concatenation(Arc::new(left), Arc::new(right))
            .attr_visible_candidates(render_bazel_label)
            .unwrap();

        assert_eq!(
            candidates,
            ["left-enabled-right-enabled", "left-disabled-right-disabled"]
        );
    }

    #[test]
    fn attr_candidates_cross_product_distinct_selector_key_sets() {
        let left = string_selector(&[
            ("@@//conditions:enabled", "left-enabled"),
            ("@@//conditions:disabled", "left-disabled"),
        ]);
        let right = string_selector(&[
            ("@@//conditions:linux", "-right-linux"),
            ("@@//conditions:mac", "-right-mac"),
        ]);

        let candidates = CoercedAttributeValue::Concatenation(Arc::new(left), Arc::new(right))
            .attr_visible_candidates(render_bazel_label)
            .unwrap();

        assert_eq!(
            candidates,
            [
                "left-enabled-right-linux",
                "left-enabled-right-mac",
                "left-disabled-right-linux",
                "left-disabled-right-mac",
            ]
        );
    }

    #[test]
    fn attr_candidates_correlate_explicit_keys_even_when_only_one_selector_has_default() {
        let condition = label("@@//conditions:a");
        let left = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition.clone(), string_value("x"))]),
            default: Some(string_value("y")),
        };
        let right = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition, string_value("z"))]),
            default: None,
        };

        let candidates = CoercedAttributeValue::Concatenation(Arc::new(left), Arc::new(right))
            .attr_visible_candidates(render_bazel_label)
            .unwrap();

        assert_eq!(candidates, ["xz"]);
    }

    #[test]
    fn nested_selectors_reject_conflicting_equal_key_and_default_bindings() {
        let condition = label("@@//conditions:a");
        let inner_for_explicit = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition.clone(), string_value("explicit-explicit"))]),
            default: Some(string_value("explicit-default-conflict")),
        };
        let explicit_outer = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition.clone(), Arc::new(inner_for_explicit))]),
            default: None,
        };

        assert_eq!(
            explicit_outer
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["explicit-explicit"]
        );

        let inner_for_default = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition.clone(), string_value("default-explicit-conflict"))]),
            default: Some(string_value("default-default")),
        };
        let default_outer = CoercedAttributeValue::Selector {
            branches: Arc::from([(condition, Arc::new(CoercedAttributeValue::None))]),
            default: Some(Arc::new(inner_for_default)),
        };

        assert_eq!(
            default_outer
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["default-default"]
        );
    }

    #[test]
    fn invalid_concatenation_returns_a_typed_error() {
        let invalid = CoercedAttributeValue::Concatenation(
            Arc::new(CoercedAttributeValue::Label(label("@@//pkg:left"))),
            Arc::new(CoercedAttributeValue::Label(label("@@//pkg:right"))),
        );

        let error = invalid
            .attr_visible_candidates(render_bazel_label)
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            "cannot concatenate attribute candidate types label and label"
        );
    }

    #[test]
    fn attr_candidates_render_ordered_lists_and_dictionaries_with_canonical_labels() {
        let scalar_label = CoercedAttributeValue::Label(label("@@//pkg:scalar"));
        let scalar_string = CoercedAttributeValue::String(CompactString::new("literal"));
        let labels = CoercedAttributeValue::LabelList(Arc::from([
            label("@@//pkg:first"),
            label("@@//pkg:second"),
            label("@@//pkg:second"),
        ]));
        let keyed_labels = CoercedAttributeValue::StringKeyedLabelDict(Arc::from([
            (CompactString::new("z"), label("@@//pkg:last")),
            (CompactString::new("a"), label("@@//pkg:first")),
        ]));
        let label_keyed = CoercedAttributeValue::LabelKeyedStringDict(Arc::from([
            (label("@@//pkg:z"), CompactString::new("last")),
            (label("@@//pkg:a"), CompactString::new("first")),
        ]));
        let label_lists = CoercedAttributeValue::LabelListDict(Arc::from([(
            CompactString::new("ordered"),
            Arc::from([label("@@//pkg:one"), label("@@//pkg:one")]),
        )]));

        assert_eq!(
            scalar_label
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["//pkg:scalar"]
        );
        assert_eq!(
            scalar_string
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["literal"]
        );
        assert_eq!(
            labels.attr_visible_candidates(render_bazel_label).unwrap(),
            ["[//pkg:first, //pkg:second, //pkg:second]"]
        );
        assert_eq!(
            keyed_labels
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["{z=//pkg:last, a=//pkg:first}"]
        );
        assert_eq!(
            label_keyed
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["{//pkg:z=last, //pkg:a=first}"]
        );
        assert_eq!(
            label_lists
                .attr_visible_candidates(render_bazel_label)
                .unwrap(),
            ["{ordered=[//pkg:one, //pkg:one]}"]
        );
    }

    #[test]
    fn attr_candidates_skip_null_optional_values_and_are_available_from_query_values() {
        let value = AttributeQueryValue {
            kind: AttributeKind::Label,
            provenance: AttributeProvenance::Default,
            value: Arc::new(CoercedAttributeValue::None),
        };

        assert!(
            value
                .attr_visible_candidates(render_bazel_label)
                .unwrap()
                .is_empty()
        );
    }
}
