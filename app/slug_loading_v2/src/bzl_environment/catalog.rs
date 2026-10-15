//! Bounded bundled source producer; integrity checks do not choose implementations.
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use sha2::Digest;
use sha2::Sha256;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::syntax::StringEncoding;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;

const TAG: &str = "bazel-9.2.0-8220c6198837d5c13d53fea211cf3282aa12408a";
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct CatalogEntry {
    pub(super) path: CompactString,
    pub(super) source: Arc<[u8]>,
    pub(super) loads: Arc<[CompactString]>,
}
#[derive(Debug, Allocative)]
pub(crate) struct BuiltinSourceCatalog {
    pub(super) tag: CompactString,
    pub(super) entries: Arc<[CatalogEntry]>,
    pub(super) budget: Budget,
    hash: u64,
}
impl PartialEq for BuiltinSourceCatalog {
    fn eq(&self, o: &Self) -> bool {
        self.tag == o.tag && self.entries == o.entries
    }
}
impl Eq for BuiltinSourceCatalog {}
impl std::hash::Hash for BuiltinSourceCatalog {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.hash.hash(h);
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum BuiltinCatalogSnapshot {
    Bazel9_2,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum BuiltinSourceCatalogKey {
    Bundled(BuiltinCatalogSnapshot),
    #[cfg(test)]
    Snapshot(Arc<BuiltinSourceCatalog>),
}
impl std::fmt::Display for BuiltinSourceCatalogKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("builtin-source-catalog")
    }
}
const SOURCES: &[(&str, &[u8], &str)] = &[
    (
        "common/objc/apple_common.bzl",
        include_bytes!("assets/common/objc/apple_common.bzl"),
        "dcbf8f2cbb1c87e711c44800737b2611a42116fdb9f0acbe25b35af668a75c86",
    ),
    (
        "common/objc/apple_env.bzl",
        include_bytes!("assets/common/objc/apple_env.bzl"),
        "f1b2c27562c47d4220022c633c67c7d756d45c22a57480832bc460ae25986ffe",
    ),
    (
        "common/objc/apple_platform.bzl",
        include_bytes!("assets/common/objc/apple_platform.bzl"),
        "c7ba7537915222ddab538ab3bd0baf23fcdd6357e0ed4baafee15aa2bf5fa5cc",
    ),
    (
        "common/objc/apple_toolchain.bzl",
        include_bytes!("assets/common/objc/apple_toolchain.bzl"),
        "772c5ec41f885d720646fd3472af04d2f95dd59bd7f71f656197fdf565d43ba5",
    ),
    (
        "common/objc/objc_info.bzl",
        include_bytes!("assets/common/objc/objc_info.bzl"),
        "7f645d2c92b8d22011ad0da7b8675638964ff8a460e0498594bc4363d05b4a84",
    ),
    (
        "common/xcode/providers.bzl",
        include_bytes!("assets/common/xcode/providers.bzl"),
        "67c02e7c2ae4793ce74f8e29e10058096439edda4f0aa32585296f4cc38f5827",
    ),
];
fn bundled() -> Result<Arc<BuiltinSourceCatalog>, Arc<str>> {
    prepare(TAG, SOURCES).map_err(Arc::from)
}
pub(super) fn prepare(
    tag: &str,
    entries: &[(&str, &[u8], &str)],
) -> Result<Arc<BuiltinSourceCatalog>, String> {
    let mut budget = Budget::default();
    budget.charge_with_context(
        budget_context(PeakPhase::CatalogPreconstructHash, FirstLedger::Catalog),
        1,
        std::mem::size_of::<BuiltinSourceCatalog>() + 4 * std::mem::size_of::<usize>() + tag.len(),
    )?;
    budget.array_with_context::<CatalogEntry>(
        budget_context(PeakPhase::CatalogPreconstructHash, FirstLedger::Catalog),
        entries.len().saturating_mul(2),
    )?;
    // Borrowed full source/parse peak forecast before owning any catalog source.
    for (path, source, _) in entries {
        if tag.is_empty()
            || tag.contains('\0')
            || path.contains(['\\', '\0'])
            || !path.ends_with(".bzl")
            || path.split('/').any(|p| matches!(p, "" | "." | ".."))
        {
            return Err("invalid builtin catalog path".into());
        }
        budget.charge_with_context(
            budget_context(PeakPhase::CatalogPreconstructHash, FirstLedger::Catalog),
            1,
            path.len()
                .saturating_mul(3)
                .saturating_add(source.len().saturating_mul(3))
                .saturating_add(128),
        )?;
    }
    // Check duplicate catalog paths over borrowed input. Every actual
    // comparison is metered before inspection; no source guard exists yet.
    for (index, (path, _, _)) in entries.iter().enumerate() {
        for (previous, _, _) in &entries[..index] {
            budget.charge_with_context(
                budget_context(PeakPhase::CatalogPreconstructHash, FirstLedger::Catalog),
                1,
                0,
            )?;
            if path == previous {
                return Err("duplicate builtin catalog path".into());
            }
        }
    }
    // One borrowed admission covers the complete later hash writer before any
    // owned entry exists. Every load/decoded character consumes at least one
    // source byte, so bytes also bound all load records (without another scan).
    let mut hash_work = Budget::default();
    hash_work.charge_with_context(
        budget_context(PeakPhase::CatalogPreconstructHash, FirstLedger::Work),
        1,
        tag.len(),
    )?;
    for (path, source, _) in entries {
        hash_work.charge_with_context(
            budget_context(PeakPhase::CatalogPreconstructHash, FirstLedger::Work),
            source.len().checked_add(1).ok_or_else(|| {
                crate::bzl_module::semantic_evaluation::budget::limit_with_context(budget_context(
                    PeakPhase::CatalogPreconstructHash,
                    FirstLedger::Work,
                ))
            })?,
            path.len()
                .checked_add(source.len().saturating_mul(2))
                .ok_or_else(|| {
                    crate::bzl_module::semantic_evaluation::budget::limit_with_context(
                        budget_context(PeakPhase::CatalogPreconstructHash, FirstLedger::Work),
                    )
                })?,
        )?;
    }
    crate::bzl_module::semantic_evaluation::budget::admit_peak(
        PeakContext::new(
            PeakOwner::BuiltinCatalog,
            PeakPhase::CatalogPreconstructHash,
            FirstLedger::Catalog,
        ),
        budget,
        hash_work,
        0,
    )?;
    let mut out = Vec::with_capacity(entries.len());
    for (path, source, expected) in entries {
        let digest = Sha256::digest(source);
        let actual = digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if &actual != expected {
            return Err("bundled builtin source integrity mismatch".into());
        }
        let text = std::str::from_utf8(source).map_err(|_| "builtin source is not UTF-8")?;
        // Independently bounded ephemeral parser storage is admitted jointly
        // with the full retained catalog before the source String/AST exists.
        super::catalog_parse::admit_with_work(
            budget,
            hash_work,
            path.len(),
            source.len(),
            PeakPhase::CatalogBeforeParse,
        )?;
        let ast = AstModule::parse_with_string_encoding(
            path,
            text.to_owned(),
            &Dialect::Bazel,
            StringEncoding::BazelInternal,
        )
        .map_err(|e| e.to_string())?;
        let loads =
            super::catalog_parse::loads(&ast, &mut budget, hash_work, path.len(), source.len())?;
        out.push(CatalogEntry {
            path: (*path).into(),
            source: Arc::from(*source),
            loads,
        });
    }
    budget.charge_with_context(
        budget_context(PeakPhase::CatalogFinalHash, FirstLedger::Catalog),
        1,
        std::mem::size_of::<u64>(),
    )?;
    crate::bzl_module::semantic_evaluation::budget::admit_peak(
        PeakContext::new(
            PeakOwner::BuiltinCatalog,
            PeakPhase::CatalogFinalHash,
            FirstLedger::Catalog,
        ),
        budget,
        hash_work,
        0,
    )?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    tag.hash(&mut hash);
    out.hash(&mut hash);
    Ok(Arc::new(BuiltinSourceCatalog {
        tag: tag.into(),
        entries: out.into(),
        budget,
        hash: std::hash::Hasher::finish(&hash),
    }))
}
#[async_trait]
impl Key for BuiltinSourceCatalogKey {
    type Value = Arc<Result<Arc<BuiltinSourceCatalog>, Arc<str>>>;
    async fn compute(&self, _: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        Arc::new(match self {
            Self::Bundled(BuiltinCatalogSnapshot::Bazel9_2) => bundled(),
            #[cfg(test)]
            Self::Snapshot(value) => Ok(value.clone()),
        })
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
    fn validity(v: &Self::Value) -> bool {
        v.is_ok()
    }
}

impl slug_build_api_v2::published_value::ImmutablePublicationInput for BuiltinSourceCatalog {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn retained_shape(&self) -> slug_build_api_v2::published_value::PublicationShape {
        super::publication_shape(self.budget)
    }
}

const fn budget_context(phase: PeakPhase, ledger: FirstLedger) -> PeakContext {
    PeakContext::new(PeakOwner::BuiltinCatalog, phase, ledger)
}
