/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_core::package::PackageLabel;
use kuro_core::package::source_path::SourcePathRef;
use kuro_core::provider::label::ProvidersLabel;
use kuro_core::target::label::label::TargetLabel;
use kuro_node::attrs::attr_type::AttrType;
use kuro_node::attrs::attr_type::configuration_dep::ConfigurationDepKind;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::traversal::CoercedAttrTraversal;
use kuro_node::visibility::VisibilityPattern;
use kuro_node::visibility::VisibilityPatternList;
use kuro_node::visibility::WithinViewSpecification;
use dupe::Dupe;

fn indented_within_view(spec: &WithinViewSpecification) -> String {
    match &spec.0 {
        VisibilityPatternList::Public => format!("  {}\n", VisibilityPattern::PUBLIC),
        VisibilityPatternList::List(items) => {
            let mut s = String::new();
            for item in items {
                s.push_str(&format!("  {item}\n"));
            }
            s
        }
    }
}

#[derive(Debug, kuro_error::Error)]
#[kuro(input)]
enum CheckWithinViewError {
    #[error(
        "Target's `within_view` attribute does not allow dependency `{}`. Allowed dependencies:\n{}",
        _0,
        indented_within_view(_1)
    )]
    #[kuro(tag = Visibility)]
    DepNotWithinView(TargetLabel, WithinViewSpecification),
}

/// Check that dependencies in attribute do not violate `within_view`.
pub(crate) fn check_within_view(
    attr: &CoercedAttr,
    pkg: PackageLabel,
    attr_type: &AttrType,
    within_view: &WithinViewSpecification,
) -> kuro_error::Result<()> {
    if within_view == &WithinViewSpecification::PUBLIC {
        // Shortcut.
        return Ok(());
    }

    struct WithinViewCheckTraversal<'x> {
        pkg: PackageLabel,
        within_view: &'x WithinViewSpecification,
    }

    impl<'x> WithinViewCheckTraversal<'x> {
        fn check_dep_within_view(&self, dep: &TargetLabel) -> kuro_error::Result<()> {
            if self.pkg == dep.pkg() || self.within_view.0.matches_target(dep) {
                Ok(())
            } else {
                Err(
                    CheckWithinViewError::DepNotWithinView(dep.dupe(), self.within_view.dupe())
                        .into(),
                )
            }
        }
    }

    impl<'a, 'x> CoercedAttrTraversal<'a> for WithinViewCheckTraversal<'x> {
        fn dep(&mut self, dep: &ProvidersLabel) -> kuro_error::Result<()> {
            self.check_dep_within_view(dep.target())
        }

        fn configuration_dep(
            &mut self,
            dep: &ProvidersLabel,
            t: ConfigurationDepKind,
        ) -> kuro_error::Result<()> {
            match t {
                // Skip some configuration deps
                ConfigurationDepKind::CompatibilityAttribute => (),
                ConfigurationDepKind::SelectKey => (),
                ConfigurationDepKind::ConfiguredDepPlatform | ConfigurationDepKind::Transition => {
                    self.check_dep_within_view(dep.target())?
                }
            }
            Ok(())
        }

        fn input(&mut self, _input: SourcePathRef) -> kuro_error::Result<()> {
            Ok(())
        }
    }

    attr.traverse(
        attr_type,
        pkg.dupe(),
        &mut WithinViewCheckTraversal { pkg, within_view },
    )
}
