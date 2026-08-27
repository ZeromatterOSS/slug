use std::sync::Arc;

use compact_str::CompactString;
use slug_bzlmod_v2::LogicalModuleFileId;
use slug_bzlmod_v2::LogicalSpan;
use slug_bzlmod_v2::ModuleRegistrationPattern;
use slug_bzlmod_v2::NonrootAttributeKey;
use slug_bzlmod_v2::NonrootAttributeValue;
use slug_bzlmod_v2::NonrootDependency;
use slug_bzlmod_v2::NonrootExtensionIsolationKey;
use slug_bzlmod_v2::NonrootExtensionProxy;
use slug_bzlmod_v2::NonrootExtensionTag;
use slug_bzlmod_v2::NonrootExtensionUsage;
use slug_bzlmod_v2::NonrootModuleBuilder;
use slug_bzlmod_v2::NonrootModuleKey;
use slug_bzlmod_v2::NonrootRepoImports;
use slug_bzlmod_v2::NonrootRepoOverride;
use slug_bzlmod_v2::inspect_nonroot_module_file;
use starlark_map::small_map::SmallMap;

fn span(file: &str, line: u32, column: u32) -> LogicalSpan {
    LogicalSpan {
        file: LogicalModuleFileId::new(file),
        start_line: line,
        start_column: column,
        end_line: line,
        end_column: column + 1,
    }
}

