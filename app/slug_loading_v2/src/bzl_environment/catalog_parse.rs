//! Pre-parse peak and allocation-free load discovery for the pinned parser.
use std::mem::size_of;

use starlark::syntax::AstModule;
use starlark::syntax::ast::*;

use super::*;
use crate::bzl_module::semantic_evaluation::budget::peak_context::FirstLedger;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakContext;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakOwner;
use crate::bzl_module::semantic_evaluation::budget::peak_context::PeakPhase;

pub(super) fn admit(retained: Budget, path: usize, source: usize) -> Result<(), String> {
    admit_with_work(
        retained,
        Budget::default(),
        path,
        source,
        PeakPhase::CatalogBeforeParse,
    )
}
pub(super) fn admit_with_work(
    retained: Budget,
    work: Budget,
    path: usize,
    source: usize,
    phase: PeakPhase,
) -> Result<(), String> {
    // grammar.lalrpop associates every stored AST/collection record with a
    // consumed terminal; at most two records per byte including statement
    // wrappers. Lexer real tokens consume a byte; each INDENT is backed by
    // indentation whitespace and its one later DEDENT. Total terminals are
    // at most two per byte plus EOF. The generated parser's largest Symbol alternatives are pairs of
    // AstExpr or (ForClause, Vec<Clause>), bounded by two of these max records.
    // 24 max-record slots per byte cover AST records (2n*4 old/new)
    // and parser symbols (2n*2 records*4 old/new). Added span/state words
    // cover parser location/state arrays; lexer indentation/buffer,
    // lint and CodeMap line arrays. The additive text credit covers decoded
    // strings (including implicit load-name copies), source and bigint bytes.
    let record = [
        size_of::<AstStmt>(),
        size_of::<AstExpr>(),
        size_of::<AstTypeExpr>(),
        size_of::<AstArgument>(),
        size_of::<AstParameter>(),
        size_of::<AstAssignTarget>(),
        size_of::<ForClause>(),
        size_of::<Clause>(),
        size_of::<AstString>(),
        size_of::<AstFString>(),
        size_of::<AstInt>(),
        size_of::<AstFloat>(),
        size_of::<AstIdent>(),
        size_of::<AstAssignIdent>(),
        size_of::<Load>(),
        size_of::<LoadArgP<AstNoPayload>>(),
    ]
    .into_iter()
    .max()
    .unwrap()
    .checked_add(4 * size_of::<usize>())
    .ok_or_else(|| semantic_evaluation_limit(phase))?;
    let bytes = source
        .checked_add(1)
        .and_then(|n| n.checked_mul(24))
        .and_then(|n| n.checked_mul(record))
        .and_then(|n| n.checked_add(source.checked_mul(8)?))
        .and_then(|n| n.checked_add(path.checked_mul(4)?))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| semantic_evaluation_limit(phase))?;
    let mut scratch = Budget::default();
    scratch.charge_with_context(
        budget_context(phase, FirstLedger::Work),
        work.records,
        work.bytes,
    )?;
    scratch.charge_with_context(budget_context(phase, FirstLedger::Work), 1, bytes)?;
    crate::bzl_module::semantic_evaluation::budget::admit_peak(
        PeakContext::new(PeakOwner::CatalogParser, phase, FirstLedger::Catalog),
        retained,
        scratch,
        0,
    )
}
fn semantic_evaluation_limit(phase: PeakPhase) -> String {
    crate::bzl_module::semantic_evaluation::budget::limit_with_context(budget_context(
        phase,
        FirstLedger::Work,
    ))
}
fn visit<'a>(
    stmt: &'a AstStmt,
    depth: usize,
    budget: &mut Budget,
    f: &mut impl FnMut(&'a str, &mut Budget) -> Result<(), String>,
) -> Result<(), String> {
    budget.charge_with_context(
        budget_context(PeakPhase::CatalogBeforeLoadCopies, FirstLedger::Catalog),
        1,
        0,
    )?;
    budget.depth_with_context(
        budget_context(PeakPhase::CatalogBeforeLoadCopies, FirstLedger::Catalog),
        depth,
    )?;
    match &stmt.node {
        Stmt::Load(load) => f(&load.module.node, budget),
        Stmt::Statements(statements) => {
            for statement in statements {
                visit(statement, depth + 1, budget, f)?;
            }
            Ok(())
        }
        _ => Ok(()), // Same top-level-only semantics as AstModule::loads.
    }
}
pub(super) fn loads(
    ast: &AstModule,
    budget: &mut Budget,
    work: Budget,
    path: usize,
    source: usize,
) -> Result<Arc<[CompactString]>, String> {
    let mut count = 0usize;
    visit(ast.statement(), 0, budget, &mut |raw, budget| {
        count = count.checked_add(1).ok_or_else(|| {
            crate::bzl_module::semantic_evaluation::budget::limit_with_context(budget_context(
                PeakPhase::CatalogBeforeLoadCopies,
                FirstLedger::Catalog,
            ))
        })?;
        budget.charge_with_context(
            budget_context(PeakPhase::CatalogBeforeLoadCopies, FirstLedger::Catalog),
            1,
            raw.len().saturating_add(size_of::<CompactString>()),
        )
    })?;
    budget.array_with_context::<CompactString>(
        budget_context(PeakPhase::CatalogBeforeLoadCopies, FirstLedger::Catalog),
        count.saturating_mul(3),
    )?;
    // The AST is still live. Re-admit its complete peak together with the
    // newly forecast output before any load Vec/string/Arc allocation.
    admit_with_work(
        *budget,
        work,
        path,
        source,
        PeakPhase::CatalogBeforeLoadCopies,
    )?;
    let mut loads = Vec::with_capacity(count);
    visit(ast.statement(), 0, budget, &mut |raw, _| {
        loads.push(CompactString::from(raw));
        Ok(())
    })?;
    Ok(loads.into())
}

const fn budget_context(phase: PeakPhase, ledger: FirstLedger) -> PeakContext {
    PeakContext::new(PeakOwner::CatalogParser, phase, ledger)
}
