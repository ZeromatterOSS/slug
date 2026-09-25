/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::hash::Hash;

use allocative::Allocative;
use starlark::any::ProvidesStaticType;
use starlark::collections::StarlarkHasher;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;
use starlark::values::structs::AllocStruct;

use crate::provider::DeclarationOnlyAppleProviderKey;
use crate::provider::DeclarationOnlyAppleProviderKind;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct AppleCommon {
    platform: FrozenValue,
    platform_type: FrozenValue,
}

impl fmt::Display for AppleCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("apple_common")
    }
}

starlark::starlark_simple_value!(AppleCommon);

#[starlark_value(type = "apple_common")]
impl<'v> StarlarkValue<'v> for AppleCommon {
    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        if attribute == "platform" {
            return Some(self.platform.to_value());
        }
        if attribute == "platform_type" {
            return Some(self.platform_type.to_value());
        }
        let kind = match attribute {
            "Objc" => DeclarationOnlyAppleProviderKind::ObjcInfo,
            "XcodeVersionConfig" => DeclarationOnlyAppleProviderKind::XcodeVersionInfo,
            _ => return None,
        };
        Some(heap.alloc_simple(DeclarationOnlyAppleProviderKey(kind)))
    }

    fn dir_attr(&self) -> Vec<String> {
        ["Objc", "XcodeVersionConfig", "platform", "platform_type"]
            .map(str::to_owned)
            .into()
    }

    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(apple_common_methods)
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct AppleToolchain;

impl fmt::Display for AppleToolchain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("apple_toolchain")
    }
}

starlark::starlark_simple_value!(AppleToolchain);

#[starlark_value(type = "apple_toolchain")]
impl<'v> StarlarkValue<'v> for AppleToolchain {}

// The pinned apple_support empty-version branch passes only these five
// provider defaults. This is a separate Starlark value from the native
// DottedVersion.Option configuration representation, whose equality is
// deliberately spelling-sensitive in Bazel.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub(crate) struct DefaultSdkDottedVersion {
    spelling: &'static str,
    components: (u8, u8),
}

impl DefaultSdkDottedVersion {
    pub(crate) fn parse(version: &str) -> anyhow::Result<Self> {
        let (spelling, components) = match version {
            "8.4" => ("8.4", (8, 4)),
            "10.11" => ("10.11", (10, 11)),
            "9.0" => ("9.0", (9, 0)),
            "2.0" => ("2.0", (2, 0)),
            "1.0" => ("1.0", (1, 0)),
            _ => anyhow::bail!("unsupported apple_common.dotted_version input '{version}'"),
        };
        Ok(Self {
            spelling,
            components,
        })
    }
}

impl fmt::Display for DefaultSdkDottedVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.spelling)
    }
}

starlark::starlark_simple_value!(DefaultSdkDottedVersion);

#[starlark_value(type = "DottedVersion")]
impl<'v> StarlarkValue<'v> for DefaultSdkDottedVersion {
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(Self::from_value(other).is_some_and(|other| self.components == other.components))
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.components.hash(hasher);
        Ok(())
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        (attribute == "to_string").then(|| heap.alloc(self.spelling))
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["to_string".to_owned()]
    }
}

#[starlark_module]
fn apple_common_methods(builder: &mut MethodsBuilder) {
    fn apple_toolchain<'v>(#[starlark(this)] _this: Value<'v>) -> anyhow::Result<AppleToolchain> {
        Ok(AppleToolchain)
    }

    fn dotted_version<'v>(
        #[starlark(this)] _this: Value<'v>,
        version: &str,
    ) -> anyhow::Result<DefaultSdkDottedVersion> {
        DefaultSdkDottedVersion::parse(version)
    }
}

