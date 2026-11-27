//! Existing artifact owners and scratch borrowing; no path-admission changes.
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::WorkspaceStatusArtifacts;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark::environment::Globals;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

fn artifacts() -> Vec<AnalysisArtifact> {
    let mut result = [
        "@@//:root.h",
        "@@//:nested/root.h",
        "@@//:.h",
        "@@//pkg:source.雪.h",
        "@@dep+//pkg/sub:dir/file.h",
        "@@dep+//:.hidden",
    ]
    .map(|label| AnalysisArtifact::Source(CanonicalLabel::parse(label).unwrap()))
    .to_vec();
    let long = "name_é雪".repeat(1024) + ".h";
    for label in [format!("@@//:{long}"), format!("@@dep+//pkg:dir/{long}")] {
        result.push(AnalysisArtifact::Source(
            CanonicalLabel::parse(&label).unwrap(),
        ));
    }
    let owner = AnalysisConfiguredTargetKey::new(
        CanonicalLabel::parse("@@//:owner").unwrap(),
        b"config".as_slice(),
    );
    for (path, kind) in [
        ("generated/file.o".to_owned(), ActionOutputKind::File),
        (
            "external/dep+/pkg/.h".to_owned(),
            ActionOutputKind::Directory,
        ),
        ("name_🙂".repeat(1024) + ".cc", ActionOutputKind::Symlink),
    ] {
        result.push(AnalysisArtifact::Derived {
            owner: owner.clone(),
            output: ActionOutput::new(crate::artifact_root::test_root(), path, kind),
        });
    }
    let status = WorkspaceStatusArtifacts::new(NormalizedAbsolutePath::new("/workspace").unwrap());
    result.extend(
        [status.stable(), status.volatile()]
            .map(|status| AnalysisArtifact::WorkspaceStatus(status.clone())),
    );
    result
}

#[test]
fn frozen_file_admits_source_clones_and_reuses_shared_owners() {
    let heap = FrozenHeap::new();
    let baseline = AnalysisArtifact::Source(CanonicalLabel::parse("@@//:source").unwrap());
    let object_bytes = (0..8)
        .find_map(|_| {
            let (reserved, available) = (heap.allocated_bytes(), heap.available_bytes());
            heap.alloc(AnalysisArtifactValue::new(baseline.clone()));
            (heap.allocated_bytes() == reserved).then(|| available - heap.available_bytes())
        })
        .expect("a File allocation in an open chunk");
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("active-map").unwrap());
    let source = ApparentLabel::parse("@external//pkg/sub:source.雪").unwrap();
    let mut values = artifacts();
    values.push(AnalysisArtifact::Source(source.resolve(&mapping)));
    for artifact in values {
        let bytes = object_bytes
            + match &artifact {
                AnalysisArtifact::Source(label) => label.cloned_text_lengths().iter().sum(),
                AnalysisArtifact::Derived { .. } | AnalysisArtifact::WorkspaceStatus(_) => 0,
            };
        let heap = FrozenHeap::new();
        let short = ExecutionBudget::new(20_000, bytes - 1, 8);
        assert_eq!(
            AnalysisArtifactValue::alloc_frozen_budgeted(&heap, &short, &artifact).unwrap_err(),
            ExecutionLimit::AllocationBytes
        );
        assert_eq!(heap.allocated_bytes(), 0);

        let exact = ExecutionBudget::new(20_000, bytes, 8);
        let value = AnalysisArtifactValue::alloc_frozen_budgeted(&heap, &exact, &artifact).unwrap();
        let restored = AnalysisArtifactValue::from_starlark(value.to_value()).unwrap();
        assert_eq!(restored.artifact(), &artifact);
        assert_eq!(exact.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        if let (
            AnalysisArtifact::Derived { output, .. },
            AnalysisArtifact::Derived {
                output: original, ..
            },
        ) = (restored.artifact(), &artifact)
        {
            assert_eq!(output.path().as_ptr(), original.path().as_ptr());
        }
    }
}

#[test]
fn file_helpers_borrow_existing_path_and_basename_owners() {
    let mut path_matches = 0;
    let mut borrowed_paths = 0;
    let mut basename_matches = 0;
    let values = artifacts();
    for artifact in &values {
        let value = AnalysisArtifactValue::new(artifact.clone());
        let path = value.path();
        if let std::borrow::Cow::Borrowed(expected) = value.artifact.path() {
            borrowed_paths += 1;
            assert_eq!(path.as_ref() as &str, expected);
            path_matches += usize::from(path.as_ptr() == expected.as_ptr());
        }
        let source = match &value.artifact {
            AnalysisArtifact::Source(label) => label.target().as_str(),
            AnalysisArtifact::Derived { output, .. } => output.path(),
            AnalysisArtifact::WorkspaceStatus(status) => status.path(),
        };
        let expected = source.rsplit('/').next().unwrap();
        let basename = value.basename();
        assert_eq!(basename.as_ref() as &str, expected);
        basename_matches += usize::from(basename.as_ptr() == expected.as_ptr());
    }
    assert_eq!(values.len(), 13);
    assert_eq!(borrowed_paths, 9);
    println!(
        "FILE_PATH_BORROWED={borrowed_paths} PATH_MATCHES={path_matches} BASENAME_MATCHES={basename_matches}"
    );
    assert_eq!(path_matches, borrowed_paths);
    assert_eq!(basename_matches, values.len());
}

