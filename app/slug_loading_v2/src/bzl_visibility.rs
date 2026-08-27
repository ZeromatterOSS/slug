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
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Value;
use starlark::values::list::ListRef;
use starlark::values::none::NoneType;

use crate::bzl_module::BzlModuleIdentity;
use crate::provider::BzlEvaluationContext;

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Default, Dupe)]
pub(crate) enum BzlLoadVisibility {
    #[default]
    Public,
    Private,
    Packages(Arc<[BzlVisibilityPackageSpec]>),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum BzlVisibilityPackageSpec {
    Exact(PackageIdentifier),
    Subpackages(PackageIdentifier),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct BzlLoadVisibilityError {
    dependency: CanonicalLabel,
    importer: PackageIdentifier,
}

impl fmt::Display for BzlLoadVisibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Starlark file {} is not visible for loading from package {}. Check the file's `visibility()` declaration.",
            self.dependency, self.importer
        )
    }
}

impl std::error::Error for BzlLoadVisibilityError {}

impl BzlLoadVisibility {
    pub(crate) fn allows_load_from(
        &self,
        loaded_package: &PackageIdentifier,
        importer_package: &PackageIdentifier,
    ) -> bool {
        if loaded_package == importer_package {
            return true;
        }
        match self {
            Self::Public => true,
            Self::Private => false,
            Self::Packages(specifications) => specifications
                .iter()
                .any(|specification| specification.allows(importer_package)),
        }
    }
}

impl BzlVisibilityPackageSpec {
    fn allows(&self, importer: &PackageIdentifier) -> bool {
        match self {
            Self::Exact(package) => package == importer,
            Self::Subpackages(package) => {
                package.repo() == importer.repo()
                    && is_same_or_descendant(
                        package.package().as_str(),
                        importer.package().as_str(),
                    )
            }
        }
    }
}