pub(crate) fn apple_common_globals(builder: &mut GlobalsBuilder) {
    // Bazel 9.2 builtins_bzl/common/objc/apple_platform.bzl defines these as
    // ordinary structs, not the parallel Java ApplePlatform enum.
    const PLATFORMS: [(&str, &str, &str, bool); 10] = [
        ("ios_device", "iPhoneOS", "ios", true),
        ("ios_simulator", "iPhoneSimulator", "ios", false),
        ("macos", "MacOSX", "macos", true),
        ("tvos_device", "AppleTVOS", "tvos", true),
        ("tvos_simulator", "AppleTVSimulator", "tvos", false),
        ("visionos_device", "XROS", "visionos", true),
        ("visionos_simulator", "XRSimulator", "visionos", false),
        ("watchos_device", "WatchOS", "watchos", true),
        ("watchos_simulator", "WatchSimulator", "watchos", false),
        ("catalyst", "MacOSX", "catalyst", true),
    ];
    let members = PLATFORMS
        .into_iter()
        .map(|(name, name_in_plist, platform_type, is_device)| {
            (
                name,
                builder.alloc(AllocStruct([
                    ("name", builder.alloc(name)),
                    ("name_in_plist", builder.alloc(name_in_plist)),
                    ("platform_type", builder.alloc(platform_type)),
                    ("is_device", builder.alloc(is_device)),
                ])),
            )
        })
        .collect::<Vec<_>>();
    let platform = builder.alloc(AllocStruct(members));
    // Bazel 9.2 builtins_bzl/common/objc/apple_platform.bzl:27-34.
    let platform_type = builder.alloc(AllocStruct([
        ("ios", builder.alloc("ios")),
        ("visionos", builder.alloc("visionos")),
        ("watchos", builder.alloc("watchos")),
        ("tvos", builder.alloc("tvos")),
        ("macos", builder.alloc("macos")),
        ("catalyst", builder.alloc("catalyst")),
    ]));
    builder.set(
        "apple_common",
        AppleCommon {
            platform,
            platform_type,
        },
    );
}

#[cfg(test)]
mod tests {
    use slug_build_api_v2::ProviderIdentity;
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use crate::package::FrozenAspectDefinition;
    use crate::package::FrozenRuleDefinition;
    use crate::package::build_file_loading_globals;
    use crate::package::bzlmod_loading_globals;
    use crate::package::loading_globals;
    use crate::provider::BzlEvaluationContext;
    use crate::provider::starlark_provider_identity;
    use crate::subrule::FrozenSubruleDefinition;