#[test]
fn finalized_nonroot_module_keeps_every_compact_field_and_order() {
    let imports = NonrootRepoImports::from_local_to_exported(SmallMap::from_iter([
        (
            CompactString::from("local_one"),
            CompactString::from("export_one"),
        ),
        (
            CompactString::from("local_two"),
            CompactString::from("export_two"),
        ),
    ]))
    .unwrap();
    let nested_attributes = Arc::new(SmallMap::from_iter([(
        CompactString::from("nested"),
        NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter([(
            NonrootAttributeKey::Label(CompactString::from("@@subject+//:key")),
            NonrootAttributeValue::List(Arc::from([
                NonrootAttributeValue::Bool(true),
                NonrootAttributeValue::integer("-100000000000000000000").unwrap(),
            ])),
        )]))),
    )]));
    let ordinary_usage = NonrootExtensionUsage {
        bzl_label: CompactString::from("@dep//:ext.bzl"),
        extension_name: CompactString::from("ext"),
        proxies: Arc::from([
            NonrootExtensionProxy {
                proxy_name: CompactString::from("first_proxy"),
                containing_file: LogicalModuleFileId::new("//:MODULE.bazel"),
                dev_dependency: false,
                location: span("//:MODULE.bazel", 4, 3),
                imports,
            },
            NonrootExtensionProxy {
                proxy_name: CompactString::from("second_proxy"),
                containing_file: LogicalModuleFileId::new("//:part.MODULE.bazel"),
                dev_dependency: true,
                location: span("//:part.MODULE.bazel", 2, 5),
                imports: NonrootRepoImports::from_local_to_exported(SmallMap::new()).unwrap(),
            },
        ]),
        tags: Arc::from([
            NonrootExtensionTag {
                tag_class: CompactString::from("first_tag"),
                attributes: Arc::clone(&nested_attributes),
                dev_dependency: false,
                location: span("//:MODULE.bazel", 5, 7),
            },
            NonrootExtensionTag {
                tag_class: CompactString::from("second_tag"),
                attributes: Arc::new(SmallMap::new()),
                dev_dependency: true,
                location: span("//:part.MODULE.bazel", 3, 7),
            },
        ]),
        repo_overrides: Arc::new(SmallMap::new()),
        isolation: Some(NonrootExtensionIsolationKey {
            module: NonrootModuleKey::new("subject", "1.0"),
            exported_proxy_name: CompactString::from("first_proxy"),
        }),
    };
    let innate_usage = NonrootExtensionUsage {
        bzl_label: CompactString::from("//:MODULE.bazel"),
        extension_name: CompactString::from("@dep//:repo.bzl repo_rule"),
        proxies: Arc::from([NonrootExtensionProxy {
            proxy_name: CompactString::new(""),
            containing_file: LogicalModuleFileId::new("//:MODULE.bazel"),
            dev_dependency: false,
            location: span("//:MODULE.bazel", 9, 1),
            imports: NonrootRepoImports::from_local_to_exported(SmallMap::from_iter([(
                CompactString::from("generated"),
                CompactString::from("generated"),
            )]))
            .unwrap(),
        }]),
        tags: Arc::from([NonrootExtensionTag {
            tag_class: CompactString::from("repo_rule"),
            attributes: Arc::new(SmallMap::from_iter([(
                CompactString::from("name"),
                NonrootAttributeValue::String(CompactString::from("generated")),
            )])),
            dev_dependency: false,
            location: span("//:MODULE.bazel", 9, 1),
        }]),
        repo_overrides: Arc::new(SmallMap::new()),
        isolation: None,
    };

    let mut builder = NonrootModuleBuilder::new(
        NonrootModuleKey::new("subject", "1.0"),
        "subject",
        "1.0",
        "subject_self",
    );
    builder.bazel_compatibility = vec!["<10.0".into(), ">=9.0".into()];
    builder.dependencies.insert(
        CompactString::from("alias"),
        NonrootDependency::new("dep", "2.0"),
    );
    builder.nodep_dependencies = vec![NonrootDependency::new("floor", "3.0")];
    builder.execution_platforms = ["//:exec_one", "//:exec_two"]
        .map(|pattern| ModuleRegistrationPattern::parse(pattern).unwrap())
        .into();
    builder.toolchains = ["//:toolchain_one", "//:toolchain_two"]
        .map(|pattern| ModuleRegistrationPattern::parse(pattern).unwrap())
        .into();
    builder.flag_aliases.insert(
        CompactString::from("compilation_mode"),
        CompactString::from("//:subject_mode"),
    );
    builder.extension_usages = vec![ordinary_usage, innate_usage];
    let module = builder.build().unwrap();

    assert_eq!(module.base.expected_key.name, "subject");
    assert_eq!(module.base.declared_name, "subject");
    assert_eq!(module.base.declared_version, "1.0");
    assert_eq!(module.base.repo_name, "subject_self");
    assert_eq!(module.base.compatibility_level, 0);
    assert_eq!(module.base.bazel_compatibility.as_ref(), ["<10.0", ">=9.0"]);
    assert_eq!(
        module
            .base
            .dependencies
            .get("alias")
            .unwrap()
            .max_compatibility_level(),
        -1
    );
    assert_eq!(
        module.base.nodep_dependencies[0].max_compatibility_level(),
        -1
    );
    assert_eq!(
        module.base.dependencies.get("bazel_tools").unwrap().name,
        "bazel_tools"
    );
    assert_eq!(
        module.base.dependencies.get("bazel_tools").unwrap().version,
        ""
    );
    assert_eq!(
        module
            .base
            .dependencies
            .get("bazel_tools")
            .unwrap()
            .max_compatibility_level(),
        -1
    );
    assert_eq!(module.base.dependencies, module.base.original_dependencies);
    assert!(Arc::ptr_eq(
        &module.base.dependencies,
        &module.base.original_dependencies
    ));
    assert_eq!(
        module
            .base
            .execution_platforms
            .iter()
            .map(ModuleRegistrationPattern::as_str)
            .collect::<Vec<_>>(),
        ["//:exec_one", "//:exec_two"]
    );
    assert_eq!(
        module
            .base
            .toolchains
            .iter()
            .map(ModuleRegistrationPattern::as_str)
            .collect::<Vec<_>>(),
        ["//:toolchain_one", "//:toolchain_two"]
    );
    assert_eq!(
        module.base.flag_aliases.get("compilation_mode").unwrap(),
        "//:subject_mode"
    );
    assert_eq!(module.extension_usages[0].extension_name, "ext");
    assert_eq!(
        module.extension_usages[0].proxies[0].proxy_name,
        "first_proxy"
    );
    assert_eq!(
        module.extension_usages[0].proxies[1].containing_file.0,
        "//:part.MODULE.bazel"
    );
    assert!(module.extension_usages[0].proxies[1].dev_dependency);
    assert_eq!(module.extension_usages[0].tags[0].tag_class, "first_tag");
    assert!(module.extension_usages[0].tags[1].dev_dependency);
    assert_eq!(
        module.extension_usages[0]
            .isolation
            .as_ref()
            .unwrap()
            .exported_proxy_name,
        "first_proxy"
    );
    assert_eq!(
        module.extension_usages[1].extension_name,
        "@dep//:repo.bzl repo_rule"
    );
    assert!(
        module
            .extension_usages
            .iter()
            .all(|usage| usage.repo_overrides.is_empty())
    );

    let mut reordered = module.clone();
    reordered.base.toolchains = Arc::from([
        ModuleRegistrationPattern::parse("//:toolchain_two").unwrap(),
        ModuleRegistrationPattern::parse("//:toolchain_one").unwrap(),
    ]);
    assert_ne!(module, reordered);
    let mut relocated = module.clone();
    relocated.extension_usages = {
        let mut usages = relocated.extension_usages.to_vec();
        let mut tags = usages[0].tags.to_vec();
        tags[0].location.start_column += 1;
        usages[0].tags = tags.into();
        usages.into()
    };
    assert_ne!(module, relocated);
}

