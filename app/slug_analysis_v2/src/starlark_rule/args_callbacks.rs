//! Evaluator-end Args publication. Registration snapshots are private phase state;
//! only complete graphs and payload-owned original executables leave this module.
use std::sync::Arc;

use compact_str::CompactString;
use slug_build_api_v2::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;
use slug_build_api_v2::*;
use slug_loading_v2::subrule_invocation::EvaluatorArgCallGen;
use slug_loading_v2::subrule_invocation::EvaluatorArgsSnapshot;
use slug_loading_v2::subrule_invocation::EvaluatorVectorArgGen;
use slug_loading_v2::subrule_invocation::EvaluatorVectorMapEachGen;
use slug_loading_v2::subrule_invocation::EvaluatorVectorSourceGen;
use starlark::environment::Module;
use starlark::values::OwnedFrozenValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::tuple::TupleRef;

use crate::analysis_value::AnalysisValueLowerer;

mod directory;
mod graph;
mod native;
pub(super) mod pending;
mod runtime;
mod source;
use slug_build_api_v2::published_value::PublishedNodeId;
use slug_build_api_v2::published_value::PublishedValueArena;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;

use crate::analysis_value::published::StarlarkPublishedOriginals;
use crate::analysis_value::published::project::Projector;

#[cfg(test)]
mod bound_method_tests;

#[derive(Clone)]
pub(super) struct DraftRecipe {
    calls: Vec<DraftCall>,
    format: RetainedParamFileFormat,
    policy: Option<RetainedSpawnParamFilePolicy>,
}
#[derive(Clone)]
enum DraftCall {
    Ready(RetainedArgCall),
    All(DraftVector),
    Joined(DraftVector),
}
#[derive(Clone)]
struct DraftVector {
    inputs: DraftInputs,
    node: PublishedNodeId,
    options: RetainedVectorOptions,
    location: Option<CompactString>,
}

#[derive(Clone)]
enum DraftInputs {
    Sequence(Arc<[PublishedNodeId]>),
    Depset(PublishedNodeId),
}

pub(super) struct Publisher<'v> {
    module: &'v Module,
    graph: graph::PublicationBuilder<'v>,
}
impl<'v> Publisher<'v> {
    pub(super) fn new(module: &'v Module) -> Self {
        Self {
            module,
            graph: graph::PublicationBuilder::new(module),
        }
    }

