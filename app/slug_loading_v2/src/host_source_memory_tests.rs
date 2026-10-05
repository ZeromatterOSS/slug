use dice::DetectCycles;
use dice::Dice;
use slug_bzlmod_v2::RootPackageBzlTarget;
use slug_bzlmod_v2::RootPackageSourceKey;
use slug_identity_v2::PackagePath;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::syntax::StringEncoding;

use super::super::HostSourceInputError;
use super::super::digest;
use super::super::host_source_name;
use super::super::host_source_text;
use super::EpochBuilder;
use super::transaction;
use super::workspace;
use crate::LoadingPreparationOutcome;

#[tokio::test]
async fn host_source_view_shares_bytes_and_parser_owns_diagnostics() {
    let text = "load(':one.bzl', 'ONE')\nload(':two.bzl', 'TWO')\nvalue = 'é'\n";
    let ast = {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let epoch = EpochBuilder::workspace_sources(
            "",
            "filegroup(name='x')\n",
            &[("defs.bzl", text)],
            270,
        );
        let mut transaction = transaction(&dice, epoch.build(), false, None).await;
        let value = transaction
            .compute(&RootPackageSourceKey::for_bzl(
                workspace(),
                PackagePath::parse("pkg").unwrap(),
                RootPackageBzlTarget::parse("defs.bzl").unwrap(),
            ))
            .await
            .unwrap();
        let LoadingPreparationOutcome::Complete(result) = value else {
            panic!("complete source fixture returned Need");
        };
        let source = result.as_ref().as_ref().unwrap();
        let decoded = host_source_text(source).unwrap();
        assert_eq!(decoded.as_ptr(), source.bytes().as_ptr());
        let view: &str = &decoded;
        assert_eq!(view, text);
        assert_eq!(digest(view), digest(text));
        let ast = AstModule::parse_with_string_encoding(
            &host_source_name(source).unwrap(),
            view.to_owned(),
            &Dialect::Bazel,
            StringEncoding::BazelInternal,
        )
        .unwrap();
        assert_ne!(
            ast.file_span(ast.statement().span).file.source().as_ptr(),
            source.bytes().as_ptr(),
        );
        ast
    };
    assert_eq!(ast.file_span(ast.statement().span).file.source(), text);
    assert_eq!(
        ast.loads()
            .iter()
            .map(|load| load.module_id)
            .collect::<Vec<_>>(),
        [":one.bzl", ":two.bzl"],
    );
}

#[tokio::test]
async fn host_source_encoding_error_retains_the_logical_path() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut epoch = EpochBuilder::workspace_sources("", "filegroup(name='x')\n", &[], 271);
    epoch.file("/workspace/pkg/defs.bzl", [0xffu8], 271);
    let mut transaction = transaction(&dice, epoch.build(), false, None).await;
    let value = transaction
        .compute(&RootPackageSourceKey::for_bzl(
            workspace(),
            PackagePath::parse("pkg").unwrap(),
            RootPackageBzlTarget::parse("defs.bzl").unwrap(),
        ))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(result) = value else {
        panic!("complete source fixture returned Need");
    };
    assert!(matches!(
        host_source_text(result.as_ref().as_ref().unwrap()),
        Err(HostSourceInputError::UnsupportedSourceEncoding { logical_path })
            if logical_path.as_path() == std::path::Path::new("/workspace/pkg/defs.bzl")
    ));
}
