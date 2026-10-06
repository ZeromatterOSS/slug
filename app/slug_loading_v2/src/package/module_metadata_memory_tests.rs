//! Getter-copy controls over existing recorder-owned metadata, not new admission.
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;
use crate::BzlModuleIdentity;
use crate::BzlModuleSourceProvenance;

fn evaluate(
    source: &str,
    mode: usize,
    metadata: Option<(&str, &str)>,
    call_source: bool,
    package_state: bool,
) -> anyhow::Result<FrozenModule> {
    let filename = if mode == 0 {
        "BUILD.bazel"
    } else {
        "metadata.bzl"
    };
    let recorder = PackageRecorder::new(
        PackageListing::new(Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        "",
    )
    .with_associated_module_metadata(metadata.map(|(name, version)| (name.into(), version.into())))
    .with_bzl_call_sources(call_source.then(|| {
        Arc::from([(
            filename.into(),
            BzlModuleSourceProvenance::new(
                BzlModuleIdentity {
                    label: CanonicalLabel::parse("@@dep+//:metadata.bzl").unwrap(),
                    workspace_path: filename.into(),
                    repository_mapping: Arc::from([]),
                },
                [0; 32],
            ),
        )])
    }));
    let globals = match mode {
        0 => build_file_loading_globals(),
        1 => loading_globals(),
        2 => bzlmod_loading_globals(),
        _ => unreachable!(),
    };
    let module = Module::new();
    let mut evaluator = Evaluator::new(&module);
    if package_state {
        evaluator.extra = Some(&recorder);
    }
    evaluator
        .eval_module(
            AstModule::parse(filename, source.to_owned(), &Dialect::Bazel)
                .map_err(starlark::Error::into_anyhow)?,
            &globals,
        )
        .map_err(starlark::Error::into_anyhow)?;
    assert_eq!(recorder.associated_module_metadata().ok(), metadata);
    drop(evaluator);
    drop(recorder);
    Ok(module.freeze()?)
}

#[test]
fn module_metadata_strings_preserve_bytes_type_hash_and_retained_ownership() {
    let cases = [
        (String::new(), String::new()),
        ("root".to_owned(), "1.2.3".to_owned()),
        ("dep".to_owned(), String::new()),
        ("é雪\0🙂".to_owned(), "1\0β".to_owned()),
        ("module_é雪".repeat(1024), "version_🙂".repeat(512)),
    ];
    let mut held = Vec::new();
    for mode in 0..3 {
        let source = if mode == 0 {
            "NAME=module_name()\nVERSION=module_version()\nTYPES=(type(NAME),type(VERSION))"
        } else {
            "def read(): return (native.module_name(),native.module_version())\nNAME,VERSION=read()\nTYPES=(type(NAME),type(VERSION))\nread=None"
        };
        for (name, version) in &cases {
            let frozen = evaluate(source, mode, Some((name, version)), mode != 0, true).unwrap();
            assert_eq!(
                frozen.get("TYPES").unwrap().value().to_repr(),
                "(\"string\", \"string\")"
            );
            let heap = FrozenHeap::new();
            for (field, expected) in [("NAME", name), ("VERSION", version)] {
                let value = frozen.get(field).unwrap();
                assert_eq!(value.value().unpack_str(), Some(expected.as_str()));
                assert_eq!(
                    value.value().get_hashed().unwrap().hash(),
                    heap.alloc_str(expected)
                        .to_value()
                        .get_hashed()
                        .unwrap()
                        .hash()
                );
                held.push((value, expected.clone()));
            }
            drop(frozen);
        }
    }
    drop(cases);
    for (value, expected) in &held {
        assert_eq!(value.value().unpack_str(), Some(expected.as_str()));
        assert_eq!(value.value().get_type(), "string");
    }
    assert_eq!(held.len(), 30);
}

#[test]
fn module_metadata_errors_preserve_binding_and_lookup_priority() {
    let mut errors = Vec::new();
    for mode in 0..3 {
        for name in ["module_name", "module_version"] {
            let prefix = if mode == 0 { "" } else { "native." };
            let source = |args: &str| {
                if mode == 0 {
                    format!("RESULT={prefix}{name}({args})")
                } else {
                    format!("def read(): return {prefix}{name}({args})\nRESULT=read()")
                }
            };
            for (args, metadata, call_source, package_state, expected) in [
                ("1", Some(("root", "1")), true, true, "positional arguments"),
                (
                    "unexpected=1",
                    Some(("root", "1")),
                    true,
                    true,
                    "named parameter",
                ),
                (
                    "",
                    None,
                    true,
                    true,
                    "unsupported for this repository source",
                ),
                (
                    "",
                    Some(("root", "1")),
                    true,
                    false,
                    "without package state",
                ),
            ] {
                let error = evaluate(&source(args), mode, metadata, call_source, package_state)
                    .unwrap_err()
                    .to_string();
                assert!(error.contains(expected), "{error}");
                errors.push(error);
            }
            if mode != 0 {
                let error = evaluate(&source(""), mode, None, false, true)
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("Label() can only be used"), "{error}");
                assert!(
                    !error.contains("unsupported for this repository source"),
                    "{error}"
                );
                errors.push(error);
            }
        }
    }
    assert_eq!(errors.len(), 28);
    println!("MODULE_METADATA_ERRORS={errors:?}");
}