    /// Admit the exact live registration collector before ANY provider capture.
    pub(super) fn with_resource_sets(module: &'v Module) -> Result<Self, String> {
        let mut publisher = Self::new(module);
        slug_loading_v2::resource_set::transfer_resource_sets(module, |visits, bytes| {
            // Includes phase Box fees still live beyond arena seal. Retain the
            // whole conservative forecast rather than subtracting a portion.
            publisher.graph.resource_storage(visits, bytes, false)
        })?;
        Ok(publisher)
    }
    pub(super) fn resource_storage(
        &mut self,
        visits: usize,
        bytes: usize,
        temporary: bool,
    ) -> anyhow::Result<()> {
        self.graph
            .resource_storage(visits, bytes, temporary)
            .map_err(anyhow::Error::msg)
    }
    pub(super) fn resource_set_value(
        &mut self,
        token: &slug_loading_v2::resource_set::ResourceSetRegistration,
    ) -> anyhow::Result<Value<'v>> {
        slug_loading_v2::resource_set::resource_set_value(self.module, token, |visits, bytes| {
            self.graph.resource_storage(visits, bytes, true)
        })
    }
    pub(super) fn capture_resource_callable(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
    ) -> anyhow::Result<PublishedNodeId> {
        self.graph
            .add_callable(value, lowerer)
            .map_err(anyhow::Error::msg)
    }
    pub(super) fn capture(
        &mut self,
        value: Value<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
    ) -> Result<PublishedNodeId, String> {
        self.graph.add_value(value, lowerer)
    }
    pub(super) fn prepare(
        &mut self,
        snapshot: EvaluatorArgsSnapshot<'v>,
        lowerer: &mut AnalysisValueLowerer<'v>,
    ) -> anyhow::Result<DraftRecipe> {
        anyhow::ensure!(
            snapshot.calls.len() <= CALLABLE_GRAPH_NODE_LIMIT,
            "Args call count limit exceeded"
        );
        let mut calls = Vec::with_capacity(snapshot.calls.len());
        for call in snapshot.calls {
            calls.push(match call {
                EvaluatorArgCallGen::Scalar(value) => {
                    DraftCall::Ready(RetainedArgCall::Scalar(value))
                }
                EvaluatorArgCallGen::AddAll(value) => self.vector(value, false, lowerer)?,
                EvaluatorArgCallGen::AddJoined(value) => self.vector(value, true, lowerer)?,
            });
        }
        Ok(DraftRecipe {
            calls,
            format: snapshot.format,
            policy: snapshot
                .param_file
                .map(|(format, always)| RetainedSpawnParamFilePolicy::new(format, always)),
        })
    }

    fn vector(
        &mut self,
        value: EvaluatorVectorArgGen<Value<'v>>,
        joined: bool,
        lowerer: &mut AnalysisValueLowerer<'v>,
    ) -> anyhow::Result<DraftCall> {
        let expand_directories = value.options.expand_directories;
        let Some(EvaluatorVectorMapEachGen::Callable(callback)) = value.map_each else {
            let source = match value.source {
                EvaluatorVectorSourceGen::Sequence(values) => RetainedVectorSource::Sequence(
                    values
                        .into_iter()
                        .map(|item| {
                            super::vector_scalar_value(item, value.options.expand_directories)
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?
                        .into(),
                ),
                EvaluatorVectorSourceGen::Depset(value) => {
                    let lowered = lowerer
                        .lower(value, "Args vector depset")
                        .map_err(anyhow::Error::msg)?;
                    let AnalysisValueKind::Depset(depset) = lowered.kind() else {
                        anyhow::bail!("Args vector values must be a sequence or depset")
                    };
                    let retained = if expand_directories {
                        RetainedArgsDepset::new(depset.clone())
                    } else {
                        RetainedArgsDepset::new_unexpanded(depset.clone())
                    }?;
                    RetainedVectorSource::Depset(retained)
                }
            };
            let vector = RetainedVectorArg::new(source, value.options);
            return Ok(DraftCall::Ready(if joined {
                RetainedArgCall::AddJoined(vector)
            } else {
                RetainedArgCall::AddAll(vector)
            }));
        };
        let node = self
            .graph
            .add_callable(callback, lowerer)
            .map_err(anyhow::Error::msg)?;
        let inputs = match value.source {
            EvaluatorVectorSourceGen::Sequence(values) => {
                anyhow::ensure!(
                    values.len() <= CALLABLE_GRAPH_NODE_LIMIT,
                    "Args input count limit exceeded"
                );
                DraftInputs::Sequence(
                    values
                        .into_iter()
                        .map(|value| self.graph.add_value(value, lowerer))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(anyhow::Error::msg)?
                        .into(),
                )
            }
            EvaluatorVectorSourceGen::Depset(value) => DraftInputs::Depset(
                self.graph
                    .add_value(value, lowerer)
                    .map_err(anyhow::Error::msg)?,
            ),
        };
        let draft = DraftVector {
            inputs,
            node,
            options: value.options,
            location: value.registration_location,
        };
        Ok(if joined {
            DraftCall::Joined(draft)
        } else {
            DraftCall::All(draft)
        })
    }

    /// Install only audited executable roots before the actual Module is frozen.
    pub(super) fn seal(
        mut self,
        lowerer: &mut AnalysisValueLowerer<'v>,
    ) -> anyhow::Result<PublishedValueArenaBuilder> {
        // The producer Module normally has no slots. Every slot is nevertheless
        // audited, including anonymous compiler slots, before retaining it.
        self.graph
            .audit_module(lowerer)
            .map_err(anyhow::Error::msg)?;
        let (graph, roots) = self.graph.finish();
        self.module.set_extra_value(
            self.module
                .heap()
                .alloc(starlark::values::tuple::AllocTuple(roots)),
        );
        Ok(graph)
    }
}

pub(super) struct FrozenPublisher {
    graph: PublishedValueArena,
    originals: Arc<StarlarkPublishedOriginals>,
    projections: std::cell::RefCell<Projector>,
}
impl FrozenPublisher {
    pub(super) fn new(
        graph: PublishedValueArenaBuilder,
        roots: OwnedFrozenValue,
    ) -> Result<Self, String> {
        let originals = Arc::new(StarlarkPublishedOriginals::new(
            roots,
            Arc::from(graph.imports()),
        )?);
        let graph = graph.seal(originals.clone()).map_err(|e| e.to_string())?;
        Ok(Self {
            graph,
            originals,
            projections: std::cell::RefCell::new(Projector::default()),
        })
    }
    pub(super) fn value(&self, node: PublishedNodeId) -> Result<AnalysisValue, String> {
        self.projections
            .borrow_mut()
            .value(&self.graph.value(node).map_err(|e| e.to_string())?)
    }
    pub(super) fn resource_original(
        &self,
        node: PublishedNodeId,
    ) -> anyhow::Result<(
        slug_build_api_v2::published_value::PublishedValueRef,
        OwnedFrozenValue,
    )> {
        let published = self.graph.value(node)?;
        let slot = published
            .original_slot()
            .ok_or_else(|| anyhow::anyhow!("resource callback has no original root"))?;
        let original = self
            .originals
            .owned_root(slot.index())
            .map_err(anyhow::Error::msg)?;
        Ok((published, original))
    }
    pub(super) fn complete(
        &self,
        draft: DraftRecipe,
    ) -> anyhow::Result<(RetainedArgsRecipe, Option<RetainedSpawnParamFilePolicy>)> {
        let vector = |value: DraftVector| -> anyhow::Result<RetainedVectorArg> {
            let original = self.graph.value(value.node)?;
            let slot = original
                .original_slot()
                .ok_or_else(|| anyhow::anyhow!("callback has no original root"))?;
            let callable = self
                .originals
                .owned_root(slot.index())
                .map_err(anyhow::Error::msg)?;
            let mapper = RetainedArgsMapper::from_published(
                original,
                Arc::new(runtime::StarlarkRuntimeMapper::new(callable)),
            )?;
            let inputs = match value.inputs {
                DraftInputs::Sequence(values) => RetainedArgsInputs::Sequence(
                    values
                        .iter()
                        .map(|id| self.value(*id))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(anyhow::Error::msg)?
                        .into(),
                ),
                DraftInputs::Depset(id) => {
                    let value = self.value(id).map_err(anyhow::Error::msg)?;
                    let AnalysisValueKind::Depset(value) = value.kind() else {
                        anyhow::bail!("callback input is not a depset");
                    };
                    RetainedArgsInputs::Depset(value.clone())
                }
            };
            Ok(RetainedVectorArg::new(
                RetainedVectorSource::Mapped {
                    inputs,
                    mapper,
                    registration_location: value.location,
                },
                value.options,
            ))
        };
        let calls = draft
            .calls
            .into_iter()
            .map(|call| {
                Ok(match call {
                    DraftCall::Ready(value) => value,
                    DraftCall::All(value) => RetainedArgCall::AddAll(vector(value)?),
                    DraftCall::Joined(value) => RetainedArgCall::AddJoined(vector(value)?),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok((RetainedArgsRecipe::new(calls, draft.format), draft.policy))
    }
}
