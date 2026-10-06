//! Existing artifact owners and scratch borrowing; no path-admission changes.
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::WorkspaceStatusArtifacts;
use slug_workspace_v2::NormalizedAbsolutePath;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::FrozenHeap;

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
