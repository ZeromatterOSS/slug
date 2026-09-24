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

use allocative::Allocative;
use slug_identity_v2::CanonicalLabel;
use starlark::any::ProvidesStaticType;
use starlark::environment::GlobalsBuilder;
use starlark::environment::GlobalsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;
use starlark::values::structs::AllocStruct;

use crate::provider::BuiltinProviderKey;

// Bazel 9.2 builtins_bzl/common/python/py_internal.bzl:133-163. The
// source-defined field shape is admitted here; these functions' behavior is not.
const DEFERRED_FUNCTIONS: [&str; 24] = [
    "add_py_extra_pseudo_action",
    "are_action_listeners_enabled",
    "copy_without_caching",
    "create_repo_mapping_manifest",
    "create_sources_only_manifest",
    "declare_constant_metadata_file",
    "declare_shareable_artifact",
    "expand_location_and_make_variables",
    "get_current_os_name",
    "get_label_repo_runfiles_path",
    "get_legacy_external_runfiles",
    "get_rule_name",
    "is_available_for",
    "is_bzlmod_enabled",
    "is_singleton_depset",
    "is_tool_configuration",
    "linkstamp_file",
    "make_runfiles_respect_legacy_external_runfiles",
    "merge_runfiles_with_generated_inits_empty_files_supplier",
    "regex_match",
    "runfiles_enabled",
    "share_native_deps",
    "stamp_binaries",
    "strip_opts",
];

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct DeferredPyInternalFunction {
    name: &'static str,
}

impl fmt::Display for DeferredPyInternalFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function py_internal.{}>", self.name)
    }
}

starlark::starlark_simple_value!(DeferredPyInternalFunction);

#[starlark_value(type = "function")]
impl<'v> StarlarkValue<'v> for DeferredPyInternalFunction {
    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "py_internal.{} invocation is unsupported",
            self.name
        )))
    }
}

