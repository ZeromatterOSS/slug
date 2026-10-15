//! Both actual registration source modes, never declaration expansion.
use std::mem::size_of;

use dice::DiceTransaction;
use slug_bzlmod_v2::HostRepositorySourceInputDispositionView;
use slug_bzlmod_v2::HostSelectedRegistrationPatterns;
use slug_bzlmod_v2::HostSelectedRegistrationPatternsKey;
use slug_bzlmod_v2::HostSelectedRegistrationPatternsObservationKey;
use slug_bzlmod_v2::ObservedHostSelectedRegistrationPatterns;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_bzlmod_v2::source_preparation::compute_repository_materialization_result;
use slug_identity_v2::CanonicalTargetPattern;
use slug_loading_v2::HostCanonicalRepositoryLoadRouteKey;
use slug_loading_v2::HostCanonicalRepositoryLoadRouteObservationKey;
use slug_loading_v2::ObservedHostCanonicalRepositoryLoadRoute;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::Budget;
use super::request;

#[derive(Clone, Copy)]
enum Mode {
    Legacy,
    Observed,
}

pub(super) async fn repository_need(
    transaction: &mut DiceTransaction,
    root: &NormalizedAbsolutePath,
    budget: &mut Budget,
) -> Result<Option<SourcePreparationNeeds>, String> {
    // The same caller ledger spans both modes and all materialization rounds.
    for mode in [Mode::Legacy, Mode::Observed] {
        let bytes = root
            .as_path()
            .as_os_str()
            .len()
            .checked_add(
                size_of::<HostSelectedRegistrationPatternsKey>()
                    .max(size_of::<HostSelectedRegistrationPatternsObservationKey>()),
            )
            .and_then(|n| n.checked_add(size_of::<ObservedHostSelectedRegistrationPatterns>()))
            .ok_or("fixture pattern key/carrier storage overflow")?;
        budget.charge(1, bytes)?;
        let need = match mode {
            Mode::Legacy => {
                let selected = transaction
                    .compute(&HostSelectedRegistrationPatternsKey::new(root.clone()))
                    .await
                    .map_err(|e| e.to_string())?;
                match selected {
                    SourcePreparationOutcome::Need(need) => return Ok(Some(need)),
                    SourcePreparationOutcome::Complete(selected) => match selected.as_ref() {
                        Ok(selected) => {
                            pattern_routes(transaction, root, selected, mode, budget).await?
                        }
                        Err(_) => None, // Original consumer retains its real semantic terminal.
                    },
                }
            }
            Mode::Observed => {
                let selected = transaction
                    .compute(&HostSelectedRegistrationPatternsObservationKey::new(
                        root.clone(),
                    ))
                    .await
                    .map_err(|e| e.to_string())?;
                match selected {
                    SourcePreparationOutcome::Need(need) => return Ok(Some(need)),
                    SourcePreparationOutcome::Complete(Err(_)) => None,
                    SourcePreparationOutcome::Complete(Ok(observed)) => {
                        // Keep the complete actual observed carrier/epoch alive
                        // across the borrowed result walk, without flattening it.
                        match observed.result().as_ref() {
                            Ok(selected) => {
                                pattern_routes(transaction, root, selected, mode, budget).await?
                            }
                            Err(_) => None,
                        }
                    }
                }
            }
        };
        if need.is_some() {
            return Ok(need);
        }
    }
    Ok(None)
}

