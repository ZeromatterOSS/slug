use std::fmt;

use allocative::Allocative;
use async_trait::async_trait;
use dice::Demand;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_workspace_v2::BudgetedImageBlock;

use super::SelectedScratchElfMetadataRequest;
use super::SelectedScratchElfMetadataResult;
use super::SelectedScratchElfMetadataSelector;
use super::SelectedToolOperand;
use super::SelectedToolSourceError;
use crate::SelectedScratchPrefix;
use crate::SelectedScratchResources;
use crate::SourcePreparationNeeds;
use crate::SourcePreparationOutcome;

#[derive(Debug, Clone, Dupe, Eq, PartialEq, Allocative)]
pub enum SelectedScratchElfMetadataEpoch {
    Unauthorized,
    AuthorizedEmpty,
    Ready(SelectedScratchElfMetadataResult),
}

/// Only this compact selector enters the engine-lifetime key/demand catalogue.
/// The full prefix/member/blob are observation VALUES, never key fields.
#[derive(Debug, Clone, Dupe, Eq, PartialEq, Hash, Allocative)]
pub struct SelectedScratchElfMetadataEpochKey {
    selector: SelectedScratchElfMetadataSelector,
    operand: SelectedToolOperand,
}
impl SelectedScratchElfMetadataEpochKey {
    pub fn new(request: &SelectedScratchElfMetadataRequest) -> Self {
        Self {
            selector: request.selector().dupe(),
            operand: request.operand(),
        }
    }
    pub fn selector(&self) -> &SelectedScratchElfMetadataSelector {
        &self.selector
    }
    pub fn operand(&self) -> SelectedToolOperand {
        self.operand
    }
}
impl fmt::Display for SelectedScratchElfMetadataEpochKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "selected-scratch-elf:{:?}:{}",
            self.operand.role, self.operand.argument_index
        )
    }
}
// The public InjectedKey blanket cannot provide demand metadata. This raw input
// uses existing normal-Key changed_to ownership, like other raw observations.
// A missing/recomputed cell can only deny authorization, never invent Ready.
#[async_trait]
impl Key for SelectedScratchElfMetadataEpochKey {
    type Value = SelectedScratchElfMetadataEpoch;
    async fn compute(&self, _: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        SelectedScratchElfMetadataEpoch::Unauthorized
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
    fn provide<'a>(&'a self, demand: &mut Demand<'a>) {
        demand.provide_value_with(|| self.dupe());
    }
}

#[derive(Eq, PartialEq, Allocative)]
struct NeedsInner {
    rows: [Option<SelectedScratchElfMetadataRequest>; 3],
    len: usize,
}
#[derive(Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct SelectedScratchElfMetadataNeeds(BudgetedImageBlock<NeedsInner>);
impl SelectedScratchElfMetadataNeeds {
    pub fn singleton(
        request: &SelectedScratchElfMetadataRequest,
        resources: &SelectedScratchResources,
    ) -> Result<Self, SelectedToolSourceError> {
        Self::new(std::slice::from_ref(request), resources)
    }
    pub fn new(
        requests: &[SelectedScratchElfMetadataRequest],
        resources: &SelectedScratchResources,
    ) -> Result<Self, SelectedToolSourceError> {
        if requests.is_empty() {
            return Err(SelectedToolSourceError::InvalidRequest);
        }
        if requests.len() > 3 {
            return Err(SelectedToolSourceError::TooManyRequests);
        }
        let mut inner = NeedsInner {
            rows: std::array::from_fn(|_| None),
            len: 0,
        };
        for request in requests {
            insert(&mut inner, request, resources)?;
        }
        Self::freeze(inner, resources)
    }
    fn freeze(
        mut inner: NeedsInner,
        resources: &SelectedScratchResources,
    ) -> Result<Self, SelectedToolSourceError> {
        let policy = inner.rows[0]
            .as_ref()
            .expect("nonempty batch")
            .selector()
            .policy();
        inner.rows[..inner.len].sort_by_key(|row| {
            policy
                .operands()
                .iter()
                .position(|p| *p == row.as_ref().unwrap().operand())
                .unwrap()
        });
        Ok(Self(
            resources
                .descriptor_budget()
                .try_reserve(0)?
                .freeze(inner)?,
        ))
    }
    pub fn selector(&self) -> &SelectedScratchElfMetadataSelector {
        self.0.get().rows[0].as_ref().unwrap().selector()
    }
    pub fn prefix(&self) -> &SelectedScratchPrefix {
        self.0.get().rows[0].as_ref().unwrap().prefix()
    }
    pub fn requests(&self) -> impl ExactSizeIterator<Item = &SelectedScratchElfMetadataRequest> {
        self.0.get().rows[..self.len()]
            .iter()
            .map(|row| row.as_ref().unwrap())
    }
    pub fn len(&self) -> usize {
        self.0.get().len
    }
    pub fn try_union(&self, other: &Self) -> Result<Self, SelectedToolSourceError> {
        let resources = self.selector().resources();
        if !self.same_resources(resources) || !other.same_resources(resources) {
            return Err(SelectedToolSourceError::ForeignResources);
        }
        let mut inner = NeedsInner {
            rows: std::array::from_fn(|_| None),
            len: 0,
        };
        for request in self.requests().chain(other.requests()) {
            insert(&mut inner, request, resources)?;
        }
        // Avoid an allocation for equal or subset batches, including under pressure.
        if inner.len == self.len()
            && self.requests().all(|r| {
                inner.rows[..inner.len]
                    .iter()
                    .any(|row| row.as_ref() == Some(r))
            })
        {
            return Ok(self.dupe());
        }
        if inner.len == other.len()
            && other.requests().all(|r| {
                inner.rows[..inner.len]
                    .iter()
                    .any(|row| row.as_ref() == Some(r))
            })
        {
            return Ok(other.dupe());
        }
        Self::freeze(inner, resources)
    }
    pub fn same_resources(&self, resources: &SelectedScratchResources) -> bool {
        self.0.same_budget(resources.descriptor_budget())
            && self.requests().all(|r| r.same_resources(resources))
    }
}
fn insert(
    inner: &mut NeedsInner,
    request: &SelectedScratchElfMetadataRequest,
    resources: &SelectedScratchResources,
) -> Result<(), SelectedToolSourceError> {
    if !request.same_resources(resources) {
        return Err(SelectedToolSourceError::ForeignResources);
    }
    if let Some(first) = &inner.rows[0] {
        if first.selector() != request.selector() || first.prefix() != request.prefix() {
            return Err(SelectedToolSourceError::ConflictingCheckpoint);
        }
    }
    for row in &inner.rows[..inner.len] {
        let row = row.as_ref().unwrap();
        if row.operand() == request.operand() {
            return if row == request {
                Ok(())
            } else {
                Err(SelectedToolSourceError::ConflictingRequest)
            };
        }
    }
    if inner.len == 3 {
        return Err(SelectedToolSourceError::TooManyRequests);
    }
    inner.rows[inner.len] = Some(request.dupe());
    inner.len += 1;
    Ok(())
}
impl fmt::Debug for SelectedScratchElfMetadataNeeds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedScratchElfMetadataNeeds")
            .field("len", &self.len())
            .finish()
    }
}

