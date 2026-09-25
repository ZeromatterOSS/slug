//! The finite empty-version XcodeVersionInfo admitted from Bazel 9.2 builtins.
//!
//! The five callable fields are evaluator-local native methods. Their outputs
//! are fixed by the validated constructor tuple, so no Starlark closure or
//! borrowed evaluator value enters the retained provider collection.

use std::fmt;

use allocative::Allocative;
use slug_build_api_v2::XcodeVersionInfo;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::dict::AllocDict;
use starlark::values::starlark_value;

use crate::apple_common::DefaultSdkDottedVersion;

/// The instance is a singleton semantic fact only after all 14 arguments have
/// been authenticated. It has no evaluator-owned state to retain.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct DefaultXcodeVersionInfo;

impl fmt::Display for DefaultXcodeVersionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("XcodeVersionInfo(...)")
    }
}

starlark::starlark_simple_value!(DefaultXcodeVersionInfo);

#[starlark_value(type = "XcodeVersionInfo")]
impl<'v> StarlarkValue<'v> for DefaultXcodeVersionInfo {
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(xcode_version_info_methods)
    }
}

#[starlark_module]
fn xcode_version_info_methods(builder: &mut MethodsBuilder) {
    fn xcode_version<'v>(this: &DefaultXcodeVersionInfo) -> anyhow::Result<Value<'v>> {
        let _ = this;
        Ok(Value::new_none())
    }

    fn availability<'v>(
        this: &DefaultXcodeVersionInfo,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let _ = this;
        Ok(eval.heap().alloc_str("unknown").to_value())
    }

    fn execution_info<'v>(
        this: &DefaultXcodeVersionInfo,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let _ = this;
        Ok(eval.heap().alloc(AllocDict([
            ("requires-darwin", ""),
            ("supports-xcode-requirements-set", ""),
        ])))
    }

    fn minimum_os_for_platform_type<'v>(
        this: &DefaultXcodeVersionInfo,
        platform_type: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let _ = this;
        // Bazel 9.2 common/xcode/providers.bzl:73-91; Catalyst uses iOS minimum.
        let version = match platform_type {
            "ios" | "catalyst" => "8.4",
            "visionos" => "1.0",
            "watchos" => "2.0",
            "tvos" => "9.0",
            "macos" => "10.11",
            _ => anyhow::bail!("Unhandled platform type: {platform_type}"),
        };
        Ok(eval.heap().alloc(DefaultSdkDottedVersion::parse(version)?))
    }

    fn sdk_version_for_platform<'v>(
        this: &DefaultXcodeVersionInfo,
        platform: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let _ = this;
        // Bazel 9.2 common/xcode/providers.bzl:92-114 uses `platform.name`,
        // including for Java ApplePlatform objects. Only ten names are admitted.
        let name = platform
            .get_attr("name", eval.heap())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?
            .ok_or_else(|| anyhow::anyhow!("platform has no attribute `name`"))?;
        let name = name
            .unpack_str()
            .ok_or_else(|| anyhow::anyhow!("platform.name must be a string"))?;
        let version = match name {
            "ios_device" | "ios_simulator" => "8.4",
            "visionos_device" | "visionos_simulator" => "1.0",
            "watchos_device" | "watchos_simulator" => "2.0",
            "tvos_device" | "tvos_simulator" => "9.0",
            "macos" | "catalyst" => "10.11",
            _ => anyhow::bail!("Unhandled platform: {platform}"),
        };
        Ok(eval.heap().alloc(DefaultSdkDottedVersion::parse(version)?))
    }
}