#[test]
fn attribute_integers_preserve_small_large_signed_and_nested_values() {
    let NonrootAttributeValue::Int(max) = NonrootAttributeValue::integer("2147483647").unwrap()
    else {
        panic!("integer constructor returned a non-integer value");
    };
    assert_eq!(max.as_i32(), Some(i32::MAX));
    assert_eq!(max.to_decimal(), "2147483647");
    let NonrootAttributeValue::Int(min) = NonrootAttributeValue::integer("-2147483648").unwrap()
    else {
        panic!("integer constructor returned a non-integer value");
    };
    assert_eq!(min.as_i32(), Some(i32::MIN));
    assert_eq!(min.to_decimal(), "-2147483648");

    let NonrootAttributeValue::Int(large) =
        NonrootAttributeValue::integer("100000000000000000000").unwrap()
    else {
        panic!("integer constructor returned a non-integer value");
    };
    assert_eq!(large.as_i32(), None);
    assert_eq!(large.to_decimal(), "100000000000000000000");
    let NonrootAttributeValue::Int(negative_large) =
        NonrootAttributeValue::integer("-100000000000000000000").unwrap()
    else {
        panic!("integer constructor returned a non-integer value");
    };
    assert_eq!(negative_large.as_i32(), None);
    assert_eq!(negative_large.to_decimal(), "-100000000000000000000");

    for noncanonical in ["", "+1", "01", "-0", "-01"] {
        assert!(NonrootAttributeValue::integer(noncanonical).is_err());
    }
    let nested = NonrootAttributeValue::List(Arc::from([
        NonrootAttributeValue::integer("2147483648").unwrap(),
        NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter([(
            NonrootAttributeKey::String(CompactString::from("negative")),
            NonrootAttributeValue::integer("-2147483649").unwrap(),
        )]))),
    ]));
    let NonrootAttributeValue::List(values) = nested else {
        panic!("nested value was not retained as a list");
    };
    let NonrootAttributeValue::Int(value) = &values[0] else {
        panic!("nested integer was not retained as an integer");
    };
    assert_eq!(value.as_i32(), None);
    assert_eq!(value.to_decimal(), "2147483648");
}

#[test]
fn retained_attributes_keep_list_tuple_identity_and_order_insensitive_dict_equality() {
    let list =
        NonrootAttributeValue::List(Arc::from([NonrootAttributeValue::String("value".into())]));
    let tuple =
        NonrootAttributeValue::Tuple(Arc::from([NonrootAttributeValue::String("value".into())]));
    assert_ne!(list, tuple);

    let first = NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter([
        (
            NonrootAttributeKey::String("first".into()),
            NonrootAttributeValue::String("one".into()),
        ),
        (
            NonrootAttributeKey::String("second".into()),
            NonrootAttributeValue::String("two".into()),
        ),
    ])));
    let second = NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter([
        (
            NonrootAttributeKey::String("second".into()),
            NonrootAttributeValue::String("two".into()),
        ),
        (
            NonrootAttributeKey::String("first".into()),
            NonrootAttributeValue::String("one".into()),
        ),
    ])));
    assert_eq!(first, second);
}