#[derive(Allocative)]
struct TransactionInner {
    selector: SelectedScratchElfMetadataSelector,
    allowed: Option<SelectedScratchElfMetadataNeeds>,
    ready: [Option<SelectedScratchElfMetadataResult>; 3],
    len: usize,
}
impl PartialEq for TransactionInner {
    fn eq(&self, other: &Self) -> bool {
        self.selector == other.selector
            && self.allowed == other.allowed
            && self.ready == other.ready
            && self.len == other.len
    }
}
impl Eq for TransactionInner {}
#[derive(Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct SelectedScratchElfMetadataTransaction(BudgetedImageBlock<TransactionInner>);
impl SelectedScratchElfMetadataTransaction {
    pub fn new(
        selector: &SelectedScratchElfMetadataSelector,
        resources: &SelectedScratchResources,
        allowed: Option<&SelectedScratchElfMetadataNeeds>,
        ready: &[SelectedScratchElfMetadataResult],
    ) -> Result<Self, SelectedToolSourceError> {
        if !selector.same_resources(resources) {
            return Err(SelectedToolSourceError::ForeignResources);
        }
        if ready.len() > 3 {
            return Err(SelectedToolSourceError::TooManyRequests);
        }
        if let Some(allowed) = allowed {
            if !allowed.same_resources(resources) {
                return Err(SelectedToolSourceError::ForeignResources);
            }
            if allowed.selector() != selector {
                return Err(SelectedToolSourceError::UnauthorizedRequest);
            }
        }
        let mut rows: [Option<SelectedScratchElfMetadataResult>; 3] = std::array::from_fn(|_| None);
        for (i, result) in ready.iter().enumerate() {
            if !result.same_resources(resources) {
                return Err(SelectedToolSourceError::ForeignResources);
            }
            if !allowed.is_some_and(|batch| batch.requests().any(|r| r == result.request())) {
                return Err(SelectedToolSourceError::UnauthorizedRequest);
            }
            if ready[..i]
                .iter()
                .any(|r| r.request().operand() == result.request().operand())
            {
                return Err(SelectedToolSourceError::ConflictingRequest);
            }
            rows[i] = Some(result.dupe());
        }
        rows[..ready.len()].sort_by_key(|row| {
            selector
                .policy()
                .operands()
                .iter()
                .position(|p| *p == row.as_ref().unwrap().request().operand())
                .unwrap()
        });
        Ok(Self(resources.descriptor_budget().try_reserve(0)?.freeze(
            TransactionInner {
                selector: selector.dupe(),
                allowed: allowed.map(Dupe::dupe),
                ready: rows,
                len: ready.len(),
            },
        )?))
    }
    pub fn selector(&self) -> &SelectedScratchElfMetadataSelector {
        &self.0.get().selector
    }
    pub fn allowed(&self) -> Option<&SelectedScratchElfMetadataNeeds> {
        self.0.get().allowed.as_ref()
    }
    pub fn authorizes_selector(&self, selector: &SelectedScratchElfMetadataSelector) -> bool {
        self.selector() == selector && self.same_resources(selector.resources())
    }
    pub fn is_authorized(&self, request: &SelectedScratchElfMetadataRequest) -> bool {
        self.authorizes_selector(request.selector())
            && request.same_resources(self.selector().resources())
            && self
                .allowed()
                .is_some_and(|batch| batch.requests().any(|r| r == request))
    }
    pub fn get(
        &self,
        request: &SelectedScratchElfMetadataRequest,
    ) -> Option<&SelectedScratchElfMetadataResult> {
        if !self.is_authorized(request) {
            return None;
        }
        self.0.get().ready[..self.0.get().len]
            .iter()
            .filter_map(Option::as_ref)
            .find(|result| result.request() == request)
    }
    pub fn epoch(
        &self,
        key: &SelectedScratchElfMetadataEpochKey,
    ) -> SelectedScratchElfMetadataEpoch {
        if !self.authorizes_selector(key.selector()) {
            return SelectedScratchElfMetadataEpoch::Unauthorized;
        }
        let Some(request) = self
            .allowed()
            .and_then(|batch| batch.requests().find(|r| r.operand() == key.operand()))
        else {
            return SelectedScratchElfMetadataEpoch::Unauthorized;
        };
        self.get(request)
            .map_or(SelectedScratchElfMetadataEpoch::AuthorizedEmpty, |result| {
                SelectedScratchElfMetadataEpoch::Ready(result.dupe())
            })
    }
    pub fn same_resources(&self, resources: &SelectedScratchResources) -> bool {
        self.0.same_budget(resources.descriptor_budget())
            && self.selector().same_resources(resources)
            && self
                .allowed()
                .is_none_or(|allowed| allowed.same_resources(resources))
            && self.0.get().ready[..self.0.get().len]
                .iter()
                .flatten()
                .all(|r| r.same_resources(resources))
    }
}
impl fmt::Debug for SelectedScratchElfMetadataTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedScratchElfMetadataTransaction")
            .field("ready", &self.0.get().len)
            .finish()
    }
}