async fn pattern_routes(
    transaction: &mut DiceTransaction,
    root: &NormalizedAbsolutePath,
    selected: &HostSelectedRegistrationPatterns,
    mode: Mode,
    budget: &mut Budget,
) -> Result<Option<SourcePreparationNeeds>, String> {
    for view in selected.toolchains().chain(selected.execution_platforms()) {
        budget.charge(1, 0)?;
        let mut largest_repo = view.canonical_repo().as_str().len();
        for (_, repo) in view.mapping() {
            budget.charge(1, 0)?;
            largest_repo = largest_repo.max(repo.as_str().len());
        }
        let strings = view
            .raw_pattern()
            .len()
            .checked_add(largest_repo)
            .and_then(|n| n.checked_add(root.as_path().as_os_str().len()))
            .and_then(|n| n.checked_mul(8));
        let headers = size_of::<CanonicalTargetPattern>()
            .checked_add(
                size_of::<HostCanonicalRepositoryLoadRouteKey>()
                    .max(size_of::<HostCanonicalRepositoryLoadRouteObservationKey>()),
            )
            .and_then(|n| n.checked_add(size_of::<slug_identity_v2::CanonicalRepoName>()))
            .and_then(|n| n.checked_add(size_of::<String>()))
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| n.checked_add(size_of::<ObservedHostCanonicalRepositoryLoadRoute>()));
        let bytes = strings
            .and_then(|n| n.checked_add(headers?))
            .ok_or("fixture pattern/route storage overflow")?;
        budget.charge(view.raw_pattern().len(), bytes)?;
        let Ok(pattern) =
            CanonicalTargetPattern::parse(view.raw_pattern(), view.canonical_repo(), |name| {
                view.mapping_target(name)
            })
        else {
            continue;
        };
        let repo = match &pattern {
            CanonicalTargetPattern::Single(label) => label.package().repo(),
            CanonicalTargetPattern::PackageWildcard { package, .. }
            | CanonicalTargetPattern::Recursive { package, .. } => package.repo(),
        };
        if repo.is_root() {
            continue;
        }
        budget.charge(1, 0)?;
        let need = match mode {
            Mode::Legacy => {
                let outcome = transaction
                    .compute(&HostCanonicalRepositoryLoadRouteKey::new(
                        root.clone(),
                        repo.clone(),
                    ))
                    .await
                    .map_err(|e| e.to_string())?;
                match outcome {
                    SourcePreparationOutcome::Need(need) => return Ok(Some(need)),
                    SourcePreparationOutcome::Complete(result) => match result.as_ref() {
                        Ok(route) => {
                            materialization_need(transaction, route.input(), budget).await?
                        }
                        Err(_) => None,
                    },
                }
            }
            Mode::Observed => {
                let outcome = transaction
                    .compute(&HostCanonicalRepositoryLoadRouteObservationKey::new(
                        root.clone(),
                        repo.clone(),
                    ))
                    .await
                    .map_err(|e| e.to_string())?;
                match outcome {
                    SourcePreparationOutcome::Need(need) => return Ok(Some(need)),
                    SourcePreparationOutcome::Complete(Err(_)) => None,
                    SourcePreparationOutcome::Complete(Ok(observed)) => {
                        // Its actual full observation epoch remains with this
                        // carrier while the input/result owner is borrowed.
                        match observed.result().as_ref() {
                            Ok(route) => {
                                materialization_need(transaction, route.input(), budget).await?
                            }
                            Err(_) => None,
                        }
                    }
                }
            }
        };
        if need.is_some() {
            return Ok(need);
        }
    }
    Ok(None)
}

async fn materialization_need(
    transaction: &mut DiceTransaction,
    input: &slug_bzlmod_v2::HostCanonicalRepositorySourceInput,
    budget: &mut Budget,
) -> Result<Option<SourcePreparationNeeds>, String> {
    if let HostRepositorySourceInputDispositionView::Request(request) = input.view().disposition() {
        if request::admit(request, budget)? {
            // Existing complete prepayment precedes the actual request clone
            // and real owner. No mode-specific epoch shortcut or fake Need.
            let result = compute_repository_materialization_result(transaction, request.clone())
                .await
                .map_err(|e| e.to_string())?;
            if let SourcePreparationOutcome::Need(need) = result {
                return Ok(Some(need));
            }
        }
    }
    Ok(None)
}
