//! Real frozen originals and independent rendering vectors; no callback recipes.
use std::sync::Arc;

use slug_build_api_v2::published_value::BYTE_LIMIT;
use slug_build_api_v2::published_value::NODE_LIMIT;
use starlark::environment::Globals;
use starlark::environment::LibraryExtension;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::dict::AllocDict;

use super::*;
#[derive(Default)]
struct Consumer {
    cancel: Cell<bool>,
    prints: RefCell<Vec<String>>,
    fail_print: Cell<bool>,
    cancel_on_print: Cell<bool>,
}
impl BuildInfoEvaluationContext for Consumer {
    fn is_cancelled(&self) -> bool {
        self.cancel.get()
    }
    fn print(&self, s: &str) -> Result<(), Error> {
        self.prints.borrow_mut().push(s.to_owned());
        if self.cancel_on_print.get() {
            self.cancel.set(true)
        }
        if self.fail_print.get() {
            Err(Error::Unsupported("sink refusal".into()))
        } else {
            Ok(())
        }
    }
    fn mnemonic(&self) -> &str {
        "ActualBuildInfoConsumer"
    }
}
fn runtime(source: &str) -> StarlarkBuildInfoRuntime {
    let module = Module::new();
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "unrelated_transform.bzl",
                source.to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::extended_by(&[LibraryExtension::Print]),
        )
        .unwrap();
    StarlarkBuildInfoRuntime::new(module.freeze().unwrap().get("transform").unwrap())
}
fn returned(expr: &str) -> StarlarkBuildInfoRuntime {
    runtime(&format!("def transform(status): return {expr}\n"))
}
#[test]
fn original_status_is_frozen_sorted_latin1_firstspace_lastduplicate_and_outlives_producer() {
    let backend = Arc::new(runtime(
        "def make():\n    captured = 'cell'\n    def f(status):\n        return {'@': '|'.join(status.keys()) + ':' + status['z'] + ':' + status[''] + ':' + captured}\n    return f\ntransform = make()\n",
    ));
    let weak = Arc::downgrade(&backend);
    let retained = backend.clone();
    drop(backend);
    let out = retained
        .render(
            b"z old\na first space value\nz last\n empty\nno_separator\n",
            b"@",
            &Consumer::default(),
        )
        .unwrap();
    assert_eq!(out, b"|a|z:last:empty:cell");
    let error = runtime("def transform(status):\n    status['new'] = 'bad'\n    return {}\n")
        .render(b"a b", b"x", &Consumer::default())
        .unwrap_err();
    assert!(matches!(error, Error::Callback(_)), "{error}");
    let text = error.to_string().to_ascii_lowercase();
    assert!(
        text.contains("frozen") || text.contains("immutable"),
        "{error}"
    );
    drop(retained);
    assert!(weak.upgrade().is_none());
    let all: Vec<u8> = (0..=255).collect();
    assert_eq!(
        returned("{}")
            .render(&[], &all, &Consumer::default())
            .unwrap(),
        all
    );
    assert_eq!(
        returned("{}")
            .render(&[], b"\xc3\xa9", &Consumer::default())
            .unwrap(),
        b"\xc3\xa9"
    );
}
#[test]
fn ordered_literal_passes_skip_empty_keys_do_not_recurse_and_encode_only_at_end() {
    let c = Consumer::default();
    for (expr, template, want) in [
        ("{'': 'bad'}", "text", "text"),
        ("{'a':'aa'}", "aa", "aaaa"),
        ("{'a':'b','b':'c'}", "a", "c"),
        ("{'b':'c','a':'b'}", "a", "b"),
        ("{'a':'x','xb':'yes'}", "ab", "yes"),
        ("{'x':'☃','☃':'é'}", "x", "é"),
        ("{'x':'😀'}", "x", "?"),
    ] {
        let expected: Vec<u8> = want
            .chars()
            .map(|c| u8::try_from(c as u32).unwrap_or(b'?'))
            .collect();
        assert_eq!(
            returned(expr).render(&[], template.as_bytes(), &c).unwrap(),
            expected,
            "{expr}"
        );
    }
    let out = runtime("def transform(s): return {'@': s['k']}\n")
        .render(b"k old\nk caf\xe9\r\nk \n", b"@", &c)
        .unwrap();
    assert!(out.is_empty());
    assert_eq!(
        runtime("def transform(s): return {'@': s['k']}\n")
            .render(b"k caf\xe9\r\n", b"@", &c)
            .unwrap(),
        b"caf\xe9\r"
    );
}
#[test]
fn return_validation_is_complete_before_replacement_and_borrowed_consumer_errors_are_current() {
    let c = Consumer::default();
    for (expr, needle) in [
        ("[]", "dictionary"),
        ("{1:'x'}", "key"),
        ("{'x':1}", "value"),
        ("{'x':'y'*100000,1:'z'}", "key"),
    ] {
        let e = returned(expr).render(&[], b"x", &c).unwrap_err();
        assert!(matches!(e, Error::InvalidReturn(_)), "{e}");
        assert!(e.to_string().contains(needle), "{e}");
    }
    let b = runtime("def transform(s):\n    print('sentinel')\n    return {'x':'ok'}\n");
    assert_eq!(b.render(&[], b"x", &c).unwrap(), b"ok");
    assert_eq!(&*c.prints.borrow(), &["sentinel"]);
    c.fail_print.set(true);
    assert!(matches!(
        b.render(&[], b"x", &c),
        Err(Error::Unsupported(_))
    ));
    c.fail_print.set(false);
    c.cancel_on_print.set(true);
    assert!(matches!(b.render(&[], b"x", &c), Err(Error::Cancelled)));
    let cancelled = Consumer::default();
    cancelled.cancel.set(true);
    assert!(matches!(
        b.render(&[], b"x", &cancelled),
        Err(Error::Cancelled)
    ));
    assert!(cancelled.prints.borrow().is_empty());
    assert!(matches!(
        runtime("def transform(a,b): return {}\n").render(&[], b"x", &Consumer::default()),
        Err(Error::Callback(_))
    ));
    let mut b = returned("{}");
    b.limits.timeout = Duration::ZERO;
    assert!(matches!(
        b.render(&[], b"x", &Consumer::default()),
        Err(Error::Limit(_))
    ));
    b.limits = Limits {
        heap: 0,
        ..Limits::default()
    };
    assert!(matches!(
        b.render(&[], b"x", &Consumer::default()),
        Err(Error::Limit(_))
    ));
}
#[test]
fn one_ledger_exact_plus_one_late_pass_and_poison_preserve_prepay() {
    let module = Module::new();
    let value = module.heap().alloc(AllocDict([("x", "y")]));
    // Independent actual operations:1dict+4validation+2Latin1+13passes+2scan+2encode=24.
    // Controlled bytes:2+StringHeader,1+StringHeader,1+VecHeader.
    let bytes = 4 + 2 * size_of::<String>() + size_of::<Vec<u8>>();
    for (work_seed, byte_seed, ok) in [
        (NODE_LIMIT - 24, BYTE_LIMIT - bytes, true),
        (NODE_LIMIT - 23, 0, false),
        (0, BYTE_LIMIT - bytes + 1, false),
    ] {
        let ledger = RefCell::new(Budget {
            work: work_seed,
            bytes: byte_seed,
            failed: false,
        });
        let result = render::output(value, b"x", &ledger, &|| Ok(()));
        assert_eq!(result.is_ok(), ok);
        if ok {
            assert_eq!(result.unwrap(), b"y");
            assert_eq!(ledger.borrow().work, NODE_LIMIT);
            assert_eq!(ledger.borrow().bytes, BYTE_LIMIT);
        } else {
            assert!(ledger.borrow().failed);
            assert!(ledger.borrow_mut().pay(0, 0).is_err());
        }
    }
    let ledger = RefCell::new(Budget::default());
    let value = module.heap().alloc(AllocDict([("a", "b"), ("b", "c")]));
    assert_eq!(
        render::output(value, b"a", &ledger, &|| Ok(())).unwrap(),
        b"c"
    );
    assert!(ledger.borrow().work > 24);
    assert!(ledger.borrow().bytes > bytes);
    let ledger = RefCell::new(Budget {
        work: 0,
        bytes: BYTE_LIMIT,
        failed: false,
    });
    assert!(render::latin1(b"x", &ledger, &|| Ok(())).is_err());
    assert!(ledger.borrow().failed);
}

