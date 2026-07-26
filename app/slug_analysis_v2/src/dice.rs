/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use futures::FutureExt;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_loading_v2::PackageTargetKind;
use slug_loading_v2::keys::PackageLoadKey;
use starlark::PrintHandler;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::key::ConfiguredTargetKey;
use crate::result::AnalysisResult;
use crate::starlark_rule::PreparedDependency;
use crate::starlark_rule::evaluate_loaded_rule;

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct AnalysisError {
    message: String,
}

impl AnalysisError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AnalysisError {}

/// The single production DICE identity for configured-target analysis.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct ConfiguredTargetAnalysisKey {
    pub workspace: PathBuf,
    pub configured_target: ConfiguredTargetKey,
}

impl fmt::Display for ConfiguredTargetAnalysisKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configured-target-analysis:{}", self.configured_target)
    }
}

type AnalysisKeyValue = Arc<Result<AnalysisResult, AnalysisError>>;

#[derive(Default)]
struct AnalysisPrintCapture {
    events: RefCell<Vec<EvaluationEvent>>,
}

impl AnalysisPrintCapture {
    fn into_batch(self) -> EventBatch {
        EventBatch::from_events(self.events.into_inner())
    }
}

impl PrintHandler for AnalysisPrintCapture {
    fn println(&self, text: &str) -> starlark::Result<()> {
        self.events
            .borrow_mut()
            .push(EvaluationEvent::StarlarkPrint { text: text.into() });
        Ok(())
    }
}

#[async_trait]
impl Key for ConfiguredTargetAnalysisKey {
    type Value = AnalysisKeyValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = Arc::new(
            self.compute_inner(ctx, capture_events, &mut event_batch)
                .await,
        );
        if capture_events {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("ConfiguredTargetAnalysisKey stores exactly one local event batch");
        }
        value
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!((x.as_ref(), y.as_ref()), (Ok(x), Ok(y)) if x == y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

impl ConfiguredTargetAnalysisKey {
    async fn compute_inner(
        &self,
        ctx: &mut DiceComputations<'_>,
        capture_events: bool,
        event_batch: &mut Option<EventBatch>,
    ) -> Result<AnalysisResult, AnalysisError> {
        let label = self.configured_target.label();
        if !label.package().repo().is_root() {
            return Err(AnalysisError::new(format!(
                "external repository configured targets are not supported: {label}"
            )));
        }
        let package_path = self.workspace.join(label.package().package().as_str());
        let package = ctx
            .compute(&PackageLoadKey {
                workspace: self.workspace.clone(),
                package: package_path,
            })
            .await
            .map_err(|error| {
                AnalysisError::new(format!("loading package through DICE: {error}"))
            })?;
        let package = package
            .as_ref()
            .as_ref()
            .map_err(|error| AnalysisError::new(error.to_string()))?;
        let target = package
            .targets
            .iter()
            .find(|target| target.name == label.target().as_str())
            .ok_or_else(|| {
                AnalysisError::new(format!(
                    "target `{label}` was not found in {}",
                    package.build_file.display()
                ))
            })?;
        let PackageTargetKind::StarlarkRule(implementation) = &target.kind else {
            return Err(AnalysisError::new(format!(
                "target `{label}` is not a Starlark rule"
            )));
        };

        let declared_dependency_keys = implementation
            .dependencies()
            .iter()
            .cloned()
            .map(|label| {
                ConfiguredTargetKey::new(label, self.configured_target.configuration().clone())
            })
            .collect::<Vec<_>>();
        let mut unique = SmallSet::with_capacity(declared_dependency_keys.len());
        for dependency in &declared_dependency_keys {
            unique.insert(dependency.clone());
        }
        let workspace = &self.workspace;
        let computed = ctx
            .try_compute_join(unique.into_iter(), |ctx, configured_target| {
                async move {
                    let value = ctx
                        .compute(&ConfiguredTargetAnalysisKey {
                            workspace: workspace.clone(),
                            configured_target: configured_target.clone(),
                        })
                        .await
                        .map_err(|error| {
                            AnalysisError::new(format!(
                                "computing dependency `{configured_target}` through DICE: {error}"
                            ))
                        })?;
                    let result = value.as_ref().as_ref().map_err(Clone::clone)?.clone();
                    Ok((configured_target, result))
                }
                .boxed()
            })
            .await?;
        let computed = computed
            .into_iter()
            .collect::<SmallMap<ConfiguredTargetKey, AnalysisResult>>();
        let dependencies = declared_dependency_keys
            .iter()
            .map(|key| {
                let result = computed.get(key).ok_or_else(|| {
                    AnalysisError::new(format!(
                        "internal error: dependency result missing for `{key}`"
                    ))
                })?;
                Ok(PreparedDependency {
                    key: key.clone(),
                    providers: result.providers().clone(),
                })
            })
            .collect::<Result<Vec<_>, AnalysisError>>()?;
        let print_capture = capture_events.then(AnalysisPrintCapture::default);
        let value = evaluate_loaded_rule(
            package,
            label.target().as_str(),
            self.configured_target.clone(),
            label.package().package().as_str(),
            dependencies,
            print_capture
                .as_ref()
                .map(|capture| capture as &dyn PrintHandler),
        );
        *event_batch = print_capture.map(AnalysisPrintCapture::into_batch);
        value.map_err(AnalysisError::new)
    }
}