#[test]
fn extension_imports_keep_a_compact_bimap_without_duplicate_exports() {
    let imports = NonrootRepoImports::from_local_to_exported(SmallMap::from_iter([
        (
            CompactString::from("local_one"),
            CompactString::from("export_one"),
        ),
        (
            CompactString::from("local_two"),
            CompactString::from("export_two"),
        ),
    ]))
    .unwrap();
    assert_eq!(
        imports.exported_to_local.get("export_one").unwrap(),
        "local_one"
    );
    assert!(
        NonrootRepoImports::from_local_to_exported(SmallMap::from_iter([
            (CompactString::from("one"), CompactString::from("same")),
            (CompactString::from("two"), CompactString::from("same")),
        ]))
        .is_err()
    );
}

#[test]
fn finalization_inserts_or_skips_builtin_and_rejects_all_builtin_collisions() {
    let module = NonrootModuleBuilder::new(
        NonrootModuleKey::new("subject", "1.0"),
        "subject",
        "1.0",
        "subject",
    )
    .build()
    .unwrap();
    assert!(module.base.dependencies.contains_key("bazel_tools"));

    let builtin = NonrootModuleBuilder::new(
        NonrootModuleKey::new("bazel_tools", ""),
        "bazel_tools",
        "",
        "bazel_tools",
    )
    .build()
    .unwrap();
    assert!(!builtin.base.dependencies.contains_key("bazel_tools"));

    let mut declared_collision = NonrootModuleBuilder::new(
        NonrootModuleKey::new("subject", "1.0"),
        "subject",
        "1.0",
        "subject",
    );
    declared_collision.dependencies.insert(
        CompactString::from("bazel_tools"),
        NonrootDependency::new("other", "1.0"),
    );
    assert!(declared_collision.build().is_err());

    let self_repo_collision = NonrootModuleBuilder::new(
        NonrootModuleKey::new("subject", "1.0"),
        "subject",
        "1.0",
        "bazel_tools",
    );
    assert!(self_repo_collision.build().is_err());

    let mut import_collision = NonrootModuleBuilder::new(
        NonrootModuleKey::new("subject", "1.0"),
        "subject",
        "1.0",
        "subject",
    );
    import_collision
        .extension_usages
        .push(NonrootExtensionUsage {
            bzl_label: CompactString::from("//:ext.bzl"),
            extension_name: CompactString::from("ext"),
            proxies: Arc::from([NonrootExtensionProxy {
                proxy_name: CompactString::from("proxy"),
                containing_file: LogicalModuleFileId::new("//:MODULE.bazel"),
                dev_dependency: false,
                location: span("//:MODULE.bazel", 1, 1),
                imports: NonrootRepoImports::from_local_to_exported(SmallMap::from_iter([(
                    CompactString::from("bazel_tools"),
                    CompactString::from("generated"),
                )]))
                .unwrap(),
            }]),
            tags: Arc::from([]),
            repo_overrides: Arc::new(SmallMap::new()),
            isolation: None,
        });
    assert!(import_collision.build().is_err());

    let mut empty_usage = NonrootModuleBuilder::new(
        NonrootModuleKey::new("subject", "1.0"),
        "subject",
        "1.0",
        "subject",
    );
    empty_usage.extension_usages.push(NonrootExtensionUsage {
        bzl_label: CompactString::from("//:MODULE.bazel"),
        extension_name: CompactString::from("//:unused.bzl unused_rule"),
        proxies: Arc::from([]),
        tags: Arc::from([]),
        repo_overrides: Arc::new(SmallMap::new()),
        isolation: None,
    });
    assert!(empty_usage.build().unwrap().extension_usages.is_empty());
}

