/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalRepoName(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApparentRepoName(String);

impl CanonicalRepoName {
    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn new(name: impl Into<String>) -> Result<Self, String> {
        let name = name.into();
        validate_repo_name(&name)?;
        Ok(Self(name))
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let Some(rest) = value.strip_prefix("@@") else {
            return Err(format!("canonical repository must start with @@: {value}"));
        };
        if rest.is_empty() {
            return Ok(Self::root());
        }
        Self::new(rest)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

impl ApparentRepoName {
    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn new(name: impl Into<String>) -> Result<Self, String> {
        let name = name.into();
        validate_repo_name(&name)?;
        Ok(Self(name))
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let Some(rest) = value.strip_prefix('@') else {
            return Err(format!("apparent repository must start with @: {value}"));
        };
        if rest.starts_with('@') {
            return Err(format!(
                "apparent repository uses one @, not canonical spelling: {value}"
            ));
        }
        if rest.is_empty() {
            return Ok(Self::root());
        }
        Self::new(rest)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for CanonicalRepoName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_root() {
            f.write_str("@@")
        } else {
            write!(f, "@@{}", self.0)
        }
    }
}

impl fmt::Display for ApparentRepoName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_root() {
            f.write_str("@")
        } else {
            write!(f, "@{}", self.0)
        }
    }
}

fn validate_repo_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("repository name must not be empty".to_owned());
    }
    if name.starts_with('.') || name.ends_with('.') {
        return Err(format!(
            "repository name must not start or end with '.': {name}"
        ));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '+' | '~'))
    {
        return Err(format!("invalid repository name: {name}"));
    }
    Ok(())
}
