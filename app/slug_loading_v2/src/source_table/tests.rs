use std::mem::size_of;

use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;

use super::*;
use crate::BzlModuleIdentity;
#[path = "tests/formula.rs"]
mod formula;
#[path = "tests/lifetime.rs"]
mod lifetime;
#[path = "tests/mapping.rs"]
mod mapping_tests;
#[path = "tests/oracle.rs"]
mod oracle;
#[path = "tests/propagation.rs"]
mod propagation;
fn rows() -> Rows {
    Arc::from([(
        "/actual/defs.bzl".into(),
        BzlModuleSourceProvenance::new(
            BzlModuleIdentity {
                label: CanonicalLabel::parse("@@repo+//pkg:defs.bzl").unwrap(),
                source: slug_build_api_v2::ModuleSourceOrigin::Workspace("/actual/defs.bzl".into()),
                repository_mapping: Arc::from([
                    (
                        ApparentRepoName::new("first").unwrap(),
                        CanonicalRepoName::new("one+").unwrap(),
                    ),
                    (
                        ApparentRepoName::new("second_long").unwrap(),
                        CanonicalRepoName::new("different_long+").unwrap(),
                    ),
                ]),
            },
            [7; 32],
        ),
    )])
}
fn certify(raw: Rows, work: &mut Budget) -> CertifiedSourceTable {
    match SourceTableCapture::from_raw(raw, work) {
        SourceTableCapture::Complete(table) => table,
        SourceTableCapture::Refused { error, .. } => panic!("{error}"),
    }
}
