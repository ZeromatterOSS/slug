/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed licenses.
 */

//! DICE keys for aspect computation (Phase 8c).
//!
//! AspectKey is the cache key for aspect computation results:
//! - Key: (target, aspect_type)
//! - Value: AspectValue (provider collection returned by the aspect)

use std::sync::Arc;

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;
use slug_build_api::analysis::AnalysisResult;
use slug_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;
use slug_core::target::configured_target_label::ConfiguredTargetLabel;
use slug_node::aspect_type::StarlarkAspectType;

/// DICE key for caching aspect computation results (Phase 8c).
///
/// Key = (target, aspect_type) → Value = AspectValue (providers)
///
/// This key identifies a unique aspect computation:
/// - `target`: The configured target this aspect is applied to
/// - `aspect_type`: The full aspect identity (module path + name) for loading via DICE
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("AspectKey({}, {})", target, aspect_type)]
pub struct AspectKey {
    pub target: ConfiguredTargetLabel,
    pub aspect_type: Arc<StarlarkAspectType>,
}

impl AspectKey {
    pub fn new(target: ConfiguredTargetLabel, aspect_type: Arc<StarlarkAspectType>) -> Self {
        Self {
            target,
            aspect_type,
        }
    }
}

/// Result of aspect computation (cached in DICE).
///
/// Contains both the frozen provider collection (for provider access) and the
/// full AnalysisResult (for action lookup during build execution).
#[derive(Clone, Dupe, Debug, Allocative)]
pub struct AspectValue {
    pub providers: FrozenProviderCollectionValue,
    /// The full analysis result including recorded actions.
    /// Needed by `EVAL_ASPECT_DEFERRED` to resolve action lookups for
    /// aspect-registered artifacts.
    pub analysis_result: Option<AnalysisResult>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dupe::Dupe;
    use slug_core::bzl::ImportPath;
    use slug_core::configuration::data::ConfigurationData;
    use slug_core::target::label::label::TargetLabel;
    use slug_node::aspect_type::StarlarkAspectType;
    use slug_node::bzl_or_bxl_path::BzlOrBxlPath;

    use super::AspectKey;

    fn make_aspect_type(path: &str, name: &str) -> Arc<StarlarkAspectType> {
        Arc::new(StarlarkAspectType::new(
            BzlOrBxlPath::Bzl(ImportPath::testing_new(path)),
            name.to_owned(),
        ))
    }

    fn make_configured_label(
        label: &str,
    ) -> slug_core::target::configured_target_label::ConfiguredTargetLabel {
        TargetLabel::testing_parse(label).configure(ConfigurationData::testing_new())
    }

    #[test]
    fn aspect_key_display() {
        let target = make_configured_label("root//pkg:target");
        let aspect_type = make_aspect_type("root//aspects:defs.bzl", "my_aspect");
        let key = AspectKey::new(target, aspect_type);

        // Verify Display includes both target and aspect
        let display = key.to_string();
        assert!(display.contains("root//pkg:target"));
        assert!(display.contains("my_aspect"));
    }

    #[test]
    fn aspect_key_equality() {
        let target1 = make_configured_label("root//pkg:t1");
        let target2 = make_configured_label("root//pkg:t1");
        let target3 = make_configured_label("root//pkg:t2");

        let aspect1 = make_aspect_type("root//a:a.bzl", "asp");
        let aspect2 = make_aspect_type("root//a:a.bzl", "asp");

        let key1 = AspectKey::new(target1, aspect1.clone());
        let key2 = AspectKey::new(target2, aspect2);
        let key3 = AspectKey::new(target3, aspect1);

        assert_eq!(key1, key2); // Same target and aspect
        assert_ne!(key1, key3); // Different target
    }

    #[test]
    fn aspect_key_dupe() {
        let target = make_configured_label("root//pkg:target");
        let aspect_type = make_aspect_type("root//aspects:defs.bzl", "my_aspect");
        let key = AspectKey::new(target, aspect_type);

        let duped = key.dupe();
        assert_eq!(key, duped);
        // Verify Arc sharing (cheap clone)
        assert!(Arc::ptr_eq(&key.aspect_type, &duped.aspect_type));
    }

    #[test]
    fn aspect_key_different_aspects_not_equal() {
        let target = make_configured_label("root//pkg:target");
        let aspect1 = make_aspect_type("root//aspects:defs.bzl", "aspect1");
        let aspect2 = make_aspect_type("root//aspects:defs.bzl", "aspect2");

        let key1 = AspectKey::new(target.clone(), aspect1);
        let key2 = AspectKey::new(target, aspect2);

        assert_ne!(key1, key2); // Different aspects on same target
    }
}
