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

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AttributeValue {
    Bool(bool),
    Int(i64),
    String(String),
    Label(CanonicalLabel),
    LabelList(Vec<CanonicalLabel>),
    File(String),
    FileList(Vec<String>),
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct AttributeMap {
    values: BTreeMap<String, AttributeValue>,
}

impl AttributeMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: impl Into<String>, value: AttributeValue) {
        self.values.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<&AttributeValue> {
        self.values.get(name)
    }

    pub fn get_string(&self, name: &str) -> Option<&str> {
        match self.get(name) {
            Some(AttributeValue::String(value)) => Some(value),
            _ => None,
        }
    }

    pub fn get_label(&self, name: &str) -> Option<&CanonicalLabel> {
        match self.get(name) {
            Some(AttributeValue::Label(value)) => Some(value),
            _ => None,
        }
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }
}
