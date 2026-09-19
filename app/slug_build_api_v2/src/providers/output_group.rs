/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 * This source code is dual-licensed under the MIT license and Apache License, Version 2.0.
 */

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use starlark_map::small_map::SmallMap;

use super::ProviderError;
use crate::analysis_value::AnalysisDepset;
use crate::analysis_value::AnalysisValue;
use crate::analysis_value::AnalysisValueKind;
use crate::analysis_value::AnalysisValueType;
use crate::analysis_value::ProviderIdentity;
use crate::analysis_value::ProviderOccurrence;
use crate::analysis_value::PublicationEqState;
use crate::depset::DepsetOrder;

/// Named artifact depsets retaining their configured owners and shared graph.
#[derive(Debug, Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct OutputGroupInfo {
    groups: Arc<SmallMap<CompactString, AnalysisDepset>>,
}

impl OutputGroupInfo {
    pub fn new(
        groups: impl IntoIterator<Item = (impl Into<CompactString>, AnalysisDepset)>,
    ) -> Result<Self, ProviderError> {
        let mut groups = groups
            .into_iter()
            .map(|(name, files)| (name.into(), files))
            .collect::<Vec<_>>();
        groups.sort_by(|(left, _), (right, _)| left.encode_utf16().cmp(right.encode_utf16()));
        let mut all_empty = true;
        for (index, (name, files)) in groups.iter().enumerate() {
            match files.element_type() {
                AnalysisValueType::Empty => {}
                AnalysisValueType::Artifact => all_empty = false,
                element_type => {
                    return Err(ProviderError::InvalidOutputGroupFiles {
                        name: name.clone(),
                        element_type,
                    });
                }
            }
            if index > 0 && groups[index - 1].0 == *name {
                return Err(ProviderError::DuplicateOutputGroup { name: name.clone() });
            }
        }
        let groups = groups
            .into_iter()
            .map(|(name, files)| {
                let files = if all_empty {
                    AnalysisDepset::empty(DepsetOrder::Default)
                } else {
                    files
                };
                (name, files)
            })
            .collect();
        Ok(Self {
            groups: Arc::new(groups),
        })
    }

    pub fn groups(&self) -> &SmallMap<CompactString, AnalysisDepset> {
        &self.groups
    }

    pub fn to_occurrence(&self) -> ProviderOccurrence {
        ProviderOccurrence::new(
            ProviderIdentity::builtin("OutputGroupInfo"),
            self.groups
                .iter()
                .map(|(name, files)| (name.clone(), AnalysisValue::depset(files.dupe()))),
        )
    }

    pub fn from_occurrence(value: &ProviderOccurrence) -> Result<Self, ProviderError> {
        if value.identity() != &ProviderIdentity::builtin("OutputGroupInfo") {
            return Err(ProviderError::InvalidOutputGroupIdentity {
                identity: value.identity().clone(),
            });
        }
        let groups = value
            .fields()
            .iter()
            .map(|(name, value)| {
                let AnalysisValueKind::Depset(files) = value.kind() else {
                    return Err(ProviderError::InvalidOutputGroupField {
                        name: name.clone(),
                        value_type: value.value_type(),
                    });
                };
                Ok((name.clone(), files.dupe()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(groups)
    }

    pub(super) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.groups.len() == other.groups.len()
            && self.groups.iter().all(|(name, files)| {
                other
                    .groups
                    .get(name)
                    .is_some_and(|other| files.publication_eq_with(other, state))
            })
    }
}
