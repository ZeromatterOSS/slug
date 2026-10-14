//! Native constructor/cache seams run under Assert's Auto/Never/Always GC modes.
//! Authentic public dispatch and unchanged compile-variable producers are tested
//! by the Analysis integration owner; these helpers isolate native lifetimes.
use slug_build_api_v2::cc_features::CcVariableAccess;
use starlark::assert::Assert;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::list::AllocList;
use starlark::values::tuple::UnpackTuple;

use super::*;

#[starlark_module]
fn helpers(builder: &mut GlobalsBuilder) {
    fn make<'v>(vars: Value<'v>, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<Value<'v>> {
        construct(vars, eval.heap())
    }
    fn join<'v>(
        parent: Value<'v>,
        #[starlark(args)] rest: UnpackTuple<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        combine(parent, &rest.items, eval.heap())
    }
    fn lookup<'v>(
        vars: Value<'v>,
        name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<String> {
        let mut access = access::Access::new(vars, eval.heap())?;
        let value = access.lookup(name, true)?.unwrap();
        Ok(access.string(&value, name)?.to_string())
    }
    fn sequence<'v>(
        vars: Value<'v>,
        name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let mut access = access::Access::new(vars, eval.heap())?;
        let value = access.lookup(name, true)?.unwrap();
        let values = access.sequence(&value, name)?;
        let strings = values
            .map(|v| access.string(&v?, name))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(eval
            .heap()
            .alloc(AllocList(strings.iter().map(|v| v.as_str()))))
    }
    fn available<'v>(
        vars: Value<'v>,
        name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<bool> {
        Ok(access::Access::new(vars, eval.heap())?
            .lookup(name, false)?
            .is_some())
    }
    fn scoped<'v>(
        vars: Value<'v>,
        name: &str,
        field: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let mut access = access::Access::new(vars, eval.heap())?;
        let value = access.lookup(name, true)?.unwrap();
        let values = access.sequence(&value, name)?;
        let mut strings = Vec::new();
        for value in values {
            let mut scope = access.with_binding(name, value?);
            let value = scope.lookup(field, true)?.unwrap();
            strings.push(scope.string(&value, field)?);
        }
        Ok(eval
            .heap()
            .alloc(AllocList(strings.iter().map(|v| v.as_str()))))
    }
    fn intern<'v>(
        values: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        string_sequence(values, eval.heap())
    }
    fn empty<'v>(eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<Value<'v>> {
        Ok(dispatch::empty(eval.heap()))
    }
    fn toolchain<'v>(
        fields: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let fields = DictRef::from_value(fields).unwrap();
        Ok(crate::provider::StarlarkToolchainInfo::alloc_value(
            eval.heap(),
            fields
                .iter()
                .map(|(name, value)| (name.unpack_str().unwrap().into(), value)),
        ))
    }
}
fn harness() -> Assert<'static> {
    let mut a = Assert::new();
    a.disable_static_typechecking();
    a.globals_add(helpers);
    a
}

#[test]
fn native_variable_shape_parent_reference_and_raw_map_hash() {
    let a = harness();
    a.pass(
        r#"
p=make({})
q=make({})
assert_eq(type(p),'Variables')
assert_eq(repr(p),'<unknown object com.google.devtools.build.lib.rules.cpp.CcToolchainVariables$MapVariables>')
x=make({'x':['a'], 'd':{'k':['v']}})
y=make({'d':{'k':['v']}, 'x':['a']})
assert_eq(x,y)
assert_eq({x:7}[y],7)
tx=make({'toolchain':toolchain({'b':{'k':['v']},'a':['one']})})
ty=make({'toolchain':toolchain({'a':['one'],'b':{'k':['v']}})})
assert_eq(tx,ty)
assert_eq({tx:9}[ty],9)
assert_eq(sequence(tx,'toolchain.a'),['one'])
assert_eq(join(p,x),join(p,y))
assert_ne(join(p,x),join(q,x))
single=join(p,make({'x':'a'}))
assert_eq(type(single),'Variables')
assert_eq(repr(single),'<unknown object com.google.devtools.build.lib.rules.cpp.CcToolchainVariables$SingleVariables>')
assert_ne(single,make({'x':'a'}))
assert_eq(join(empty(),x),join(empty(),x))
assert_ne(join(empty(),x),join(make({}),x))
ancestor=make({'ancestor':'hidden'})
local=join(ancestor,make({'x':'local'}))
copied=join(p,local)
assert_eq(lookup(copied,'x'),'local')
assert_eq(available(copied,'ancestor'),False)
"#,
    );
    a.fails(
        "p=make({})\nx=join(p,make({'x':'a'}))\nz={x:1}",
        &["unhashable type"],
    );
    a.fails(
        "join(make({}),make({'x':1}),make({'x':2}))",
        &["Cannot overwrite existing variables"],
    );
}