/// Authenticates exactly the pinned apple_support empty-version call tuple.
pub(crate) fn construct<'v>(
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    args.no_positional_args(eval.heap())?;
    let names = args.names_map()?;
    let invalid = || {
        starlark::Error::new_other(anyhow::anyhow!(
            "apple_common.XcodeVersionConfig supports only the pinned empty-version default tuple"
        ))
    };
    if names.len() != 14 {
        return Err(invalid());
    }
    for (name, value) in names.iter() {
        let admitted = match name.as_str() {
            "ios_sdk_version" | "ios_minimum_os_version" => value.unpack_str() == Some("8.4"),
            "visionos_sdk_version" | "visionos_minimum_os_version" => {
                value.unpack_str() == Some("1.0")
            }
            "watchos_sdk_version" | "watchos_minimum_os_version" => {
                value.unpack_str() == Some("2.0")
            }
            "tvos_sdk_version" | "tvos_minimum_os_version" => value.unpack_str() == Some("9.0"),
            "macos_sdk_version" | "macos_minimum_os_version" => value.unpack_str() == Some("10.11"),
            "xcode_version" | "xcode_version_flag" => value.is_none(),
            "availability" => value.unpack_str() == Some("UNKNOWN"),
            "include_xcode_execution_info" => value.unpack_bool() == Some(false),
            _ => false,
        };
        if !admitted {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "apple_common.XcodeVersionConfig unsupported default tuple: argument `{}` has value {}",
                name.as_str(),
                value
            )));
        }
    }
    Ok(eval.heap().alloc_simple(DefaultXcodeVersionInfo))
}

pub fn is_default_xcode_version_info(value: Value<'_>) -> bool {
    DefaultXcodeVersionInfo::from_value(value).is_some()
}

pub fn default_xcode_version_info(value: Value<'_>) -> Option<XcodeVersionInfo> {
    is_default_xcode_version_info(value).then(XcodeVersionInfo::default_empty_version)
}

pub fn alloc_default_xcode_version_info(heap: &FrozenHeap) -> FrozenValue {
    heap.alloc(DefaultXcodeVersionInfo)
}

#[cfg(test)]
mod tests {
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use super::*;
    use crate::package::loading_globals;
    use crate::provider::BzlEvaluationContext;

