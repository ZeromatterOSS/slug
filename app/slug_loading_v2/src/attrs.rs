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

impl CoercedAttributeValue {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use slug_identity_v2::CanonicalLabel;

    use super::AttributeKind;
    use super::AttributeProvenance;
    use super::AttributeSchema;
    use super::AttributeValue;
    use super::CoercedAttributeValue;

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
}