#[test]
fn native_variables_shallow_live_values_dotted_snapshots_and_scopes() {
    harness().pass(
        r#"
data=['before']
top={'s':struct(items=data), 'direct':data}
v=make(top)
top['direct']=['replaced']
assert_eq(sequence(v,'s.items'),['before'])
data.append('after')
assert_eq(sequence(v,'direct'),['before','after'])
assert_eq(sequence(v,'s.items'),['before'])
rows=[struct(name='one'),struct(name='two')]
parent=make({'rows':rows,'rows.name':'full.key','x':'fallback'})
assert_eq(scoped(parent,'rows','rows.name'),['full.key','full.key'])
parent=make({'rows':rows,'x':'fallback'})
assert_eq(scoped(parent,'rows','rows.name'),['one','two'])
assert_eq(scoped(parent,'rows','x'),['fallback','fallback'])
longest=make({'a':struct(b=struct(c='short')),'a.b':struct(c='long')})
assert_eq(lookup(longest,'a.b.c'),'long')
assert_eq(lookup(make({'yes':True}),'yes'),'1')
assert_eq(sequence(make({'keys':{'first':1,'second':2}}),'keys'),['first','second'])
"#,
    );
}

#[test]
fn native_variables_missing_none_and_type_errors_remain_lazy() {
    let a = harness();
    a.pass(
        r#"
parent=make({'x':'parent','a.b':'parent.dotted'})
child=join(parent,make({'x':None,'a.b':None}))
assert_eq(available(child,'x'),False)
assert_eq(available(child,'a.b'),False)
unused=make({'unsupported':42,'good':'ok'})
assert_eq(lookup(unused,'good'),'ok')
assert_eq(available(make({'x':[]}), 'x.field'),False)
"#,
    );
    a.fails(
        "lookup(make({}),'missing')",
        &["Cannot find variable named 'missing'"],
    );
    a.fails("lookup(make({'x':[]}),'x.field')", &["expected structure"]);
    a.fails(
        "lookup(make({'x':struct()}),'x.field')",
        &["doesn't have a field named 'field'"],
    );
    a.fails(
        "v=make({'x':[]})\navailable(v,'x.field')\nlookup(v,'x.field')",
        &["Cannot find variable named 'x.field'"],
    );
    a.fails(
        "lookup(make({'x':['a']}),'x')",
        &["expected string, found sequence"],
    );
    a.fails(
        "lookup(make({'x':42}),'x')",
        &["unsupported C++ variable value of type int"],
    );
    a.fails(
        "available(make({'s':struct(bad=42)}),'s.bad')",
        &["unsupported C++ variable value of type int"],
    );
    // Native Iterables.transform adapts each element only when requested. The
    // first iteration's missing field must beat the later unsupported integer.
    a.fails(
        "scoped(make({'rows':[struct(),42]}),'rows','rows.missing')",
        &["doesn't have a field named 'missing'"],
    );
}

#[test]
fn native_variables_frozen_seeds_preserve_stale_sequence_and_raw_values() {
    let mut a = harness();
    a.module(
        "vars.bzl",
        r#"
data=['cached']
v=make({'s':struct(items=data),'direct':data})
sequence(v,'s.items')
data.append('uncached')
parent=make({'nested':v})
"#,
    );
    a.pass(
        r#"
load('vars.bzl','v')
assert_eq(type(v),'Variables')
assert_eq(sequence(v,'s.items'),['cached'])
assert_eq(sequence(v,'direct'),['cached','uncached'])
assert_eq(sequence(v,'s.items'),['cached'])
"#,
    );
}