#[test]
fn original_default_vectors_large_ordered_cascade_and_final_two_mib_bound() {
    let info = runtime(
        "def transform(s):\n    return {'{BUILD_EMBED_LABEL}':s.get('BUILD_EMBED_LABEL',''),'{BUILD_HOST}':s.get('BUILD_HOST','hostname'),'{BUILD_USER}':s.get('BUILD_USER','username')}\n",
    );
    let template = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs/build_info/templates/non_volatile_file.h.template"
    ));
    assert_eq!(info.render(&[],template,&Consumer::default()).unwrap(),b"#define BUILD_EMBED_LABEL \"\"\n#define BUILD_HOST \"hostname\"\n#define BUILD_USER \"username\"\n");
    let user = "u".repeat(4096);
    let host = format!("{}xxxx", "{BUILD_USER}".repeat(341));
    assert_eq!(host.len(), 4096);
    let status = format!("BUILD_USER {user}\nBUILD_HOST {host}\n");
    let output = info
        .render(status.as_bytes(), template, &Consumer::default())
        .unwrap();
    let expected=format!("#define BUILD_EMBED_LABEL \"\"\n#define BUILD_HOST \"{}xxxx\"\n#define BUILD_USER \"{user}\"\n",user.repeat(341)).into_bytes();
    assert_eq!(output, expected);
    assert_eq!(output.len(), 1_400_909);
    // Exact full-byte oracle; the corresponding Core fixture also pins SHA256
    // 2ed28fa8eff386e888aa14627017413d9e8a83aebc4c6e755f23bea331dcc259.
    let version = runtime(
        "def transform(s):\n    if 'BUILD_TIMESTAMP' not in s: fail('timestamp information is missing')\n    return {'{BUILD_SCM_REVISION}':s.get('BUILD_SCM_REVISION','0'),'{BUILD_SCM_STATUS}':s.get('BUILD_SCM_STATUS',''),'{BUILD_TIMESTAMP}':s['BUILD_TIMESTAMP']}\n",
    );
    assert_eq!(
        version
            .render(
                b"BUILD_TIMESTAMP 1\nBUILD_TIMESTAMP 2\r\n",
                b"{BUILD_SCM_REVISION}|{BUILD_SCM_STATUS}|{BUILD_TIMESTAMP}",
                &Consumer::default()
            )
            .unwrap(),
        b"0||2\r"
    );
    assert!(matches!(
        version.render(&[], b"", &Consumer::default()),
        Err(Error::Callback(_))
    ));
    let exact = returned("{'x':'y' * 2097152}")
        .render(&[], b"x", &Consumer::default())
        .unwrap();
    assert_eq!(exact.len(), slug_build_api_v2::BUILD_INFO_MAX_OUTPUT_BYTES);
    assert!(exact.iter().all(|b| *b == b'y'));
    let error = returned("{'x':'y' * 2097153}")
        .render(&[], b"x", &Consumer::default())
        .unwrap_err();
    assert!(matches!(error, Error::Limit(_)));
    assert!(error.to_string().contains("2MiB"), "{error}");
}

