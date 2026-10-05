/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::collections::HashMap;

use dupe::Dupe;
use starlark_syntax::syntax::module::AstModuleFields;

use super::ModuleScopeBuilder;
use super::ModuleScopes;
use super::ScopeId;
use super::scope_resolver_globals::ScopeResolverGlobals;
use crate::environment::Globals;
use crate::environment::GlobalsBuilder;
use crate::environment::names::MutableNames;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::FrozenHeap;

fn named_globals(names: &[&str]) -> Globals {
    let mut builder = GlobalsBuilder::new();
    for name in names {
        builder.set(name, 7);
    }
    builder.build()
}

#[test]
fn global_suggestion_names_share_existing_frozen_bytes() {
    let names: Vec<_> = (0..32)
        .map(|i| format!("name_{i:02}_{}", "x".repeat(1016)))
        .collect();
    assert!(names.iter().all(|s| s.len() == 1024));
    let heap = FrozenHeap::new();
    let globals = heap.alloc_any(named_globals(
        &names.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    ));
    let resolver = ScopeResolverGlobals {
        globals: Some(globals),
    };
    let actual = resolver.names().unwrap();
    let bytes: Vec<&str> = actual.iter().map(AsRef::<str>::as_ref).collect();
    assert_eq!(bytes, names.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    let shared = actual
        .iter()
        .zip(globals.names())
        .filter(|(a, b)| a.as_ptr() == b.as_str().as_ptr())
        .count();
    println!(
        "SCOPE_NAMES_GLOBAL count={} name_bytes={} shared={shared}",
        actual.len(),
        bytes.iter().map(|s| s.len()).sum::<usize>()
    );
    assert_eq!(shared, 32);
    assert!(ScopeResolverGlobals::unknown().names().is_none());
    let empty = ScopeResolverGlobals {
        globals: Some(heap.alloc_any(named_globals(&[]))),
    };
    assert!(empty.names().unwrap().is_empty());
}

#[test]
fn nested_scope_suggestion_names_preserve_order_duplicates_and_owners() {
    let source = "module_first = 1\nshared = 2\nmodule_last = 3\ndef outer(outer_first, shared):\n    outer_last = 4\n    def inner(inner_first, shared):\n        inner_last = 5\n        return inner_first\n    return inner\n";
    let ast = AstModule::parse(
        "scope-names.bzl",
        source.to_owned(),
        &Dialect::AllOptionsInternal,
    )
    .unwrap();
    let heap = FrozenHeap::new();
    let module = MutableNames::new();
    let codemap = heap.alloc_any(ast.codemap().dupe());
    let globals = heap.alloc_any(named_globals(&["global_last", "shared", "global_first"]));
    let (_, mut scope) = ModuleScopeBuilder::enter_module(
        &module,
        &heap,
        &HashMap::new(),
        ast.into_parts().1,
        ScopeResolverGlobals {
            globals: Some(globals),
        },
        codemap,
        &Dialect::AllOptionsInternal,
    );
    assert!(scope.errors.is_empty());
    let find_scope = |name| {
        ScopeId(
            scope
                .scope_data
                .scopes
                .iter()
                .position(|s| s.mp.keys().any(|s| s.as_str() == name))
                .unwrap(),
        )
    };
    let outer = find_scope("outer_first");
    let inner = find_scope("inner_first");
    scope.locals.extend([outer, inner]);
    {
        let actual = scope
            .current_scope_all_visible_names_for_did_you_mean()
            .unwrap();
        let bytes: Vec<&str> = actual.iter().map(AsRef::<str>::as_ref).collect();
        assert_eq!(
            bytes,
            [
                "inner_first",
                "shared",
                "inner_last",
                "outer_first",
                "shared",
                "outer_last",
                "inner",
                "module_first",
                "shared",
                "module_last",
                "outer",
                "global_first",
                "global_last",
                "shared"
            ]
        );
        let owners: Vec<_> = scope
            .locals
            .iter()
            .rev()
            .flat_map(|id| {
                scope
                    .scope_data
                    .get_scope(*id)
                    .mp
                    .keys()
                    .map(|n| n.as_str())
            })
            .chain(scope.module_bindings.keys().map(|n| n.as_str()))
            .chain(globals.names().map(|n| n.as_str()))
            .collect();
        assert_eq!(bytes, owners);
        let shared = actual
            .iter()
            .zip(&owners)
            .filter(|(a, b)| a.as_ptr() == b.as_ptr())
            .count();
        println!("SCOPE_NAMES_VISIBLE count={} shared={shared}", actual.len());
        assert_eq!(shared, actual.len());
    }
    scope.globals = ScopeResolverGlobals::unknown();
    assert!(
        scope
            .current_scope_all_visible_names_for_did_you_mean()
            .is_none()
    );
}

fn retained_scope_error(source: String, globals: Globals) -> crate::Error {
    let ast = AstModule::parse(
        "retained-scope-names.bzl",
        source,
        &Dialect::AllOptionsInternal,
    )
    .unwrap();
    let heap = FrozenHeap::new();
    let module = MutableNames::new();
    let codemap = heap.alloc_any(ast.codemap().dupe());
    let globals = ScopeResolverGlobals {
        globals: Some(heap.alloc_any(globals)),
    };
    ModuleScopes::check_module_err(
        &module,
        &heap,
        &HashMap::new(),
        ast.into_parts().1,
        globals,
        codemap,
        &Dialect::AllOptionsInternal,
    )
    .err()
    .unwrap()
}

#[test]
fn suggestion_ties_and_owned_diagnostics_survive_source_owner_drop() {
    assert!(ScopeResolverGlobals::unknown().names().is_none());
    let heap = FrozenHeap::new();
    let empty = ScopeResolverGlobals {
        globals: Some(heap.alloc_any(named_globals(&[]))),
    };
    assert!(empty.names().unwrap().is_empty());
    let cases = [
        (
            "aaaax",
            named_globals(&["aaaaz", "aaaay"]),
            "Variable `aaaax` not found, did you mean `aaaay`?",
            "aaaax",
        ),
        (
            "aaaaz = 0\ndef f(aaaay): return aaaax",
            named_globals(&[]),
            "Variable `aaaax` not found, did you mean `aaaay`?",
            "aaaax",
        ),
        (
            "aaaaw = 0\ndef f(aaaaz):\n    def g(aaaay): return aaaax\n    return g",
            named_globals(&[]),
            "Variable `aaaax` not found, did you mean `aaaay`?",
            "aaaax",
        ),
        (
            "def f(aaaaz): return [aaaax for aaaay in []]",
            named_globals(&[]),
            "Variable `aaaax` not found, did you mean `aaaaz`?",
            "aaaax",
        ),
        (
            "a_very_missing_name",
            named_globals(&[]),
            "Variable `a_very_missing_name` not found",
            "a_very_missing_name",
        ),
        (
            "\"éé\" + aaaax",
            named_globals(&["aaaay"]),
            "Variable `aaaax` not found, did you mean `aaaay`?",
            "aaaax",
        ),
    ];
    let errors: Vec<_> = cases
        .into_iter()
        .map(|(source, globals, message, missing)| {
            (
                retained_scope_error(source.to_owned(), globals),
                message,
                missing,
            )
        })
        .collect();
    for (error, message, missing) in errors {
        assert_eq!(error.without_diagnostic().to_string(), message);
        let span = error.span().unwrap();
        assert_eq!(span.source_span(), missing);
        assert_eq!(span.filename(), "retained-scope-names.bzl");
        assert!(error.to_string().contains(message));
    }
}