#[test]
fn native_variable_constructor_sequence_validation_and_cycle_hash_rejection() {
    let a = harness();
    a.pass("data=['x']\nsnapshot=intern(data)\ndata.append('y')\nassert_eq(snapshot,['x'])\nassert_eq(intern(('a','b')),['a','b'])");
    a.fails("intern(['a']).append('b')", &["Immutable"]);
    a.fails("intern(['a',1])", &["must contain strings"]);
    a.pass("assert_eq(intern(range(0)),[])");
    a.fails("intern(range(1))", &["must contain strings"]);
    a.fails("intern({'a':True})", &["must be a sequence"]);
    a.fails("make([])", &["vars must be a dict"]);
    a.fails("make({1:'a'})", &["key must be string"]);
    a.fails(
        "x=[]\nx.append(x)\ny={make({'cycle':x}):True}",
        &["cyclic C++ variable payload is unsupported"],
    );
    a.fails(
        "x=[]\nv=make({'cycle':x})\nx.append(v)\nz={v:True}",
        &["cyclic C++ variable payload is unsupported"],
    );
}

#[test]
fn borrowed_variable_frontier_preserves_seed_aliases_and_charges_before_copy() {
    let module = starlark::environment::Module::new();
    let child = module.heap().alloc(AllocList(["cached"]));
    let mut cache = SmallMap::new();
    cache.insert(Name("value".into()), Entry::Value(child));
    cache.insert(
        Name("sequence".into()),
        Entry::Sequence(vec![child, child].into_boxed_slice()),
    );
    cache.insert(Name("missing".into()), Entry::Missing);
    cache.insert(
        Name("error".into()),
        Entry::Error("semantic failure".into()),
    );
    let vars = module.heap().alloc_complex(VariablesGen {
        kind: CcVariablesKind::Map,
        occurrence: CcVariablesOccurrence::new(),
        parent: None,
        names: Arc::from([CompactString::new("local")]),
        values: vec![child].into_boxed_slice(),
        cache: Cache::Live(LiveCache(Mutex::new(cache))),
    });
    module.set("vars", vars);
    let header = cc_variables_header(vars).unwrap();
    assert_eq!(header.local_count, 1);
    let occurrence = header.occurrence.clone();
    let mut local = None;
    visit_cc_variables_locals(vars, &mut |name, value| {
        assert_eq!(name, "local");
        local = Some(value.identity());
        Ok::<_, String>(())
    })
    .unwrap()
    .unwrap();
    assert_eq!(local, Some(child.identity()));
    let mut rejected = 0;
    let result = snapshot_cc_variables_seed_bounded(vars, &mut |_, _| {
        rejected += 1;
        Err::<(), _>("budget")
    });
    assert_eq!(result.unwrap_err(), "budget");
    assert_eq!(rejected, 1);
    let mut bytes = 0usize;
    let mut edges = 0usize;
    let seed = snapshot_cc_variables_seed_bounded(vars, &mut |b, e| {
        bytes += b;
        edges += e;
        Ok::<_, String>(())
    })
    .unwrap()
    .unwrap();
    assert!(bytes >= "semantic failure".len() + 2 * std::mem::size_of::<Value>());
    assert_eq!(edges, 7); // Four named entries and three outgoing Value edges.
    let CcVariableCacheEntry::Sequence(sequence) = &seed[1].1 else {
        panic!("sequence seed")
    };
    assert_eq!(sequence[0].identity(), sequence[1].identity());
    assert_eq!(sequence[0].identity(), child.identity());
    assert!(matches!(seed[2].1, CcVariableCacheEntry::Missing));
    assert!(
        matches!(&seed[3].1, CcVariableCacheEntry::Error(message) if message == "semantic failure")
    );
    let frozen = module.freeze().unwrap();
    let vars_owner = frozen.get("vars").unwrap();
    let vars = vars_owner.value();
    assert_eq!(cc_variables_header(vars).unwrap().occurrence, &occurrence);
    let seed = snapshot_cc_variables_seed_bounded(vars, &mut |_, _| Ok::<_, String>(()))
        .unwrap()
        .unwrap();
    let CcVariableCacheEntry::Sequence(sequence) = &seed[1].1 else {
        panic!("frozen sequence seed")
    };
    assert_eq!(sequence[0].identity(), sequence[1].identity());
}