#[test]
fn long_frozen_status_hash_and_copy_work_are_prepaid_before_string_allocation() {
    let raw = format!("k {}", "v".repeat(8192));
    let chunk = |n: usize| n.div_ceil(256);
    // Borrowed status split/search/insert, typed pair Vec/Dict, Latin1 strings,
    // then SEPARATE full UTF8 hashing and frozen copying for both strings.
    let work = 2
        + chunk(raw.len())
        + 1
        + 1
        + chunk(raw.len())
        + 1
        + 1
        + 2
        + 3
        + (1 + chunk(1))
        + (1 + chunk(8192))
        + 2
        + 2 * (chunk(1) + chunk(8192));
    let bytes = size_of::<Vec<(&[u8], &[u8])>>()
        + 16 * size_of::<(&[u8], &[u8])>()
        + size_of::<Vec<(&[u8], &[u8])>>()
        + 16 * size_of::<(starlark::values::FrozenValue, starlark::values::FrozenValue)>()
        + size_of::<Vec<(starlark::values::FrozenValue, starlark::values::FrozenValue)>>()
        + 8 * 4 * (2 * size_of::<starlark::values::FrozenValue>() + 32)
        + 128
        + 2
        + 8192 * 2
        + 2 * size_of::<String>()
        + 1
        + 8192
        + 128;
    assert_eq!(work, 180);
    for (w, b, ok) in [
        (NODE_LIMIT - work, BYTE_LIMIT - bytes, true),
        (NODE_LIMIT - work + 1, 0, false),
        (0, BYTE_LIMIT - bytes + 1, false),
    ] {
        let module = Module::new();
        let before = module.frozen_heap().allocated_bytes();
        let ledger = RefCell::new(Budget {
            work: w,
            bytes: b,
            failed: false,
        });
        let result = render::input_dict(raw.as_bytes(), &module, &ledger, &|| Ok(()));
        assert_eq!(result.is_ok(), ok);
        if ok {
            assert_eq!(ledger.borrow().work, NODE_LIMIT);
            assert_eq!(ledger.borrow().bytes, BYTE_LIMIT);
        } else {
            assert!(ledger.borrow().failed);
            assert_eq!(
                module.frozen_heap().allocated_bytes(),
                before,
                "failure precedes frozen string/hash allocation"
            );
        }
    }
}

mod vectors;
