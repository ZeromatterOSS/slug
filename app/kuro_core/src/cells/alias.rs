/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::borrow::Borrow;
use std::hash::Hash;
use std::hash::Hasher;

use allocative::Allocative;
use derive_more::Display;
use equivalent::Equivalent;
use kuro_util::hash::BuckHasher;
use static_interner::Intern;
use static_interner::interner;
use strong_hash::StrongHash;

#[derive(Debug, kuro_error::Error)]
#[kuro(input)]
enum CellAliasError {
    #[error("Empty alias where non-empty is required")]
    EmptyAlias,
}

#[derive(Clone, Debug, Display, Eq, PartialEq, Ord, PartialOrd, Allocative)]
struct CellAliasData(Box<str>);

#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for CellAliasData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        CellAliasDataRef(&self.0).hash(state)
    }
}

impl StrongHash for CellAliasData {
    fn strong_hash<H: Hasher>(&self, state: &mut H) {
        CellAliasDataRef(&self.0).strong_hash(state)
    }
}

#[derive(Clone, Debug, Display, Hash, Eq, PartialEq, StrongHash)]
struct CellAliasDataRef<'a>(&'a str);

impl Equivalent<CellAliasData> for CellAliasDataRef<'_> {
    fn equivalent(&self, key: &CellAliasData) -> bool {
        self.0 == &*key.0
    }
}

impl<'a> From<CellAliasDataRef<'a>> for CellAliasData {
    fn from(d: CellAliasDataRef<'a>) -> Self {
        CellAliasData(d.0.into())
    }
}

interner!(INTERNER, BuckHasher, CellAliasData);

/// A 'CellAlias' is a user-provided string name that maps to a 'CellName'.
/// The mapping of 'CellAlias' to 'CellName' is specific to the current cell so
/// that the same 'CellAlias' may map to different 'CellName's depending on what
/// the current 'CellInstance' is that references the 'CellAlias'.
#[derive(
    Copy, Clone, Debug, Display, Hash, Eq, PartialEq, Ord, PartialOrd, Allocative
)]
pub struct CellAlias(Intern<CellAliasData>);

impl CellAlias {
    pub fn new(alias: String) -> CellAlias {
        CellAlias(INTERNER.intern(CellAliasDataRef(&alias)))
    }

    #[inline]
    pub fn as_str(&self) -> &'static str {
        &self.0.deref_static().0
    }
}

impl Borrow<str> for CellAlias {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

/// Empty string is an alias for the current cell.
/// This type does not permit it.
#[derive(
    Display, Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Allocative
)]
pub struct NonEmptyCellAlias(Intern<CellAliasData>);

impl NonEmptyCellAlias {
    pub fn new(alias: String) -> kuro_error::Result<NonEmptyCellAlias> {
        if alias.is_empty() {
            Err(CellAliasError::EmptyAlias.into())
        } else {
            Ok(NonEmptyCellAlias(INTERNER.intern(CellAliasDataRef(&alias))))
        }
    }

    pub fn testing_new(alias: &str) -> NonEmptyCellAlias {
        Self::new(alias.to_owned()).unwrap()
    }

    pub fn as_str(&self) -> &'static str {
        &self.0.deref_static().0
    }
}

impl Borrow<str> for NonEmptyCellAlias {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
