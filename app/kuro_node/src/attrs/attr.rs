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
use std::fmt::Display;
use std::sync::Arc;

use allocative::Allocative;
use pagable::Pagable;

use crate::aspect_type::StarlarkAspectType;
use crate::attrs::attr_type::AttrType;
use crate::attrs::coerced_attr::CoercedAttr;
use crate::attrs::display::AttrDisplayWithContextExt;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Pagable, Allocative)]
enum AttributeDefault {
    No,
    Yes(Arc<CoercedAttr>),
    DefaultOnly(Arc<CoercedAttr>),
}

/// Starlark compatible container for results from e.g. `attrs.string()`
#[derive(Clone, Debug, Eq, PartialEq, Hash, Pagable, Allocative)]
pub struct Attribute {
    /// The default value. If None, the value is not optional and must be provided by the user
    default: AttributeDefault,
    /// Documentation for what the attribute actually means
    doc: String,
    /// The coercer to take this parameter's value from Starlark value -> an
    /// internal representation
    coercer: AttrType,
    /// Aspects to apply to dependencies of this attribute (Phase 8c)
    /// Uses Arc<StarlarkAspectType> to enable DICE-based module loading with cheap cloning
    aspects: Vec<Arc<StarlarkAspectType>>,
}

impl Attribute {
    pub fn new(default: Option<Arc<CoercedAttr>>, doc: &str, coercer: AttrType) -> Self {
        Attribute {
            default: match default {
                Some(x) => AttributeDefault::Yes(x),
                None => AttributeDefault::No,
            },
            doc: doc.to_owned(),
            coercer,
            aspects: Vec::new(),
        }
    }

    pub fn new_default_only(default: Arc<CoercedAttr>, doc: &str, coercer: AttrType) -> Self {
        Attribute {
            default: AttributeDefault::DefaultOnly(default),
            doc: doc.to_owned(),
            coercer,
            aspects: Vec::new(),
        }
    }

    pub fn coercer(&self) -> &AttrType {
        &self.coercer
    }

    pub fn is_default_only(&self) -> bool {
        matches!(self.default, AttributeDefault::DefaultOnly(_))
    }

    pub fn default(&self) -> Option<&Arc<CoercedAttr>> {
        match &self.default {
            AttributeDefault::Yes(x) => Some(x),
            AttributeDefault::DefaultOnly(x) => Some(x),
            AttributeDefault::No => None,
        }
    }

    pub fn doc(&self) -> &str {
        &self.doc
    }

    /// Add aspects to this attribute (Phase 8c).
    pub fn with_aspects(mut self, aspects: Vec<Arc<StarlarkAspectType>>) -> Self {
        self.aspects = aspects;
        self
    }

    /// Get the aspects attached to this attribute (Phase 8c).
    pub fn aspects(&self) -> &[Arc<StarlarkAspectType>] {
        &self.aspects
    }
}

impl Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.coercer.fmt_with_default(
            f,
            self.default()
                .map(|x| x.as_display_no_ctx().to_string())
                .as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_core::bzl::ImportPath;

    use super::*;
    use crate::aspect_type::StarlarkAspectType;
    use crate::attrs::attr_type::AttrType;
    use crate::bzl_or_bxl_path::BzlOrBxlPath;

    fn make_aspect_type(name: &str) -> Arc<StarlarkAspectType> {
        Arc::new(StarlarkAspectType::new(
            BzlOrBxlPath::Bzl(ImportPath::testing_new("root//pkg:aspects.bzl")),
            name.to_owned(),
        ))
    }

    #[test]
    fn attribute_with_aspects() {
        let attr = Attribute::new(None, "test attr", AttrType::string()).with_aspects(vec![
            make_aspect_type("aspect1"),
            make_aspect_type("aspect2"),
        ]);

        assert_eq!(attr.aspects().len(), 2);
        assert_eq!(attr.aspects()[0].name, "aspect1");
        assert_eq!(attr.aspects()[1].name, "aspect2");
    }

    #[test]
    fn attribute_without_aspects() {
        let attr = Attribute::new(None, "test attr", AttrType::string());
        assert!(attr.aspects().is_empty());
    }

    #[test]
    fn attribute_aspects_preserves_module_path() {
        let aspect = make_aspect_type("my_aspect");
        let attr = Attribute::new(None, "test attr", AttrType::string())
            .with_aspects(vec![aspect.clone()]);

        let stored = &attr.aspects()[0];
        assert_eq!(stored.name, "my_aspect");
        // The path contains the import path - verify it's not empty
        let path_str = stored.path.to_string();
        assert!(!path_str.is_empty(), "Path should not be empty");
        // Verify it contains "aspects.bzl" at minimum
        assert!(
            path_str.contains("aspects.bzl"),
            "Path '{}' should contain 'aspects.bzl'",
            path_str
        );
    }

    #[test]
    fn attribute_with_aspects_equality() {
        let aspect1 = make_aspect_type("aspect1");
        let aspect2 = make_aspect_type("aspect2");

        let attr1 =
            Attribute::new(None, "test", AttrType::string()).with_aspects(vec![aspect1.clone()]);
        let attr2 =
            Attribute::new(None, "test", AttrType::string()).with_aspects(vec![aspect1.clone()]);
        let attr3 =
            Attribute::new(None, "test", AttrType::string()).with_aspects(vec![aspect2.clone()]);

        assert_eq!(attr1, attr2); // Same aspects
        assert_ne!(attr1, attr3); // Different aspects
    }

    #[test]
    fn attribute_empty_aspects_vec() {
        let attr = Attribute::new(None, "test attr", AttrType::string()).with_aspects(vec![]);

        assert!(attr.aspects().is_empty());
        assert_eq!(attr.aspects().len(), 0);
    }
}

/// Attribute which may be either a custom value supplied by the user, or missing/None to indicate use the default.
#[derive(Eq, PartialEq)]
pub enum CoercedValue {
    Custom(CoercedAttr),
    Default,
}
