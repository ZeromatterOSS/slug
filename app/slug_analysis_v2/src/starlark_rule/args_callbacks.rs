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
    inputs: RetainedArgsInputs,
    callback: usize,
    node: CallableNodeId,
    options: RetainedVectorOptions,
    location: Option<CompactString>,
}

pub(super) struct Publisher<'v> {
    module: &'v Module,
    graph: graph::ArgsCallableGraphBuilder<'v>,
    roots: Vec<Value<'v>>,
}
impl<'v> Publisher<'v> {
    pub(super) fn new(module: &'v Module) -> Self {
        Self {
            module,
            graph: graph::ArgsCallableGraphBuilder::new(module),
            roots: Vec::new(),
        }
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
        anyhow::ensure!(
            self.roots.len() < CALLABLE_GRAPH_NODE_LIMIT,
            "Args callback root limit exceeded"
        );
        let index = self.roots.len();
        self.roots.push(callback);
        let inputs = match value.source {
            EvaluatorVectorSourceGen::Sequence(values) => {
                anyhow::ensure!(
                    values.len() <= CALLABLE_GRAPH_NODE_LIMIT,
                    "Args input count limit exceeded"
                );
                RetainedArgsInputs::Sequence(
                    values
                        .into_iter()
                        .map(|value| self.graph.lower_input(value, lowerer))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(anyhow::Error::msg)?
                        .into(),
                )
            }
            EvaluatorVectorSourceGen::Depset(value) => {
                let lowered = self
                    .graph
                    .lower_input(value, lowerer)
                    .map_err(anyhow::Error::msg)?;
                let AnalysisValueKind::Depset(depset) = lowered.kind() else {
                    anyhow::bail!("Args callback requires depset")
                };
                RetainedArgsInputs::Depset(depset.clone())
            }
        };
        let draft = DraftVector {
            inputs,
            callback: index,
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
    ) -> anyhow::Result<Arc<CallableGraph>> {
        // The producer Module normally has no slots. Every slot is nevertheless
        // audited, including anonymous compiler slots, before retaining it.
        self.graph
            .audit_module(lowerer)
            .map_err(anyhow::Error::msg)?;
        self.module.set_extra_value(
            self.module
                .heap()
                .alloc(starlark::values::tuple::AllocTuple(self.roots)),
        );
        self.graph
            .finish()
            .map(Arc::new)
            .map_err(anyhow::Error::msg)
    }
}

pub(super) struct FrozenPublisher {
    graph: Arc<CallableGraph>,
    roots: OwnedFrozenValue,
}
impl FrozenPublisher {
    pub(super) fn new(graph: Arc<CallableGraph>, roots: OwnedFrozenValue) -> Self {
        Self { graph, roots }
    }
    pub(super) fn complete(
        &self,
        draft: DraftRecipe,
    ) -> anyhow::Result<(RetainedArgsRecipe, Option<RetainedSpawnParamFilePolicy>)> {
        let vector = |value: DraftVector| -> anyhow::Result<RetainedVectorArg> {
            let roots = TupleRef::from_value(self.roots.value())
                .ok_or_else(|| anyhow::anyhow!("invalid frozen Args callback roots"))?;
            anyhow::ensure!(
                value.callback < roots.len(),
                "unknown frozen Args callback root"
            );
            let callable = self.roots.map(|tuple| {
                TupleRef::from_value(tuple.to_value())
                    .expect("checked root tuple")
                    .content()[value.callback]
                    .unpack_frozen()
                    .expect("frozen root")
            });
            let mapper = RetainedArgsMapper::new(
                (*self.graph).clone(),
                value.node,
                Arc::new(runtime::StarlarkRuntimeMapper::new(callable)),
            )?;
            Ok(RetainedVectorArg::new(
                RetainedVectorSource::Mapped {
                    inputs: value.inputs,
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