fn repo_contains_word(repo: &str, name: &str) -> bool {
    fn is_word(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    repo.match_indices(name).any(|(offset, _)| {
        let before = repo.as_bytes().get(offset.wrapping_sub(1));
        let after = repo.as_bytes().get(offset + name.len());
        before.is_none_or(|byte| !is_word(*byte)) && after.is_none_or(|byte| !is_word(*byte))
    })
}

pub(crate) fn allowed_source(label: &CanonicalLabel) -> bool {
    let repo = label.package().repo().as_str();
    if repo.is_empty() {
        let package = label.package().package().as_str();
        return package == "tools/build_defs/python"
            || package.starts_with("tools/build_defs/python/");
    }
    ["_builtins", "bazel_tools", "rules_python"]
        .into_iter()
        .any(|name| repo_contains_word(repo, name))
}

pub(crate) fn populate_if_allowed(builder: &mut GlobalsBuilder, label: &CanonicalLabel) {
    if !allowed_source(label) {
        return;
    }
    static PY_INTERNAL: GlobalsStatic = GlobalsStatic::new();
    let value = PY_INTERNAL.function(|singleton| {
        let provider = singleton.alloc(BuiltinProviderKey::new("PackageSpecificationInfo"));
        let cc_predicate = singleton.alloc(DeferredPyInternalFunction {
            name: "cc_helper.is_valid_shared_library_artifact",
        });
        let cc_helper = singleton.alloc(AllocStruct([(
            "is_valid_shared_library_artifact",
            cc_predicate,
        )]));
        let mut fields = vec![
            ("PackageSpecificationInfo", provider),
            ("cc_helper", cc_helper),
        ];
        fields.extend(
            DEFERRED_FUNCTIONS
                .into_iter()
                .map(|name| (name, singleton.alloc(DeferredPyInternalFunction { name }))),
        );
        singleton.set("py_internal", AllocStruct(fields));
    });
    builder.set("py_internal", value);
}

#[cfg(test)]
mod tests {
    use starlark::environment::FrozenModule;
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;
    use starlark::values::ValueLike;
    use starlark::values::list::FrozenListRef;

    use super::*;
    use crate::package::build_file_loading_globals;
    use crate::package::bzlmod_loading_globals_for_source;
    use crate::package::loading_globals;
    use crate::package::loading_globals_for_source;
    use crate::provider::starlark_provider_identity;

    fn label(value: &str) -> CanonicalLabel {
        CanonicalLabel::parse(value).unwrap()
    }

    fn evaluate(source: &str, globals: &Globals) -> Result<FrozenModule, String> {
        let ast = AstModule::parse(
            "//:py_internal_test.bzl",
            source.to_owned(),
            &Dialect::Standard,
        )
        .map_err(|error| error.to_string())?;
        let module = Module::new();
        let mut evaluator = Evaluator::new(&module);
        evaluator
            .eval_module(ast, globals)
            .map_err(|error| error.to_string())?;
        drop(evaluator);
        module.freeze().map_err(|error| format!("{error:?}"))
    }

    #[test]
    fn guard_matches_pinned_bazel_word_boundaries_and_package_prefix() {
        for allowed in [
            "@@_builtins//:a.bzl",
            "@@bazel_tools//:a.bzl",
            "@@rules_python+//:a.bzl",
            "@@rules_python++config+rules_python_internal//:a.bzl",
            "@@other+rules_python+generated//:a.bzl",
            "@@rules_python~1.7.0//:a.bzl",
            "@@//tools/build_defs/python:a.bzl",
            "@@//tools/build_defs/python/private:a.bzl",
        ] {
            let source = label(allowed);
            assert!(allowed_source(&source), "{allowed}");
            for globals in [
                loading_globals_for_source(&source),
                bzlmod_loading_globals_for_source(&source),
            ] {
                assert!(globals.names().any(|name| name.as_str() == "py_internal"));
            }
        }
        for denied in [
            "@@//:a.bzl",
            "@@//tools/build_defs/python_helper:a.bzl",
            "@@rules_python_internal//:a.bzl",
            "@@rules_python_helper+//:a.bzl",
            "@@my_rules_python+//:a.bzl",
            "@@rules_python9+//:a.bzl",
            "@@other+//:a.bzl",
        ] {
            let source = label(denied);
            assert!(!allowed_source(&source), "{denied}");
            assert!(
                !loading_globals_for_source(&source)
                    .names()
                    .any(|name| name.as_str() == "py_internal"),
                "{denied}"
            );
        }
        for globals in [loading_globals(), build_file_loading_globals()] {
            assert!(!globals.names().any(|name| name.as_str() == "py_internal"));
        }
    }

    #[test]
    fn guarded_global_has_source_defined_shape_and_deferred_calls_error() {
        let globals = loading_globals_for_source(&label("@@rules_python+//:defs.bzl"));
        let source = "EXPORTED = py_internal\nKIND = type(py_internal)\nFIELDS = dir(py_internal)\nFIELD_TYPES = [name + ':' + type(getattr(py_internal, name)) for name in FIELDS]\nPRESENT = [hasattr(py_internal, name) and getattr(py_internal, name, None) != None for name in FIELDS]\nCC_KIND = type(py_internal.cc_helper)\nCC_FIELDS = dir(py_internal.cc_helper)\nCC_FN_KIND = type(py_internal.cc_helper.is_valid_shared_library_artifact)\nKEY = py_internal.PackageSpecificationInfo\nKEY_EQUAL = KEY == PackageSpecificationInfo\nKEY_LOOKUP = {KEY: 1}[PackageSpecificationInfo]\n";
        let module = evaluate(source, &globals).unwrap();
        assert_eq!(
            module.get("KIND").unwrap().value().unpack_str(),
            Some("struct")
        );
        let fields = FrozenListRef::from_value(module.get("FIELDS").unwrap().value())
            .unwrap()
            .iter()
            .map(|value| value.to_value().unpack_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        let mut expected = DEFERRED_FUNCTIONS.map(str::to_owned).to_vec();
        expected.extend([
            "PackageSpecificationInfo".to_owned(),
            "cc_helper".to_owned(),
        ]);
        expected.sort();
        assert_eq!(fields, expected);
        let types = FrozenListRef::from_value(module.get("FIELD_TYPES").unwrap().value())
            .unwrap()
            .iter()
            .map(|value| value.to_value().unpack_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(types.len(), 26);
        for field in &fields {
            let kind = match field.as_str() {
                "PackageSpecificationInfo" => "Provider",
                "cc_helper" => "struct",
                _ => "function",
            };
            assert!(types.contains(&format!("{field}:{kind}")), "{field}");
        }
        let present = FrozenListRef::from_value(module.get("PRESENT").unwrap().value()).unwrap();
        assert!(
            present
                .iter()
                .all(|value| value.unpack_bool() == Some(true))
        );
        assert_eq!(
            module.get("CC_KIND").unwrap().value().unpack_str(),
            Some("struct")
        );
        assert_eq!(
            FrozenListRef::from_value(module.get("CC_FIELDS").unwrap().value())
                .unwrap()
                .iter()
                .map(|value| value.to_value().unpack_str().unwrap().to_owned())
                .collect::<Vec<_>>(),
            ["is_valid_shared_library_artifact"]
        );
        assert_eq!(
            module.get("CC_FN_KIND").unwrap().value().unpack_str(),
            Some("function")
        );
        assert!(
            starlark_provider_identity(module.get("KEY").unwrap().value())
                .is_some_and(|identity| identity.is_builtin("PackageSpecificationInfo"))
        );
        assert_eq!(
            module.get("KEY_EQUAL").unwrap().value().unpack_bool(),
            Some(true)
        );
        assert_eq!(
            module.get("KEY_LOOKUP").unwrap().value().unpack_i32(),
            Some(1)
        );
        for call in [
            "X = py_internal.runfiles_enabled()",
            "X = py_internal.cc_helper.is_valid_shared_library_artifact(None)",
        ] {
            let error = evaluate(call, &globals).unwrap_err();
            assert!(error.contains("invocation is unsupported"), "{error}");
        }
        let other = evaluate(
            "EXPORTED = py_internal",
            &loading_globals_for_source(&label("@@rules_python++config+internal//:other.bzl")),
        )
        .unwrap();
        assert!(
            module
                .get("EXPORTED")
                .unwrap()
                .value()
                .ptr_eq(other.get("EXPORTED").unwrap().value())
        );
    }
}
