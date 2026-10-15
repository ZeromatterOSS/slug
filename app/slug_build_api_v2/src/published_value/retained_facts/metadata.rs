use super::*;

fn parameters(
    parameters: &[PublishedParameter],
    draft: &Draft<'_>,
    meter: &mut Meter<'_>,
) -> Result<(), PublishedValueError> {
    for parameter in parameters {
        meter.record()?; // Complete name, mode, requirement and presence flags.
        if let PublishedParameterRequirement::Defaulted(edge) = parameter.requirement {
            draft.edge(edge, meter)?;
        }
        draft.optional_edge(parameter.annotation, meter)?;
    }
    Ok(())
}
fn code(
    code: &PublishedCode,
    draft: &Draft<'_>,
    sources: &mut sources::LocalSources,
    meter: &mut Meter<'_>,
) -> Result<(), PublishedValueError> {
    meter.record()?; // Name, definition span, source guard and range descriptor.
    draft.range(code.dependencies)?;
    sources.observe(&code.source, meter)?;
    for global in &*code.globals {
        meter.record()?; // Complete name/access and value-presence record.
        draft.optional_edge(global.value, meter)?;
    }
    Ok(())
}
fn target_identity(
    identity: &crate::AnalysisTargetIdentity,
    meter: &mut Meter<'_>,
) -> Result<(), PublishedValueError> {
    meter.record()?; // Identity variant plus complete CanonicalLabel incl mapping ID.
    if let Some(key) = identity.configured() {
        meter.record()?; // Configuration bytes.
        if key.toolchain_execution_platform().is_some() {
            meter.record()?; // Complete platform label incl optional mapping ID.
        }
    }
    Ok(())
}

pub(super) fn row(
    row: &PublishedNode,
    draft: &Draft<'_>,
    sources: &mut sources::LocalSources,
    meter: &mut Meter<'_>,
) -> Result<(), PublishedValueError> {
    match row {
        PublishedNode::Function(function) => {
            meter.record()?; // Symbol Local ordinal or complete Global name.
            meter.record()?; // Declared owner reference (payload remains in owner DAG).
            code(&function.code, draft, sources, meter)?;
            parameters(&function.parameters, draft, meter)?;
            draft.optional_edge(function.return_annotation, meter)?;
            draft.range(function.captures)?;
            meter.count_records(&function.capture_names)?;
        }
        PublishedNode::Intrinsic(intrinsic) => {
            meter.record()?; // Complete canonical API ID and version.
            parameters(&intrinsic.parameters, draft, meter)?;
            draft.optional_edge(intrinsic.return_annotation, meter)?;
            draft.optional_edge(intrinsic.receiver, meter)?;
        }
        PublishedNode::Code(value) => code(value, draft, sources, meter)?,
        PublishedNode::ProviderConstructor(constructor) => {
            meter.record()?; // Complete ProviderId source label/export name.
            let names = match &constructor.schema {
                PublishedProviderSchema::Schemaless => &[][..],
                PublishedProviderSchema::List(names)
                | PublishedProviderSchema::Documented(names)
                | PublishedProviderSchema::Initialized(names) => &**names,
            };
            meter.count_records(names)?;
            draft.optional_edge(constructor.init, meter)?;
        }
        PublishedNode::RawProviderConstructor { provider, .. } => draft.edge(*provider, meter)?,
        PublishedNode::Cell { contents } => draft.optional_edge(*contents, meter)?,
        PublishedNode::Set(range)
        | PublishedNode::List(range)
        | PublishedNode::Tuple(range)
        | PublishedNode::Dictionary(range) => draft.range(*range)?,
        PublishedNode::Struct { names, values } => {
            draft.range(*values)?;
            meter.count_records(names)?;
        }
        PublishedNode::Provider(provider) => {
            meter.record()?; // Complete Builtin name or User ProviderId.
            draft.range(provider.values)?;
            meter.count_records(&provider.names)?;
            if let Some(value) = &provider.files_to_run {
                meter.native(value.retained_visits())?;
            }
        }
        PublishedNode::ConfiguredTarget {
            identity,
            providers,
        } => {
            target_identity(identity, meter)?;
            for provider in &**providers {
                meter.record()?;
                match provider {
                    PublishedTargetProvider::Occurrence(edge) => draft.edge(*edge, meter)?,
                    PublishedTargetProvider::Native { provider, view } => {
                        draft.edge(*view, meter)?;
                        meter.native(provider.retained_visits())?;
                    }
                }
            }
        }
        PublishedNode::Depset(depset) => {
            for successor in &*depset.successors {
                meter.record()?;
                match successor {
                    PublishedDepsetInput::Direct(edge) | PublishedDepsetInput::Transitive(edge) => {
                        draft.edge(*edge, meter)?
                    }
                }
            }
        }
        PublishedNode::CcVariables(variables) => {
            draft.optional_edge(variables.parent, meter)?;
            for (_, edge) in &*variables.locals {
                meter.record()?;
                draft.edge(*edge, meter)?;
            }
            for (_, seed) in &*variables.seed {
                meter.record()?;
                match seed {
                    PublishedVariableSeed::Value(edge) => draft.edge(*edge, meter)?,
                    PublishedVariableSeed::Sequence(range) => draft.range(*range)?,
                    PublishedVariableSeed::Missing => {}
                    PublishedVariableSeed::Error(_) => meter.record()?,
                }
            }
        }
        PublishedNode::Native(native) => meter.native(native.retained_visits())?,
    }
    Ok(())
}