#[test]
fn file_properties_preserve_bytes_type_hash_errors_and_frozen_results() {
    let mut held = Vec::new();
    for artifact in artifacts() {
        let path = artifact.path().into_owned();
        let extension = path
            .rfind('.')
            .filter(|dot| *dot != 0 && path.rfind('/').is_none_or(|slash| *dot > slash))
            .map_or("", |dot| &path[dot + 1..]);
        let expected = vec![
            path.clone(),
            path.clone(),
            path.rsplit('/').next().unwrap().to_owned(),
            artifact.dirname(),
            artifact.short_path().into_owned(),
            extension.to_owned(),
        ];
        let value = AnalysisArtifactValue::new(artifact.clone());
        assert_eq!(value.path().as_ref() as &str, path);
        assert_eq!(value.basename().as_ref() as &str, expected[2]);
        assert_eq!(format!("{value}"), path);
        drop(value);
        let module = Module::new();
        module.set(
            "file",
            module
                .heap()
                .alloc_simple(AnalysisArtifactValue::new(artifact)),
        );
        Evaluator::new(&module).eval_module(
            AstModule::parse("file.bzl", "RESULT=(str(file),file.path,file.basename,file.dirname,file.short_path,file.extension)\nfile=None".to_owned(), &Dialect::Bazel).unwrap(),
            &crate::package::loading_globals(),
        ).unwrap();
        let frozen = module.freeze().unwrap();
        held.push((frozen.get("RESULT").unwrap(), expected));
        drop(frozen);
    }
    let heap = FrozenHeap::new();
    let mut fields = 0;
    for (result, expected) in &held {
        let tuple = TupleRef::from_value(result.value()).unwrap();
        assert_eq!(tuple.len(), expected.len());
        for (actual, expected) in tuple.iter().zip(expected) {
            assert_eq!(actual.unpack_str(), Some(expected.as_str()));
            assert_eq!(actual.get_type(), "string");
            assert_eq!(
                actual.get_hashed().unwrap().hash(),
                heap.alloc_str(expected)
                    .to_value()
                    .get_hashed()
                    .unwrap()
                    .hash()
            );
            fields += 1;
        }
    }
    assert_eq!(fields, 78);
    let artifact = AnalysisArtifact::Source(CanonicalLabel::parse("@@//:root.h").unwrap());
    let mut errors = Vec::new();
    for source in [
        "file.path()",
        "file.basename()",
        "file.dirname()",
        "file.path='bad'",
        "file.basename='bad'",
        "file.unknown",
    ] {
        let module = Module::new();
        module.set(
            "file",
            module
                .heap()
                .alloc_simple(AnalysisArtifactValue::new(artifact.clone())),
        );
        let error = Evaluator::new(&module)
            .eval_module(
                AstModule::parse("file.bzl", source.to_owned(), &Dialect::Bazel).unwrap(),
                &crate::package::loading_globals(),
            )
            .unwrap_err()
            .to_string();
        assert!(!error.is_empty());
        errors.push(error);
    }
    println!("FILE_PATH_ERRORS={errors:?}");
}

#[test]
fn bounded_file_short_path_matches_ordinary_dot_getattr_and_host_access() {
    for artifact in artifacts() {
        let expected = artifact.short_path();
        let ordinary = Module::new();
        let budget = Arc::new(ExecutionBudget::new(4096, 1 << 20, 16).with_bounded_operations());
        let bounded = Module::new_with_execution_budget(budget.clone());
        for module in [&ordinary, &bounded] {
            let heap = module.heap();
            let file = heap.alloc_simple(AnalysisArtifactValue::new(artifact.clone()));
            let host = file.get_attr("short_path", heap).unwrap().unwrap();
            assert_eq!(host.unpack_str(), Some(expected.as_ref()));
            assert!(file.get_attr("unknown", heap).unwrap().is_none());

            module.set("file", file);
            let ast = AstModule::parse(
                "file.bzl",
                "(file.short_path, getattr(file, 'short_path'), getattr(file, 'unknown', 'fallback'))"
                    .to_owned(),
                &Dialect::Bazel,
            )
            .unwrap();
            let value = Evaluator::new(module)
                .eval_module(ast, &Globals::standard())
                .unwrap();
            let result = TupleRef::from_value(value).unwrap();
            assert!(result.iter().map(|value| value.unpack_str()).eq([
                Some(expected.as_ref()),
                Some(expected.as_ref()),
                Some("fallback"),
            ]));
        }
        assert_eq!(budget.check(), Ok(()));
    }
}

#[test]
fn bounded_file_refuses_known_unadmitted_attribute_through_every_read_route() {
    let artifact = AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg:file").unwrap());
    let refused = ExecutionLimit::UnsupportedOperation {
        operation: "attribute access",
        value_type: "File",
    };
    for source in [None, Some("file.path"), Some("getattr(file, 'path', None)")] {
        let budget = Arc::new(ExecutionBudget::new(4096, 1 << 20, 16).with_bounded_operations());
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let file = heap.alloc_simple(AnalysisArtifactValue::new(artifact.clone()));
        let error = match source {
            Some(source) => {
                module.set("file", file);
                Evaluator::new(&module)
                    .eval_module(
                        AstModule::parse("file.bzl", source.to_owned(), &Dialect::Bazel).unwrap(),
                        &Globals::standard(),
                    )
                    .unwrap_err()
            }
            None => file.get_attr("path", heap).unwrap_err(),
        };
        match error.kind() {
            starlark::ErrorKind::Native(error) | starlark::ErrorKind::Other(error) => {
                assert_eq!(error.downcast_ref::<ExecutionLimit>(), Some(&refused));
            }
            error => panic!("unexpected error: {error:?}"),
        }
        assert_eq!(budget.check(), Err(refused));
    }
}