/// Called INSIDE Loading's normal SourcesKey, whose lower normal dependency owns
/// full prefix identity. No blob-bearing key or demand marker is interned here.
pub async fn lookup_selected_scratch_elf_metadata(
    ctx: &mut DiceComputations<'_>,
    request: &SelectedScratchElfMetadataRequest,
) -> anyhow::Result<
    SourcePreparationOutcome<Result<SelectedScratchElfMetadataResult, SelectedToolSourceError>>,
> {
    let complete_error = |error| Ok(SourcePreparationOutcome::Complete(Err(error)));
    let Ok(resources) = ctx
        .per_transaction_data()
        .data
        .get::<SelectedScratchResources>()
    else {
        return complete_error(SelectedToolSourceError::MissingResources);
    };
    let resources = resources.dupe();
    if !request.same_resources(&resources) {
        return complete_error(SelectedToolSourceError::ForeignResources);
    }
    let Ok(transaction) = ctx
        .per_transaction_data()
        .data
        .get::<SelectedScratchElfMetadataTransaction>()
    else {
        return complete_error(SelectedToolSourceError::MissingTransaction);
    };
    let transaction = transaction.dupe();
    if !transaction.same_resources(&resources) {
        return complete_error(SelectedToolSourceError::ForeignResources);
    }
    if !transaction.is_authorized(request) {
        return complete_error(SelectedToolSourceError::UnauthorizedRequest);
    }
    match ctx
        .compute(&SelectedScratchElfMetadataEpochKey::new(request))
        .await?
    {
        SelectedScratchElfMetadataEpoch::Unauthorized => {
            complete_error(SelectedToolSourceError::UnauthorizedRequest)
        }
        SelectedScratchElfMetadataEpoch::Ready(result)
            if result.request() == request
                && result.same_resources(&resources)
                && transaction.get(request) == Some(&result) =>
        {
            Ok(SourcePreparationOutcome::Complete(Ok(result)))
        }
        SelectedScratchElfMetadataEpoch::AuthorizedEmpty
        | SelectedScratchElfMetadataEpoch::Ready(_) => {
            match SelectedScratchElfMetadataNeeds::singleton(request, &resources) {
                Ok(batch) => Ok(SourcePreparationOutcome::Need(
                    SourcePreparationNeeds::scratch_elf_metadata(batch),
                )),
                Err(error) => complete_error(error),
            }
        }
    }
}
