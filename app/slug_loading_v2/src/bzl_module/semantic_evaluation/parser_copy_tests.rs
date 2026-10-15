use dice::DetectCycles;
use dice::Dice;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;
const CONTEXT: PeakContext = PeakContext::new(
    PeakOwner::SemanticInputs,
    PeakPhase::LocalRowForecast,
    FirstLedger::Local,
);

fn identity(name: &str) -> BzlModuleIdentity {
    BzlModuleIdentity {
        label: CanonicalLabel::parse(&format!("@@//:{name}.bzl")).unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(
            format!("/parser-copy/{name}.bzl").into(),
        ),
        repository_mapping: Arc::from([]),
    }
}

#[test]
fn parser_source_credit_is_exact_and_preserves_semantic_record_count() {
    let mut credit = budget::Budget::default();
    facts::reserve_parser_source_copy(&mut credit, 23, CONTEXT).unwrap();
    assert_eq!(credit.bytes, 23 + std::mem::size_of::<String>());
    assert_eq!(credit.records, 0);
    let identity = identity("source");
    let empty = facts::row_budget(
        &identity,
        "source.bzl",
        b"",
        &test_environment(),
        std::iter::empty(),
    )
    .unwrap();
    let source = b"value = 1\n";
    let full = facts::row_budget(
        &identity,
        "source.bzl",
        source,
        &test_environment(),
        std::iter::empty(),
    )
    .unwrap();
    // One immutable input source and one parser source, regardless of whether
    // the live parser can physically share the original input allocation.
    assert_eq!(full.bytes - empty.bytes, 2 * source.len());
    assert_eq!(full.records, empty.records);
}

#[tokio::test]
async fn repeated_child_edges_retain_one_complete_parser_source_credit() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = dice.updater().commit().await;
    let child_identity = identity("child");
    let a: Arc<[u8]> = Arc::from(b"value = 1\n".as_slice());
    let b: Arc<[u8]> = Arc::from(b"value = 1\n# extra source spelling\n".as_slice());
    let leaf = |source: &Arc<[u8]>| {
        prepare(
            &child_identity,
            "child.bzl",
            source,
            BzlModuleContext::Build,
            &test_environment(),
            std::iter::empty(),
        )
        .unwrap()
    };
    let child_a = transaction.compute(&leaf(&a)).await.unwrap();
    let child_b = transaction.compute(&leaf(&b)).await.unwrap();
    let parent_identity = identity("parent");
    let parent_source: Arc<[u8]> =
        Arc::from(b"load(':child.bzl', 'value')\nload(':child.bzl', 'value')\n".as_slice());
    let parent = |child: &FrozenBzlModule| {
        prepare(
            &parent_identity,
            "parent.bzl",
            &parent_source,
            BzlModuleContext::Build,
            &test_environment(),
            [(":child.bzl", child), (":child.bzl", child)].into_iter(),
        )
        .unwrap()
    };
    let parent_a = parent(child_a.result().unwrap());
    let parent_b = parent(child_b.result().unwrap());
    assert_eq!((parent_a.0.children.len(), parent_a.0.rows.len()), (2, 2));
    assert_eq!((parent_b.0.children.len(), parent_b.0.rows.len()), (2, 2));
    let extra = b.len() - a.len();
    assert_eq!(
        parent_b.0.rows[1].budget.bytes - parent_a.0.rows[1].budget.bytes,
        2 * extra
    );
    assert_eq!(parent_b.0.budget.bytes - parent_a.0.budget.bytes, 2 * extra);
    assert_eq!(parent_b.0.budget.records, parent_a.0.budget.records);
}

#[test]
fn parser_source_limit_admission_precedes_copy_at_small_remaining_boundary() {
    let source: Arc<[u8]> = Arc::from(b"value = 1\n".as_slice());
    let credit = source.len() + std::mem::size_of::<String>();
    let mut exact = budget::Budget {
        bytes: budget::BYTE_LIMIT - credit,
        ..Default::default()
    };
    facts::reserve_parser_source_copy(&mut exact, source.len(), CONTEXT).unwrap();
    assert_eq!(exact.bytes, budget::BYTE_LIMIT);
    let mut refused = budget::Budget {
        bytes: budget::BYTE_LIMIT - credit + 1,
        ..Default::default()
    };
    let source_guards = Arc::strong_count(&source);
    let mut parser_source = None;
    if facts::reserve_parser_source_copy(&mut refused, source.len(), CONTEXT).is_ok() {
        parser_source = Some(std::str::from_utf8(&source).unwrap().to_owned());
    }
    assert!(parser_source.is_none());
    assert_eq!(Arc::strong_count(&source), source_guards);
    assert_eq!(refused.records, 0);
    assert!(
        facts::reserve_parser_source_copy(&mut budget::Budget::default(), usize::MAX, CONTEXT)
            .is_err()
    );
}
