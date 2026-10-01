//! Authentication and GC-traced capture registration for the finite Rust recipe.
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use sha2::Digest;
use sha2::Sha256;
use slug_build_api_v2::RustToolchainCallableSourceAuthority;
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
use starlark::values::tuple::AllocTuple;
use starlark_map::small_map::SmallMap;

use crate::bzl_module::rust_toolchain_helper::CALLER_LABEL;
use crate::bzl_module::rust_toolchain_helper::CALLER_SHA;
use crate::subrule_invocation::AnalysisEvaluationContext;

#[derive(Debug, Clone, Allocative)]
pub struct RustCallableRegistration {
    pub source: Arc<RustToolchainCallableSourceAuthority>,
    pub identity: Arc<()>,
    pub name_hash: u32,
}

#[derive(Debug, Trace, Freeze, ProvidesStaticType, NoSerialize, Allocative)]
struct RegisteredRustCallableGen<V> {
    #[trace(unsafe_ignore)]
    #[freeze(identity)]
    registration: RustCallableRegistration,
    captures: V,
}
type RegisteredRustCallable<'v> = RegisteredRustCallableGen<Value<'v>>;
type FrozenRegisteredRustCallable = RegisteredRustCallableGen<FrozenValue>;
starlark::starlark_complex_values!(RegisteredRustCallable);
impl<V> fmt::Display for RegisteredRustCallableGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<authenticated Rust callable registration>")
    }
}
#[starlark_value(type = "authenticated_rust_callable_registration")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for RegisteredRustCallableGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = FrozenRegisteredRustCallable;
}

fn constructor_callsite(begin_line: usize) -> bool {
    begin_line == 589
}

fn initializer_frame(eval: &Evaluator<'_, '_, '_>, filename: &str) -> bool {
    // Rust invokes the rule without an entry span, so native_call_context's
    // caller-location requirement cannot authenticate this otherwise real Def.
    // Local capture access below independently requires the immediate Def.
    eval.native_caller_function_filename().as_deref() == Some(filename)
        && eval
            .call_stack()
            .frames
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|frame| frame.name == "_rust_toolchain_impl")
}

pub(crate) fn register<'v>(
    fields: &SmallMap<CompactString, Value<'v>>,
    eval: &Evaluator<'v, '_, '_>,
) -> starlark::Result<()> {
    let Some(function) = fields.get("make_libstd_and_allocator_ccinfo").copied() else {
        return Ok(());
    };
    // Unrelated functions remain unregistered and are rejected by owned lowering.
    let Some(span) = eval.call_stack_top_location() else {
        return Ok(());
    };
    let sources = crate::builtin_restriction::source_identities_for_evaluator(eval)
        .map_err(starlark::Error::new_other)?;
    let mut matching = sources
        .iter()
        .filter(|(filename, _)| filename.as_str() == span.filename());
    let Some((_, source)) = matching.next() else {
        return Ok(());
    };
    if source.identity.label.to_string() != CALLER_LABEL {
        return Ok(());
    }
    let error = |message: &str| {
        starlark::Error::new_other(anyhow::anyhow!("Rust ToolchainInfo publication: {message}"))
    };
    let site = span.resolve_span();
    let evaluated: [u8; 32] = Sha256::digest(span.file.source().as_bytes()).into();
    if matching.next().is_some()
        || source.source_digest != CALLER_SHA
        || evaluated != CALLER_SHA
        || !constructor_callsite(site.begin.line)
        || !initializer_frame(eval, span.filename())
    {
        return Err(error("caller/source/frame/callsite authority mismatch"));
    }
    if eval
        .native_call_local_value("make_ccinfo")
        .is_none_or(|local| !local.ptr_eq(function))
    {
        return Err(error(
            "callback is not the directly calling initializer's make_ccinfo",
        ));
    }
    let context =
        AnalysisEvaluationContext::from_evaluator(eval).map_err(starlark::Error::new_other)?;
    let mut bindings = context.rust_toolchain_helpers().iter().filter(|binding| {
        let caller = &binding.authority().caller;
        caller.identity.label == source.identity.label
            && caller.identity.workspace_path == source.identity.workspace_path
            && caller.identity.repository_mapping == source.identity.repository_mapping
            && caller.digest == source.source_digest
    });
    let binding = bindings
        .next()
        .ok_or_else(|| error("missing prepared helper binding"))?;
    if bindings.next().is_some() {
        return Err(error("ambiguous prepared helper binding"));
    }
    let captures = [
        "cc_toolchain",
        "feature_configuration",
        "experimental_link_std_dylib",
        "rust_std",
    ]
    .map(|name| {
        eval.native_call_local_value(name)
            .ok_or_else(|| error("missing capture"))
    });
    let [cc, feature, dynamic, std] = captures;
    let captures = eval
        .heap()
        .alloc(AllocTuple([cc?, feature?, dynamic?, std?]));
    let identity = function
        .request_value::<&dyn StarlarkFunctionIdentity>()
        .ok_or_else(|| error("callback is not a Starlark function"))?;
    let registration = RustCallableRegistration {
        source: binding.authority().clone(),
        identity: identity.issue_identity(),
        name_hash: identity.name_hash().get(),
    };
    let registry = crate::evaluator_roots::rust_callable_registry(eval.module());
    DictMut::from_value(registry)?.aref.insert_hashed(
        function.get_hashed()?,
        eval.heap().alloc_complex(RegisteredRustCallable {
            registration,
            captures,
        }),
    );
    Ok(())
}

/// Snapshot after evaluation stops; the original Def and captures are traced roots.
pub fn rust_callable_registrations<'v>(
    module: &'v Module,
) -> Vec<(Value<'v>, RustCallableRegistration, Value<'v>)> {
    let Some(registry) = crate::evaluator_roots::existing_rust_callable_registry(module) else {
        return Vec::new();
    };
    DictRef::from_value(registry)
        .expect("Rust callable registry")
        .iter()
        .map(|(function, descriptor)| {
            let (registration, captures) = match RegisteredRustCallable::from_value(descriptor)
                .expect("Rust callable descriptor")
            {
                starlark::__macro_refs::Either::Left(value) => {
                    (value.registration.clone(), value.captures)
                }
                starlark::__macro_refs::Either::Right(value) => {
                    (value.registration.clone(), value.captures.to_value())
                }
            };
            (function, registration, captures)
        })
        .collect()
}

#[cfg(test)]
mod tests;