fn is_same_or_descendant(parent: &str, child: &str) -> bool {
    parent.is_empty()
        || child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

pub(crate) fn validate_bzl_load_visibility(
    importer: &PackageIdentifier,
    dependency_label: &CanonicalLabel,
    visibility: &BzlLoadVisibility,
) -> Result<(), BzlLoadVisibilityError> {
    if visibility.allows_load_from(dependency_label.package(), importer) {
        Ok(())
    } else {
        Err(BzlLoadVisibilityError {
            dependency: dependency_label.clone(),
            importer: importer.clone(),
        })
    }
}

pub(crate) fn parse_bzl_load_visibility(
    value: Value<'_>,
    owner: &BzlModuleIdentity,
) -> anyhow::Result<BzlLoadVisibility> {
    let mut specifications = Vec::new();
    if let Some(value) = value.unpack_str() {
        collect_specification(value, owner, &mut specifications)?;
    } else if let Some(values) = ListRef::from_value(value) {
        for (index, value) in values.iter().enumerate() {
            let value = value.unpack_str().ok_or_else(|| {
                anyhow::anyhow!(
                    "at index {index} of visibility list, got element of type {}, want string",
                    value.get_type()
                )
            })?;
            collect_specification(value, owner, &mut specifications)?;
        }
    } else {
        anyhow::bail!(
            "Invalid visibility: got '{}', want string or list of strings",
            value.get_type()
        );
    }
    Ok(normalize_specifications(specifications))
}

fn collect_specification(
    value: &str,
    owner: &BzlModuleIdentity,
    specifications: &mut Vec<ParsedSpecification>,
) -> anyhow::Result<()> {
    if value.starts_with('-') {
        anyhow::bail!("Cannot use negative package patterns here");
    }
    match value {
        "public" => specifications.push(ParsedSpecification::Public),
        "private" => specifications.push(ParsedSpecification::Private),
        _ => specifications.push(ParsedSpecification::Package(parse_package_specification(
            value, owner,
        )?)),
    }
    Ok(())
}

enum ParsedSpecification {
    Public,
    Private,
    Package(BzlVisibilityPackageSpec),
}

fn normalize_specifications(specifications: Vec<ParsedSpecification>) -> BzlLoadVisibility {
    if specifications
        .iter()
        .any(|specification| matches!(specification, ParsedSpecification::Public))
    {
        return BzlLoadVisibility::Public;
    }
    let packages = specifications
        .into_iter()
        .filter_map(|specification| match specification {
            ParsedSpecification::Package(package) => Some(package),
            ParsedSpecification::Public | ParsedSpecification::Private => None,
        })
        .collect::<Vec<_>>();
    if packages.is_empty() {
        BzlLoadVisibility::Private
    } else {
        BzlLoadVisibility::Packages(packages.into())
    }
}

fn parse_package_specification(
    value: &str,
    owner: &BzlModuleIdentity,
) -> anyhow::Result<BzlVisibilityPackageSpec> {
    let (base, subtree) = if value == "//..." || value.ends_with("//...") {
        (value.strip_suffix("...").unwrap(), true)
    } else if let Some(base) = value.strip_suffix("/...") {
        (base, true)
    } else if let Some((base, target)) = value.rsplit_once(':') {
        match target {
            "__pkg__" => (base, false),
            "__subpackages__" => (base, true),
            _ => anyhow::bail!("invalid package name '{value}'"),
        }
    } else {
        (value, false)
    };
    let package = resolve_package(base, owner)?;
    Ok(if subtree {
        BzlVisibilityPackageSpec::Subpackages(package)
    } else {
        BzlVisibilityPackageSpec::Exact(package)
    })
}

fn resolve_package(value: &str, owner: &BzlModuleIdentity) -> anyhow::Result<PackageIdentifier> {
    let (repository, package) = if let Some(rest) = value.strip_prefix("@@") {
        let (repository, package) = rest
            .split_once("//")
            .ok_or_else(|| anyhow::anyhow!("invalid package name '{value}'"))?;
        let repository = if repository.is_empty() {
            CanonicalRepoName::root()
        } else {
            CanonicalRepoName::parse(&format!("@@{repository}")).map_err(anyhow::Error::msg)?
        };
        (repository, package)
    } else if let Some(rest) = value.strip_prefix('@') {
        let (repository, package) = rest
            .split_once("//")
            .ok_or_else(|| anyhow::anyhow!("invalid package name '{value}'"))?;
        if repository.is_empty() {
            (CanonicalRepoName::root(), package)
        } else {
            let apparent = ApparentRepoName::new(repository).map_err(anyhow::Error::msg)?;
            let mut matches = owner
                .repository_mapping
                .iter()
                .filter(|(candidate, _)| candidate == &apparent);
            let (_, canonical) = matches.next().ok_or_else(|| {
                anyhow::anyhow!(
                    "repository '@{repository}' is not visible from {}",
                    owner.label
                )
            })?;
            if matches.next().is_some() {
                anyhow::bail!(
                    "repository mapping for '@{repository}' is ambiguous from {}",
                    owner.label
                );
            }
            (canonical.clone(), package)
        }
    } else if let Some(package) = value.strip_prefix("//") {
        (owner.label.package().repo().clone(), package)
    } else {
        anyhow::bail!(
            "invalid package name '{value}': must start with '//', '@', or be 'public' or 'private'"
        );
    };
    let repository = if repository.is_root() {
        "@@".to_owned()
    } else {
        format!("@@{}", repository.as_str())
    };
    PackageIdentifier::parse_bazel_package_identifier(&format!("{repository}//{package}"))
        .map_err(anyhow::Error::msg)
}

#[starlark_module]
pub(crate) fn bzl_visibility_globals(builder: &mut GlobalsBuilder) {
    fn visibility<'v>(
        #[starlark(require = pos)] value: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let context = BzlEvaluationContext::from_evaluator(eval).map_err(|_| {
            anyhow::anyhow!(
                "visibility() can only be used during .bzl initialization (top-level evaluation)"
            )
        })?;
        if !is_direct_module_scope_call(eval) {
            anyhow::bail!(
                "load visibility may only be set at the top level, not inside a function"
            );
        }
        context.ensure_bzl_load_visibility_unset()?;
        let declaration = parse_bzl_load_visibility(value, context.source_identity())?;
        context.set_bzl_load_visibility(declaration)?;
        Ok(NoneType)
    }
}

