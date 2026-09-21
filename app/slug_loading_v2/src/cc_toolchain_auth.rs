//! Private source authority for the finite CcToolchainInfo constructor recipes.
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use sha2::Digest;
use sha2::Sha256;
use slug_build_api_v2::cc_toolchain_callable::CcToolchainCallableKind;
use slug_build_api_v2::cc_toolchain_callable::CcToolchainCallableSource;
use starlark::any::ProvidesStaticType;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::Freeze;
use starlark::values::FrozenValue;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::DictMut;
use starlark::values::dict::DictRef;
use starlark::values::function::StarlarkFunctionIdentity;
use starlark::values::starlark_value;

const LABEL: &str = "@@rules_cc+//cc/private/rules_impl:cc_toolchain_info.bzl";
const SHA: [u8; 32] = [
    0xf1, 0x95, 0x89, 0x57, 0x21, 0x47, 0xb7, 0xdc, 0x8f, 0x1b, 0x16, 0xab, 0x96, 0x79, 0x1b, 0x76,
    0x51, 0x92, 0x3c, 0x36, 0x82, 0x1a, 0xed, 0x70, 0x86, 0x8a, 0x74, 0xbb, 0xfc, 0xe9, 0x63, 0xf5,
];

#[derive(Debug, Clone, Allocative)]
pub(crate) struct InitializerAuthority(CcToolchainCallableSource);

pub(crate) fn authenticate(eval: &Evaluator<'_, '_, '_>) -> Option<InitializerAuthority> {
    let span = eval.call_stack_top_location()?;
    let sources = crate::builtin_restriction::source_identities_for_evaluator(eval).ok()?;
    let mut matching = sources
        .iter()
        .filter(|(filename, _)| filename.as_str() == span.filename());
    let (_, source) = matching.next()?;
    let site = span.resolve_span();
    let evaluated: [u8; 32] = Sha256::digest(span.file.source().as_bytes()).into();
    if matching.next().is_some() || source.identity.label.to_string() != LABEL
        || source.source_digest != SHA || evaluated != SHA
        // The retained compiler records the opening parenthesis as call span.
        || (site.begin.line, site.end.line) != (164, 164)
    {
        return None;
    }
    Some(InitializerAuthority(CcToolchainCallableSource {
        label: source.identity.label.clone(),
        digest: SHA,
    }))
}

#[derive(Debug, Clone, Allocative)]
pub struct CallableRegistration {
    pub kind: CcToolchainCallableKind,
    pub source: CcToolchainCallableSource,
    pub identity: Arc<()>,
    pub name_hash: u32,
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
struct RegisteredCallableGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    registration: CallableRegistration,
    capture: V,
}
type RegisteredCallable<'v> = RegisteredCallableGen<Value<'v>>;
type FrozenRegisteredCallable = RegisteredCallableGen<FrozenValue>;
starlark::starlark_complex_values!(RegisteredCallable);
impl<V> fmt::Display for RegisteredCallableGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<authenticated callable registration>")
    }
}
#[starlark_value(type = "authenticated_callable_registration")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for RegisteredCallableGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenRegisteredCallable;
}

pub(crate) fn register<'v>(
    authority: &InitializerAuthority,
    values: &DictRef<'v>,
    eval: &Evaluator<'v, '_, '_>,
) -> starlark::Result<()> {
    let error = |message: &str| {
        starlark::Error::new_other(anyhow::anyhow!("CcToolchainInfo publication: {message}"))
    };
    let field = |name: &str| {
        values
            .get_str(name)
            .ok_or_else(|| error("pinned initializer omitted a required field"))
    };
    let cpp = crate::analysis_fragments::cpp_fragment_value(field("_cpp_configuration")?)
        .ok_or_else(|| error("requires a genuine Cpp configuration"))?;
    let pic_kind = if cpp
        .projection()
        .force_pic()
        .map_err(starlark::Error::new_other)?
    {
        CcToolchainCallableKind::AlwaysPic
    } else {
        CcToolchainCallableKind::SupportsPic
    };
    let entries = [
        (
            "needs_pic_for_dynamic_libraries",
            pic_kind,
            Value::new_none(),
        ),
        (
            "static_runtime_lib",
            CcToolchainCallableKind::StaticRuntime,
            field("_static_runtime_lib_depset")?,
        ),
        (
            "dynamic_runtime_lib",
            CcToolchainCallableKind::DynamicRuntime,
            field("_dynamic_runtime_lib_depset")?,
        ),
    ];
    let registry = crate::evaluator_roots::callable_registry(eval.module());
    let mut registry = DictMut::from_value(registry)?;
    for (name, kind, capture) in entries {
        if !capture.is_none()
            && crate::provider::StarlarkDepset::parts_from_value(capture).is_none()
        {
            return Err(error("runtime capture requires None or a depset"));
        }
        let function = field(name)?;
        let identity = function
            .request_value::<&dyn StarlarkFunctionIdentity>()
            .ok_or_else(|| error("pinned initializer did not return a Starlark function"))?;
        let registration = CallableRegistration {
            kind,
            source: authority.0.clone(),
            identity: identity.issue_identity(),
            name_hash: identity.name_hash().get(),
        };
        registry.aref.insert_hashed(
            function.get_hashed()?,
            eval.heap().alloc_complex(RegisteredCallable {
                registration,
                capture,
            }),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;

/// Snapshot only after evaluation has stopped: the module roots all moving Values.
pub fn callable_registrations<'v>(
    module: &'v Module,
) -> Vec<(Value<'v>, CallableRegistration, Value<'v>)> {
    let Some(registry) = crate::evaluator_roots::existing_callable_registry(module) else {
        return Vec::new();
    };
    DictRef::from_value(registry)
        .expect("callable registry dictionary")
        .iter()
        .map(|(function, descriptor)| {
            let (registration, capture) =
                match RegisteredCallable::from_value(descriptor).expect("registered descriptor") {
                    starlark::__macro_refs::Either::Left(value) => {
                        (value.registration.clone(), value.capture)
                    }
                    starlark::__macro_refs::Either::Right(value) => {
                        (value.registration.clone(), value.capture.to_value())
                    }
                };
            (function, registration, capture)
        })
        .collect()
}
