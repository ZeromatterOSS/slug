//! Fixed globals and source-export injection have distinct owners.
use starlark::environment::GlobalsStatic;

use super::*;
use crate::bzl_environment::BzlEnvironmentMode;

pub(super) fn complete_loading_globals(
    bool_config: bool,
    bzlmod_native: bool,
    source: Option<&CanonicalLabel>,
) -> Globals {
    let mut globals = GlobalsBuilder::new();
    populate_fixed_globals(&mut globals, bool_config, bzlmod_native, source);
    globals.build()
}
fn populate_fixed_globals(
    globals: &mut GlobalsBuilder,
    bool_config: bool,
    bzlmod_native: bool,
    source: Option<&CanonicalLabel>,
) {
    populate_universe(globals);
    package_globals(globals);
    select_globals(globals);
    LibraryExtension::Json.add(globals);
    if bzlmod_native {
        globals.set("native", BzlmodNativeModule);
    } else if bool_config {
        globals.set("native", BzlNativeModule);
    } else {
        globals.set("native", NativeModule);
    }
    globals.set("attr", AttrModule);
    if bool_config {
        LibraryExtension::StructType.add(globals);
        bzl_only_globals(globals);
        bzl_visibility_globals(globals);
        globals.set(
            "proto_common_do_not_use",
            AllocStruct([("INCOMPATIBLE_ENABLE_PROTO_TOOLCHAIN_RESOLUTION", true)]),
        );
        globals.set("config", ConfigModule);
        globals.set("config_common", ConfigCommonModule);
        aspect_globals(globals);
        cc_common_globals(globals);
        globals.set("java_common", JavaCommonModule);
        label_globals(globals);
        testing_bootstrap_globals(globals);
        globals.set("OutputGroupInfo", OutputGroupInfo);
        globals.set("RunEnvironmentInfo", RunEnvironmentInfo);
        globals.set(
            "PackageSpecificationInfo",
            BuiltinProviderKey::new("PackageSpecificationInfo"),
        );
        globals.set("DefaultInfo", AnalysisBuiltinCallable::new("DefaultInfo"));
        if let Some(source) = source {
            populate_if_allowed(globals, source);
        }
    } else {
        globals.set("config", BuildFileConfigModule);
        build_genrule_globals(globals);
        build_module_metadata_globals(globals);
    }
    globals.set("platform_common", PlatformCommonModule);
    globals.set("depset", AnalysisBuiltinCallable::new("depset"));
}

pub(super) fn callable_registration_globals() -> Globals {
    let mut builder = GlobalsBuilder::new();
    populate_fixed_globals(&mut builder, true, false, None);
    crate::dotted_version::dotted_version_globals(&mut builder);
    builder.build()
}

pub(crate) fn loading_globals() -> Globals {
    complete_loading_globals(true, false, None)
}

pub(crate) fn bzlmod_loading_globals() -> Globals {
    complete_loading_globals(true, true, None)
}

pub(crate) fn loading_globals_for_source(source: &CanonicalLabel) -> Globals {
    complete_loading_globals(true, false, Some(source))
}

pub(crate) fn bzlmod_loading_globals_for_source(source: &CanonicalLabel) -> Globals {
    complete_loading_globals(true, true, Some(source))
}

pub(crate) fn build_file_loading_globals() -> Globals {
    complete_loading_globals(false, false, None)
}

/// Only fixed native globals are populated here. The prepared semantic
/// environment separately imports its original frozen source exports.
pub(crate) fn populate_bzl_environment_globals(
    builder: &mut GlobalsBuilder,
    source: &CanonicalLabel,
    mode: BzlEnvironmentMode,
) {
    match mode {
        BzlEnvironmentMode::Builtin => {
            populate_universe(builder);
            LibraryExtension::Json.add(builder);
            LibraryExtension::StructType.add(builder);
            provider_globals(builder);
            builder.set("depset", AnalysisBuiltinCallable::new("depset"));
            // Only the fixed native constructor is static. All public Apple
            // exports are evaluated source values in the prepared environment.
            static VERSION: GlobalsStatic = GlobalsStatic::new();
            let constructor = VERSION.function(crate::dotted_version::dotted_version_globals);
            let apple = builder.alloc(AllocStruct([("dotted_version", constructor)]));
            let internal = builder.alloc(AllocStruct([("apple_common", apple)]));
            builder.set("_builtins", AllocStruct([("internal", internal)]));
        }
        BzlEnvironmentMode::Build => populate_fixed_globals(builder, true, false, Some(source)),
        BzlEnvironmentMode::Bzlmod | BzlEnvironmentMode::BzlmodBootstrap => {
            populate_fixed_globals(builder, true, true, Some(source));
        }
    }
}

#[starlark_module]
pub(super) fn provider_globals(builder: &mut GlobalsBuilder) {
    fn provider<'v>(
        doc: Option<Value<'v>>,
        #[starlark(require = named)] fields: Option<Value<'v>>,
        #[starlark(require = named)] init: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        user_provider_from_arguments(doc, fields, init, eval)
    }
}
