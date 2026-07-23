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
}

impl AttributeKind {
    pub(crate) fn reaches_labels(self) -> bool {
        !matches!(self, Self::String)
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
}

impl AttributeSchema {
    pub(crate) fn new(
        declaration_name: impl Into<CompactString>,
        kind: AttributeKind,
        mandatory: bool,
        configurable: bool,
        default: Option<CoercedAttributeValue>,
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
}

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

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum CoercedAttributeValue {
    /// Bazel's optional scalar-label default is null; it is not a label.
    None,
    Label(CanonicalLabel),
    LabelList(Arc<[CanonicalLabel]>),
    String(CompactString),
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
            Self::String(_) | Self::None => {}
        }
    }
}