fn is_direct_module_scope_call(eval: &Evaluator<'_, '_, '_>) -> bool {
    eval.native_caller_function_filename().is_none() && eval.call_stack().frames.len() == 1
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dupe::Dupe;
    use slug_identity_v2::ApparentRepoName;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_identity_v2::PackageIdentifier;
    use starlark::environment::FrozenModule;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::eval::FileLoader;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::*;
    use crate::package::loading_globals;

    fn owner() -> BzlModuleIdentity {
        BzlModuleIdentity {
            label: CanonicalLabel::parse(
                "@@rules_cc+//cc/private/rules_impl:cc_toolchain_info.bzl",
            )
            .unwrap(),
            workspace_path: "/rules_cc/cc/private/rules_impl/cc_toolchain_info.bzl".into(),
            repository_mapping: Arc::from([(
                ApparentRepoName::new("dep").unwrap(),
                CanonicalRepoName::new("dep+").unwrap(),
            )]),
        }
    }

    fn package(value: &str) -> PackageIdentifier {
        PackageIdentifier::parse_bazel_package_identifier(value).unwrap()
    }

    fn evaluate(
        source: &str,
        loader: Option<&dyn FileLoader>,
    ) -> Result<(BzlLoadVisibility, Option<String>), String> {
        let module = Module::new();
        let context = BzlEvaluationContext::from_identity(owner());
        let ast = AstModule::parse(
            "/rules_cc/cc/private/rules_impl/cc_toolchain_info.bzl",
            source.to_owned(),
            &Dialect::Bazel,
        )
        .map_err(|error| error.to_string())?;
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        if let Some(loader) = loader {
            evaluator.set_loader(loader);
        }
        evaluator
            .eval_module(ast, &loading_globals())
            .map_err(|error| error.to_string())?;
        drop(evaluator);
        Ok((
            context.bzl_load_visibility(),
            module.get("RESULT").map(|value| value.to_repr()),
        ))
    }

    #[test]
    fn declaration_normalizes_and_matches_canonical_packages() {
        let (visibility, _) = evaluate(
            concat!(
                "visibility([\n",
                "  \"private\", \"//client\", \"//tree/...\",\n",
                "  \"@dep//mapped:__pkg__\", \"@@//main:__subpackages__\",\n",
                "  \"@@canonical+//canonical\",\n",
                "])\n",
            ),
            None,
        )
        .unwrap();
        let loaded = package("@@rules_cc+//cc/private/rules_impl");
        for allowed in [
            "@@rules_cc+//cc/private/rules_impl",
            "@@rules_cc+//client",
            "@@rules_cc+//tree/child",
            "@@dep+//mapped",
            "@@//main/child",
            "@@canonical+//canonical",
        ] {
            assert!(
                visibility.allows_load_from(&loaded, &package(allowed)),
                "{allowed}"
            );
        }
        for denied in [
            "@@//client",
            "@@rules_cc+//client/child",
            "@@rules_cc+//trees",
            "@@dep+//mapped/child",
            "@@canonical+//canonical/child",
        ] {
            assert!(
                !visibility.allows_load_from(&loaded, &package(denied)),
                "{denied}"
            );
        }
    }

    #[test]
    fn declaration_public_private_empty_and_repo_root_subtree_are_distinct() {
        assert_eq!(
            evaluate("RESULT = 1", None).unwrap().0,
            BzlLoadVisibility::Public
        );
        assert_eq!(
            evaluate("visibility(\"public\")", None).unwrap().0,
            BzlLoadVisibility::Public
        );
        assert_eq!(
            evaluate("visibility([\"private\", \"//client\", \"public\"])", None)
                .unwrap()
                .0,
            BzlLoadVisibility::Public
        );
        for source in ["visibility(\"private\")", "visibility([])"] {
            assert_eq!(
                evaluate(source, None).unwrap().0,
                BzlLoadVisibility::Private
            );
        }
        let visibility = evaluate("visibility(\"//...\")", None).unwrap().0;
        let loaded = package("@@rules_cc+//cc/private/rules_impl");
        assert!(visibility.allows_load_from(&loaded, &package("@@rules_cc+//other")));
        assert!(!visibility.allows_load_from(&loaded, &package("@@//other")));
    }

    #[test]
    fn rules_cc_toolchain_info_visibility_evaluates_at_its_real_owner() {
        let (visibility, _) = evaluate("visibility([\"//cc/...\"])", None).unwrap();
        let loaded = package("@@rules_cc+//cc/private/rules_impl");
        assert!(visibility.allows_load_from(&loaded, &package("@@rules_cc+//cc")));
        assert!(visibility.allows_load_from(&loaded, &package("@@rules_cc+//cc/private/child")));
        assert!(!visibility.allows_load_from(&loaded, &package("@@//cc/private/child")));
    }

    #[test]
    fn builtin_has_exact_positional_abi_returns_none_and_declares_once() {
        let (visibility, result) = evaluate("RESULT = visibility(\"private\")", None).unwrap();
        assert_eq!(visibility, BzlLoadVisibility::Private);
        assert_eq!(result.as_deref(), Some("None"));
        for source in [
            "visibility()",
            "visibility(value = \"public\")",
            "visibility(\"public\", \"private\")",
            "visibility(\"public\")\nvisibility(\"private\")",
        ] {
            assert!(evaluate(source, None).is_err(), "{source}");
        }
        let error = evaluate("visibility(\"public\")\nvisibility(1)", None).unwrap_err();
        assert!(error.contains("may not be set more than once"), "{error}");
    }

    #[test]
    fn declaration_rejects_functions_bad_types_elements_patterns_and_mappings() {
        for source in [
            "def helper():\n  visibility(\"public\")\nhelper()",
            "visibility(1)",
            "visibility((\"//ok\",))",
            "visibility([\"//ok\", 1])",
            "visibility(\"-//bad\")",
            "visibility(\"bad\")",
            "visibility(\"@missing//bad\")",
            "visibility(\"//bad:target\")",
        ] {
            assert!(evaluate(source, None).is_err(), "{source}");
        }
    }

    struct ModuleLoader(Vec<(String, FrozenModule)>);

    impl FileLoader for ModuleLoader {
        fn load(&self, path: &str) -> starlark::Result<FrozenModule> {
            self.0
                .iter()
                .find_map(|(candidate, module)| (candidate == path).then(|| module.dupe()))
                .ok_or_else(|| {
                    starlark::Error::new_other(anyhow::anyhow!("unexpected load {path}"))
                })
        }
    }

    fn freeze(path: &str, source: &str, loader: Option<&dyn FileLoader>) -> FrozenModule {
        let module = Module::new();
        let context = BzlEvaluationContext::from_identity(owner());
        let ast = AstModule::parse(path, source.to_owned(), &Dialect::Bazel).unwrap();
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        if let Some(loader) = loader {
            evaluator.set_loader(loader);
        }
        evaluator.eval_module(ast, &loading_globals()).unwrap();
        drop(evaluator);
        module.freeze().unwrap()
    }

    #[test]
    fn declaration_rejects_an_imported_function_call() {
        let child = freeze(
            "/rules/other/helper.bzl",
            "def helper():\n  visibility(\"public\")\n",
            None,
        );
        let loader = ModuleLoader(vec![("//other:helper.bzl".to_owned(), child)]);
        assert!(
            evaluate(
                "load(\"//other:helper.bzl\", \"helper\")\nhelper()",
                Some(&loader)
            )
            .is_err()
        );
    }

    #[test]
    fn declaration_rejects_a_compiler_inlined_imported_function_call() {
        // starlark-rust freezes these single-return imported functions as inlineable
        // expressions. The diagnostic stack retains their inlined frames even though
        // `native_caller_function_filename` has no ordinary `def` frame to report.
        let leaf = freeze(
            "/rules/leaf/defs.bzl",
            "def leaf():\n  return visibility(\"public\")\n",
            None,
        );
        let leaf_loader = ModuleLoader(vec![("//leaf:defs.bzl".to_owned(), leaf)]);
        let middle = freeze(
            "/rules/middle/defs.bzl",
            concat!(
                "load(\"//leaf:defs.bzl\", \"leaf\")\n",
                "def helper():\n  return leaf()\n",
            ),
            Some(&leaf_loader),
        );
        let middle_loader = ModuleLoader(vec![("//middle:defs.bzl".to_owned(), middle)]);
        let error = evaluate(
            "load(\"//middle:defs.bzl\", \"helper\")\nhelper()",
            Some(&middle_loader),
        )
        .unwrap_err();
        assert!(
            error.contains("may only be set at the top level"),
            "{error}"
        );
    }

    #[test]
    fn every_live_composition_site_calls_the_shared_checker() {
        // One definition plus the five sites enumerated in the accepted owner packet.
        assert_eq!(
            include_str!("bzl_module.rs")
                .matches("validate_direct_bzl_load_visibilities(")
                .count(),
            6
        );
    }
}
