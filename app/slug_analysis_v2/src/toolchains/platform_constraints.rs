/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;

use slug_identity_v2::CanonicalLabel;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ConstraintSetting {
    label: CanonicalLabel,
}

impl ConstraintSetting {
    pub fn new(label: CanonicalLabel) -> Self {
        Self { label }
    }

    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ConstraintValue {
    setting: ConstraintSetting,
    label: CanonicalLabel,
}

impl ConstraintValue {
    pub fn new(setting: ConstraintSetting, label: CanonicalLabel) -> Self {
        Self { setting, label }
    }

    pub fn setting(&self) -> &ConstraintSetting {
        &self.setting
    }

    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ConstraintSet {
    values: BTreeMap<ConstraintSetting, ConstraintValue>,
}

impl ConstraintSet {
    pub fn new(values: Vec<ConstraintValue>) -> Self {
        let mut set = Self::default();
        for value in values {
            set.insert(value);
        }
        set
    }

    pub fn insert(&mut self, value: ConstraintValue) {
        self.values.insert(value.setting().clone(), value);
    }

    pub fn values(&self) -> impl Iterator<Item = &ConstraintValue> {
        self.values.values()
    }

    pub fn is_satisfied_by(&self, platform: &ConstraintSet) -> bool {
        self.values.iter().all(|(setting, expected)| {
            platform
                .values
                .get(setting)
                .is_some_and(|actual| actual == expected)
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExecutionPlatform {
    label: CanonicalLabel,
    constraints: ConstraintSet,
    exec_properties: BTreeMap<String, String>,
}

impl ExecutionPlatform {
    pub fn new(label: CanonicalLabel, constraints: ConstraintSet) -> Self {
        Self {
            label,
            constraints,
            exec_properties: BTreeMap::new(),
        }
    }

    pub fn with_exec_properties(mut self, exec_properties: BTreeMap<String, String>) -> Self {
        self.exec_properties = exec_properties;
        self
    }

    pub fn label(&self) -> &CanonicalLabel {
        &self.label
    }

    pub fn constraints(&self) -> &ConstraintSet {
        &self.constraints
    }

    pub fn exec_properties(&self) -> &BTreeMap<String, String> {
        &self.exec_properties
    }
}