#[test]
fn repo_override_shape_is_complete_but_nonroot_finalization_rejects_it() {
    let location = span("//:MODULE.bazel", 7, 9);
    let mut overrides = SmallMap::new();
    overrides.insert(
        CompactString::from("generated"),
        NonrootRepoOverride {
            overriding_repo_name: CompactString::from("replacement"),
            must_exist: true,
            location: location.clone(),
        },
    );
    assert_eq!(
        overrides.get("generated").unwrap().overriding_repo_name,
        "replacement"
    );
    assert!(overrides.get("generated").unwrap().must_exist);
    assert_eq!(overrides.get("generated").unwrap().location, location);

    let mut builder = NonrootModuleBuilder::new(
        NonrootModuleKey::new("subject", "1.0"),
        "subject",
        "1.0",
        "subject",
    );
    builder.extension_usages.push(NonrootExtensionUsage {
        bzl_label: CompactString::from("//:ext.bzl"),
        extension_name: CompactString::from("ext"),
        proxies: Arc::from([]),
        tags: Arc::from([]),
        repo_overrides: Arc::new(overrides),
        isolation: None,
    });
    assert!(builder.build().is_err());
}

#[test]
fn inspector_collects_only_direct_unshadowed_literal_includes_with_logical_spans() {
    let inspection = inspect_nonroot_module_file(
        LogicalModuleFileId::new("registry:subject@1.0/MODULE.bazel"),
        b"include(\"first.MODULE.bazel\")\nthing.include(\"not-an-include\")\ninclude = \"shadowed\"\ninclude(\"also-not-an-include\")\n",
    )
    .unwrap();

    assert_eq!(inspection.includes.len(), 1);
    assert_eq!(inspection.includes[0].path, "first.MODULE.bazel");
    assert_eq!(
        inspection.includes[0].location.file.0,
        "registry:subject@1.0/MODULE.bazel"
    );
    assert_eq!(inspection.includes[0].location.start_line, 1);
    assert_eq!(inspection.includes[0].location.start_column, 1);
}

#[test]
fn inspector_rejects_restricted_syntax_and_invalid_include_forms() {
    for source in [
        b"load(\"//:x.bzl\", \"x\")".as_slice(),
        b"def f():\n  pass".as_slice(),
        b"x = lambda: 1".as_slice(),
        b"if True:\n  pass".as_slice(),
        b"f(*args)".as_slice(),
        b"f(**kwargs)".as_slice(),
        b"include(variable)".as_slice(),
        b"include.tag()".as_slice(),
        b"x = include".as_slice(),
        b"x = [v for include in values]".as_slice(),
        b"x = include\ninclude = \"too late\"".as_slice(),
        b"f(g(*args))".as_slice(),
    ] {
        assert!(inspect_nonroot_module_file(LogicalModuleFileId::new("logical"), source).is_err());
    }
    assert!(
        inspect_nonroot_module_file(
            LogicalModuleFileId::new("logical"),
            b"f(**{\"literal\": 1})",
        )
        .is_ok()
    );
    for source in [
        b"include(*args)".as_slice(),
        b"include(f(*args))".as_slice(),
    ] {
        let error =
            inspect_nonroot_module_file(LogicalModuleFileId::new("logical"), source).unwrap_err();
        assert!(error.to_string().contains("include() requires exactly one"));
        assert!(!error.to_string().contains("*args is not permitted"));
    }
    assert!(
        inspect_nonroot_module_file(
            LogicalModuleFileId::new("logical"),
            b"include = \"shadowed\"\ninclude.tag()\ninclude(\"ordinary call\")",
        )
        .is_ok()
    );
}

#[test]
fn inspector_does_not_retain_or_require_physical_paths() {
    let error = inspect_nonroot_module_file(
        LogicalModuleFileId::new("registry:subject@1.0/MODULE.bazel"),
        &[0xff],
    )
    .unwrap_err();
    assert!(error.to_string().contains("UTF-8"));
    assert!(!error.to_string().contains("/run/media"));

    let logical_id = LogicalModuleFileId::new("//:MODULE.bazel");
    let first = inspect_nonroot_module_file(
        logical_id.clone(),
        b"include(\"//:part.MODULE.bazel\") # first bytes\n",
    )
    .unwrap();
    let second = inspect_nonroot_module_file(
        logical_id,
        b"include(\"//:part.MODULE.bazel\") # other bytes\n",
    )
    .unwrap();
    assert_eq!(first, second);
    let relocated = inspect_nonroot_module_file(
        LogicalModuleFileId::new("//:other.MODULE.bazel"),
        b"include(\"//:part.MODULE.bazel\") # first bytes\n",
    )
    .unwrap();
    assert_ne!(first, relocated);
}
