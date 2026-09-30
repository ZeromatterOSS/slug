//! Immutable link Args over the existing lazy feature/variable owners.
use std::fmt;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::RetainedParamFileFormat;
use slug_build_api_v2::RetainedScalarArg;
use slug_build_api_v2::RetainedScalarValue;
use slug_build_api_v2::cc_features::CcFeatureConfiguration;
use slug_build_api_v2::cc_features::CcVariableAccess;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::environment::Methods;
use starlark::environment::MethodsStatic;
use starlark::values::Freeze;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::starlark_value;

use super::Variables;
use super::access::Access;
use crate::cc_features::cc_feature_configuration_value;
use crate::subrule_invocation::EvaluatorArgCallGen;
use crate::subrule_invocation::EvaluatorArgsSnapshot;
use crate::subrule_invocation::StarlarkArgs;
use crate::subrule_invocation::starlark_args_methods;
use crate::subrule_invocation::validate_named_format;

const PLACEHOLDER: &str = "LINKER_PARAM_FILE_PLACEHOLDER";

#[derive(Debug, ProvidesStaticType, NoSerialize, Trace, Freeze, Allocative)]
struct LinkArgsGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    configuration: CcFeatureConfiguration,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    action: CompactString,
    variables: V,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    format: RetainedParamFileFormat,
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    param_file: Option<(CompactString, bool)>,
}
type LinkArgs<'v> = LinkArgsGen<Value<'v>>;
type FrozenLinkArgs = LinkArgsGen<FrozenValue>;
starlark::starlark_complex_values!(LinkArgs);

impl<V> fmt::Display for LinkArgsGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Args")
    }
}
#[starlark_value(type = "Args")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for LinkArgsGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenLinkArgs;
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(starlark_args_methods)
    }
    fn write_hash(&self, _: &mut StarlarkHasher) -> starlark::Result<()> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "unhashable type: Args"
        )))
    }
}

pub(crate) fn construct<'v>(
    feature_configuration: Value<'v>,
    action_name: &str,
    build_variables: Value<'v>,
    parameter_file_type: Value<'v>,
    heap: Heap<'v>,
) -> anyhow::Result<Value<'v>> {
    let configuration = cc_feature_configuration_value(feature_configuration)
        .ok_or_else(|| anyhow::anyhow!("feature_configuration must be FeatureConfiguration"))?;
    anyhow::ensure!(
        Variables::from_value(build_variables).is_some(),
        "build_variables must be CcToolchainVariables"
    );
    let requested = if parameter_file_type.is_none() {
        None
    } else {
        Some(match parameter_file_type.unpack_str() {
            Some("UNQUOTED") => RetainedParamFileFormat::Multiline,
            Some("SHELL_QUOTED") => RetainedParamFileFormat::Shell,
            Some("GCC_QUOTED") => RetainedParamFileFormat::Gcc,
            Some(kind @ "WINDOWS") => {
                anyhow::bail!("get_link_args parameter_file_type={kind} is unsupported/deferred")
            }
            Some(_) => anyhow::bail!("get_link_args parameter_file_type is invalid"),
            None => anyhow::bail!("get_link_args parameter_file_type must be string or None"),
        })
    };
    let param_file = if requested.is_some() {
        let mut access = Access::new(build_variables, heap)?;
        configuration
            .get_command_line(action_name, &mut access)?
            .into_iter()
            .find(|arg| arg.contains(PLACEHOLDER))
            .map(|arg| -> anyhow::Result<_> {
                let flag = arg.replace('%', "%%").replace(PLACEHOLDER, "%s");
                validate_named_format("param_file_arg", &flag)?;
                Ok((CompactString::new(flag), true))
            })
            .transpose()?
    } else {
        None
    };
    let format = if param_file.is_some() {
        requested.expect("a policy was derived only for a requested format")
    } else {
        RetainedParamFileFormat::Shell
    };
    Ok(heap.alloc(LinkArgsGen {
        configuration,
        action: action_name.into(),
        variables: build_variables,
        format,
        param_file,
    }))
}

pub(crate) fn is_link_args(value: Value<'_>) -> bool {
    LinkArgs::from_value(value).is_some()
}

pub(crate) fn prepare<'v>(value: Value<'v>, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
    fn snapshot<'v, V: ValueLike<'v>>(
        args: &LinkArgsGen<V>,
        heap: Heap<'v>,
    ) -> anyhow::Result<EvaluatorArgsSnapshot<'v>> {
        // LinkCommandLine.java:93-112 retrieves linker_param_file before flags.
        let mut access = Access::new(args.variables.to_value(), heap)?;
        let filter = access
            .lookup("linker_param_file", false)?
            .map(|value| access.string(&value, "linker_param_file"))
            .transpose()?;
        let flags = args
            .configuration
            .get_command_line(&args.action, &mut access)?;
        let calls = flags
            .into_iter()
            .filter(|flag| {
                filter
                    .as_ref()
                    .is_none_or(|needle| !flag.contains(needle.as_str()))
            })
            .map(|flag| {
                EvaluatorArgCallGen::Scalar(RetainedScalarArg::new(
                    None::<CompactString>,
                    RetainedScalarValue::String(flag),
                    None::<CompactString>,
                ))
            })
            .collect();
        Ok(EvaluatorArgsSnapshot {
            calls,
            format: args.format,
            param_file: args.param_file.clone(),
        })
    }
    let snapshot = match LinkArgs::from_value(value) {
        Some(starlark::__macro_refs::Either::Left(args)) => snapshot(args, heap)?,
        Some(starlark::__macro_refs::Either::Right(args)) => snapshot(args, heap)?,
        None => return Ok(value),
    };
    Ok(StarlarkArgs::from_snapshot(snapshot, heap))
}

#[cfg(test)]
mod tests;