    const CALL: &str = r#"INFO = apple_common.XcodeVersionConfig(
    ios_sdk_version = "8.4",
    ios_minimum_os_version = "8.4",
    visionos_sdk_version = "1.0",
    visionos_minimum_os_version = "1.0",
    watchos_sdk_version = "2.0",
    watchos_minimum_os_version = "2.0",
    tvos_sdk_version = "9.0",
    tvos_minimum_os_version = "9.0",
    macos_sdk_version = "10.11",
    macos_minimum_os_version = "10.11",
    xcode_version = None,
    availability = "UNKNOWN",
    xcode_version_flag = None,
    include_xcode_execution_info = False,
)
"#;

    fn evaluate(source: &str) -> anyhow::Result<starlark::environment::FrozenModule> {
        let ast = AstModule::parse(
            "//:xcode_info_test.bzl",
            source.to_owned(),
            &Dialect::Standard,
        )
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let module = Module::new();
        let context = BzlEvaluationContext::new("//:xcode_info_test.bzl");
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&context);
        eval.eval_module(ast, &loading_globals())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        drop(eval);
        Ok(module.freeze()?)
    }

    #[test]
    fn empty_version_constructor_exposes_all_five_typed_callable_fields_and_freezes() {
        let checks = r#"
def verify():
    if type(INFO) != 'XcodeVersionInfo':
        fail('provider type')
    if dir(INFO) != ['availability', 'execution_info', 'minimum_os_for_platform_type', 'sdk_version_for_platform', 'xcode_version']:
        fail('provider field inventory: %s' % dir(INFO))
    for name in dir(INFO):
        if type(getattr(INFO, name)) != 'function':
            fail('non-callable field: %s' % name)
    if INFO.xcode_version() != None or INFO.availability() != 'unknown':
        fail('default Xcode identity or availability')
    if INFO.execution_info() != {'requires-darwin': '', 'supports-xcode-requirements-set': ''}:
        fail('default execution requirements')
    for name, version in [
        ('ios_device', '8.4'), ('ios_simulator', '8.4'),
        ('visionos_device', '1.0'), ('visionos_simulator', '1.0'),
        ('watchos_device', '2.0'), ('watchos_simulator', '2.0'),
        ('tvos_device', '9.0'), ('tvos_simulator', '9.0'),
        ('macos', '10.11'), ('catalyst', '10.11'),
    ]:
        sdk = INFO.sdk_version_for_platform(getattr(apple_common.platform, name))
        if type(sdk) != 'DottedVersion' or str(sdk) != version:
            fail('SDK version: %s' % name)
    if str(INFO.sdk_version_for_platform(struct(name = 'ios_device'))) != '8.4':
        fail('platform.name projection')
    for name, version in [
        ('ios', '8.4'), ('visionos', '1.0'), ('watchos', '2.0'),
        ('tvos', '9.0'), ('macos', '10.11'), ('catalyst', '8.4'),
    ]:
        minimum = INFO.minimum_os_for_platform_type(getattr(apple_common.platform_type, name))
        if type(minimum) != 'DottedVersion' or str(minimum) != version:
            fail('minimum version: %s' % name)
    clone = apple_common.XcodeVersionConfig(
        ios_sdk_version = '8.4', ios_minimum_os_version = '8.4',
        visionos_sdk_version = '1.0', visionos_minimum_os_version = '1.0',
        watchos_sdk_version = '2.0', watchos_minimum_os_version = '2.0',
        tvos_sdk_version = '9.0', tvos_minimum_os_version = '9.0',
        macos_sdk_version = '10.11', macos_minimum_os_version = '10.11',
        xcode_version = None, availability = 'UNKNOWN', xcode_version_flag = None,
        include_xcode_execution_info = False,
    )
    if str(clone.sdk_version_for_platform(apple_common.platform.catalyst)) != '10.11':
        fail('second instance')
verify()
"#;
        let frozen = evaluate(&format!("{CALL}\n{checks}")).unwrap();
        let binding = frozen.get("INFO").unwrap();
        let info = binding.value();
        assert!(is_default_xcode_version_info(info));
        assert_eq!(
            default_xcode_version_info(info),
            Some(XcodeVersionInfo::default_empty_version())
        );
        let scratch = Module::new();
        let callable = info
            .get_attr("availability", scratch.heap())
            .unwrap()
            .unwrap();
        let mut eval = Evaluator::new(&scratch);
        assert_eq!(
            eval.eval_function(callable, &[], &[]).unwrap().unpack_str(),
            Some("unknown")
        );
        let heap = FrozenHeap::new();
        let rematerialized = alloc_default_xcode_version_info(&heap);
        assert!(is_default_xcode_version_info(rematerialized.to_value()));
    }

    #[test]
    fn constructor_refuses_missing_extra_positional_or_changed_default_inputs_eagerly() {
        for name in [
            "ios_sdk_version",
            "ios_minimum_os_version",
            "visionos_sdk_version",
            "visionos_minimum_os_version",
            "watchos_sdk_version",
            "watchos_minimum_os_version",
            "tvos_sdk_version",
            "tvos_minimum_os_version",
            "macos_sdk_version",
            "macos_minimum_os_version",
            "xcode_version",
            "availability",
            "xcode_version_flag",
            "include_xcode_execution_info",
        ] {
            let missing = CALL
                .lines()
                .filter(|line| !line.trim_start().starts_with(&format!("{name} = ")))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(evaluate(&missing).is_err(), "missing {name}");
            let changed = CALL
                .lines()
                .map(|line| {
                    if line.trim_start().starts_with(&format!("{name} = ")) {
                        format!("    {name} = 'wrong',")
                    } else {
                        line.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(evaluate(&changed).is_err(), "changed {name}");
        }
        for source in [
            CALL.replace(
                "    ios_sdk_version =",
                "    extra = 1,\n    ios_sdk_version =",
            ),
            CALL.replace("    ios_sdk_version =", "    1,\n    ios_sdk_version ="),
            CALL.replace(
                "include_xcode_execution_info = False",
                "include_xcode_execution_info = True",
            ),
            CALL.replace("ios_sdk_version = \"8.4\"", "ios_sdk_version = \"invalid\""),
        ] {
            assert!(evaluate(&source).is_err(), "{source}");
        }
        assert!(evaluate("INFO = apple_common.Objc()\n").is_err());
        assert!(
            evaluate(&format!(
                "{CALL}\nX = INFO.sdk_version_for_platform(struct(name = 'unknown'))\n"
            ))
            .is_err()
        );
        assert!(
            evaluate(&format!(
                "{CALL}\nX = INFO.minimum_os_for_platform_type('unknown')\n"
            ))
            .is_err()
        );
        assert!(evaluate(&format!("{CALL}\nX = INFO.execution_info(1)\n")).is_err());
    }
}