    fn evaluate(
        source: &str,
        globals: Globals,
    ) -> anyhow::Result<starlark::environment::FrozenModule> {
        let ast = AstModule::parse("//:apple_test.bzl", source.to_owned(), &Dialect::Standard)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let module = Module::new();
        let context = BzlEvaluationContext::new("//:apple_test.bzl");
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&context);
        eval.eval_module(ast, &globals)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        drop(eval);
        Ok(module.freeze()?)
    }

    #[test]
    fn facade_is_bzl_only_and_fails_closed_outside_admitted_members() {
        let source = "OBJ=apple_common.Objc\nXCODE=apple_common.XcodeVersionConfig\nTOOLCHAIN=apple_common.apple_toolchain()\nREFLECTION=dir(apple_common)\n";
        for globals in [loading_globals(), bzlmod_loading_globals()] {
            let module = evaluate(source, globals).unwrap();
            assert_eq!(
                module.get("REFLECTION").unwrap().value().to_string(),
                "[\"Objc\", \"XcodeVersionConfig\", \"apple_toolchain\", \"dotted_version\", \"platform\", \"platform_type\"]"
            );
            assert_eq!(
                starlark_provider_identity(module.get("OBJ").unwrap().value()),
                Some(ProviderIdentity::builtin("ObjcInfo"))
            );
            assert_eq!(
                starlark_provider_identity(module.get("XCODE").unwrap().value()),
                Some(ProviderIdentity::builtin("XcodeVersionInfo"))
            );
        }
        assert!(evaluate("X=apple_common", build_file_loading_globals()).is_err());
        for name in "XcodeProperties apple_host_system_env target_apple_env new_objc_provider get_apple_config"
            .split_whitespace()
        {
            assert!(
                evaluate(&format!("X=apple_common.{name}"), loading_globals()).is_err(),
                "{name}"
            );
        }
        for source in [
            "X=apple_common.apple_toolchain().platform",
            "X=apple_common.apple_toolchain().sdk_dir",
            "X=apple_common.apple_toolchain().platform_developer_framework_dir",
            "X=apple_common.apple_toolchain(1)",
            "X=apple_common.platform()",
            "X=apple_common.platform.ios_device()",
            "X=apple_common.platform.no_such_platform",
            "X=apple_common.platform.ios_device.no_such_field",
        ] {
            assert!(evaluate(source, loading_globals()).is_err(), "{source}");
        }
    }

    #[test]
    fn default_sdk_dotted_versions_are_typed_truthy_hashable_and_frozen() {
        let source = r#"
spellings = ['8.4', '10.11', '9.0', '2.0', '1.0']
values = [apple_common.dotted_version(s) for s in spellings]
lookup = {values[i]: i for i in range(len(values))}
def verify():
    for i in range(len(values)):
        value = values[i]
        if type(value) != 'DottedVersion' or not value:
            fail('DottedVersion type/truth: %s' % spellings[i])
        if str(value) != spellings[i] or repr(value) != spellings[i] or value.to_string != spellings[i]:
            fail('DottedVersion original spelling: %s' % spellings[i])
        again = apple_common.dotted_version(spellings[i])
        if value != again or lookup[again] != i or value == spellings[i]:
            fail('DottedVersion equality/hash: %s' % spellings[i])
        for j in range(len(values)):
            if (value == values[j]) != (i == j):
                fail('DottedVersion pairwise equality: %s/%s' % (i, j))
verify()
FROZEN_DOTTED_VERSION = values[2]
"#;
        for globals in [loading_globals(), bzlmod_loading_globals()] {
            let module = evaluate(source, globals).unwrap();
            let frozen = module.get("FROZEN_DOTTED_VERSION").unwrap();
            let value = frozen.value();
            assert_eq!(value.get_type(), "DottedVersion");
            assert_eq!(value.to_string(), "9.0");
            let scratch = Module::new();
            assert_eq!(
                value
                    .get_attr("to_string", scratch.heap())
                    .unwrap()
                    .unwrap()
                    .to_string(),
                "\"9.0\""
            );
        }
        for source in [
            "X=apple_common.dotted_version('1')",
            "X=apple_common.dotted_version('8.4.0')",
            "X=apple_common.dotted_version('')",
            "X=apple_common.dotted_version(None)",
            "X=apple_common.dotted_version(8)",
            "X=apple_common.dotted_version()",
            "X=apple_common.dotted_version('8.4', '9.0')",
        ] {
            assert!(evaluate(source, loading_globals()).is_err(), "{source}");
        }
    }

    #[test]
    fn platform_values_match_bazel_structs_and_survive_globals_drop() {
        let source = r#"
expected = [
    ('ios_device', 'iPhoneOS', 'ios', True),
    ('ios_simulator', 'iPhoneSimulator', 'ios', False),
    ('macos', 'MacOSX', 'macos', True),
    ('tvos_device', 'AppleTVOS', 'tvos', True),
    ('tvos_simulator', 'AppleTVSimulator', 'tvos', False),
    ('visionos_device', 'XROS', 'visionos', True),
    ('visionos_simulator', 'XRSimulator', 'visionos', False),
    ('watchos_device', 'WatchOS', 'watchos', True),
    ('watchos_simulator', 'WatchSimulator', 'watchos', False),
    ('catalyst', 'MacOSX', 'catalyst', True),
]
platform = apple_common.platform
def verify():
    if type(platform) != 'struct':
        fail('platform namespace type')
    if dir(platform) != sorted([row[0] for row in expected]):
        fail('platform namespace members')
    swift_table = {
        ('ios', None): platform.ios_device,
        ('ios', 'simulator'): platform.ios_simulator,
        ('macos', None): platform.macos,
        ('tvos', None): platform.tvos_device,
        ('tvos', 'simulator'): platform.tvos_simulator,
        ('xros', None): getattr(platform, 'visionos_device', None),
        ('xros', 'simulator'): getattr(platform, 'visionos_simulator', None),
        ('watchos', None): platform.watchos_device,
        ('watchos', 'simulator'): platform.watchos_simulator,
    }

    if len(swift_table) != 9 or swift_table[('xros', 'simulator')] != platform.visionos_simulator:
        fail('Swift platform table')
    for name, plist, platform_type, is_device in expected:
        value = getattr(platform, name)
        clone = struct(name=name, name_in_plist=plist, platform_type=platform_type, is_device=is_device)
        if type(value) != 'struct' or dir(value) != ['is_device', 'name', 'name_in_plist', 'platform_type']:
            fail('platform member shape: %s' % name)
        if value != clone or {value: name}[clone] != name:
            fail('platform structural equality/hash: %s' % name)
    if platform.macos == platform.catalyst:
        fail('macos and catalyst collapse')
    if not hasattr(platform, 'visionos_device') or not hasattr(platform, 'visionos_simulator'):
        fail('visionos fields absent')
verify()
FROZEN_PLATFORM = platform
PLATFORM_AGAIN = apple_common.platform
IOS_REPR = repr(platform.ios_device)
"#;
        for globals in [loading_globals(), bzlmod_loading_globals()] {
            let module = evaluate(source, globals).unwrap();
            assert_eq!(
                module.get("IOS_REPR").unwrap().value().to_string(),
                "\"struct(name=\\\"ios_device\\\", name_in_plist=\\\"iPhoneOS\\\", platform_type=\\\"ios\\\", is_device=True)\""
            );
            let frozen_platform = module.get("FROZEN_PLATFORM").unwrap();
            let platform_again = module.get("PLATFORM_AGAIN").unwrap();
            let namespace = frozen_platform.value();
            assert_eq!(namespace.get_type(), "struct");
            assert!(namespace.ptr_eq(platform_again.value()));
            let scratch = Module::new();
            assert_eq!(
                namespace
                    .get_attr("watchos_simulator", scratch.heap())
                    .unwrap()
                    .unwrap()
                    .get_type(),
                "struct"
            );
        }
    }

    #[test]
    fn platform_type_is_the_six_member_pinned_string_struct() {
        let source = r#"
types = apple_common.platform_type
expected = ['ios', 'visionos', 'watchos', 'tvos', 'macos', 'catalyst']
def verify():
    if type(types) != 'struct' or dir(types) != sorted(expected):
        fail('platform_type namespace shape')
    for name in expected:
        if getattr(types, name) != name:
            fail('platform_type member: %s' % name)
verify()
FROZEN_TYPES = types
"#;
        for globals in [loading_globals(), bzlmod_loading_globals()] {
            let module = evaluate(source, globals).unwrap();
            let binding = module.get("FROZEN_TYPES").unwrap();
            let types = binding.value();
            let scratch = Module::new();
            assert_eq!(
                types
                    .get_attr("catalyst", scratch.heap())
                    .unwrap()
                    .unwrap()
                    .unpack_str(),
                Some("catalyst")
            );
        }
        assert!(
            evaluate(
                "X=apple_common.platform_type.no_such_type",
                loading_globals()
            )
            .is_err()
        );
        assert!(evaluate("X=apple_common.platform_type()", loading_globals()).is_err());
    }

    #[test]
    fn provider_keys_freeze_and_retain_loading_schema_identity() {
        let source = "def impl(ctx): return []\ndef aimpl(target,ctx): return []\nOBJ=apple_common.Objc\nXCODE=apple_common.XcodeVersionConfig\nR=rule(implementation=impl,provides=[XCODE,OBJ,XCODE],attrs={'dep':attr.label(providers=[[OBJ,XCODE,OBJ]])})\nA=aspect(implementation=aimpl,provides=[OBJ,XCODE,OBJ])\nS=subrule(implementation=impl,attrs={'_dep':attr.label(default=Label('//:x'),providers=[[XCODE,OBJ,XCODE]])})\n";
        let module = evaluate(source, loading_globals()).unwrap();
        let objc = ProviderIdentity::builtin("ObjcInfo");
        let xcode = ProviderIdentity::builtin("XcodeVersionInfo");
        let rule = module.get("R").unwrap();
        let rule = rule.downcast::<FrozenRuleDefinition>().unwrap();
        assert_eq!(rule.advertised_providers(), &[xcode.clone(), objc.clone()]);
        let required = &rule
            .schema
            .iter()
            .find(|attribute| attribute.name == "dep")
            .unwrap()
            .required_providers[0];
        assert_eq!(required.as_ref(), &[objc.clone(), xcode.clone()]);
        let aspect = module.get("A").unwrap();
        let aspect = aspect.downcast::<FrozenAspectDefinition>().unwrap();
        assert_eq!(aspect.advertised_providers.as_ref(), &[objc, xcode]);
        module
            .get("S")
            .unwrap()
            .downcast::<FrozenSubruleDefinition>()
            .unwrap();
    }
}
