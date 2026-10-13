//! Bazel Depset.java283-290 truth through configured publication and native fields.
use super::*;

const DEFS: &str = r#"
Answers = provider(fields=['value'])
Proof = provider(fields=['rows'])
Files = provider(fields=['shared','alias','present'])
Flags = provider(fields=['value'])
LOADED_EMPTY = depset(transitive=[depset(),depset()])
LOADED_FALSE = depset([False])

def _check(value, expected):
    if bool(value) != expected: fail('bool(depset) differs from emptiness')
    selected = 'yes' if value else 'no'
    if selected != ('yes' if expected else 'no'): fail('if depset differs from emptiness')
    if (not value) != (not expected): fail('not depset differs from emptiness')
    conjunction = value and 'right'
    alternative = value or 'fallback'
    if expected:
        if conjunction != 'right' or alternative != value: fail('nonempty and/or changed')
    elif conjunction != value or alternative != 'fallback':
        fail('empty and/or changed')

def _matrix(ctx):
    _check(LOADED_EMPTY, False)
    _check(LOADED_FALSE, True)
    rows = []
    for order in ['default','postorder','preorder','topological']:
        empty = depset(order=order)
        nested_empty = depset(transitive=[empty,empty],order=order)
        false = depset([False],order=order)
        zero = depset([0],order=order)
        text = depset([''],order=order)
        indirect = depset(transitive=[false],order=order)
        # Singleton children are inlined by the native constructor. Two leaves
        # retain a genuinely shared transitive node in the diamond.
        shared = depset(['shared-a','shared-b'],order=order)
        left = depset(['left'],transitive=[shared],order=order)
        right = depset(['right'],transitive=[shared],order=order)
        diamond = depset(transitive=[left,right],order=order)
        for value in [empty,nested_empty]: _check(value,False)
        for value in [false,zero,text,indirect,diamond]: _check(value,True)
        rows.append(struct(order=order,empty=empty,nested_empty=nested_empty,
            false=false,zero=zero,text=text,indirect=indirect,shared=shared,
            alias=shared,independent=depset(['shared-a','shared-b'],order=order),diamond=diamond))
    return [Proof(rows=rows)]
matrix = rule(implementation=_matrix)

def _read_matrix(ctx):
    rows = ctx.attr.dep[Proof].rows
    for row in rows:
        for value in [row.empty,row.nested_empty]: _check(value,False)
        for value in [row.false,row.zero,row.text,row.indirect,row.diamond]: _check(value,True)
        if row.shared != row.alias or row.shared == row.independent:
            fail('truth changed depset occurrence identity')
        if depset(transitive=[row.shared],order=row.order) != row.shared:
            fail('truth changed sole-child identity')
    return [Answers(value='orders=' + str(len(rows))), Proof(rows=rows)]
read_matrix = rule(implementation=_read_matrix,attrs={'dep':attr.label(providers=[Proof])})

def _files(ctx):
    output = ctx.actions.declare_file('payload.txt')
    ctx.actions.write(output,ctx.attr.content)
    shared = depset([output])
    files = shared if ctx.attr.present else depset(transitive=[depset(),depset()])
    _check(shared,True)
    _check(files,ctx.attr.present)
    return [DefaultInfo(files=files),OutputGroupInfo(available=shared,optional=files,empty=depset()),
        Files(shared=shared,alias=shared,present=ctx.attr.present)]
files = rule(implementation=_files,attrs={'present':attr.bool(),'content':attr.string(default='content-A')})
def _read_files(ctx):
    target = ctx.attr.dep
    proof = target[Files]
    defaults = target[DefaultInfo].files
    groups = target[OutputGroupInfo]
    _check(defaults,proof.present)
    _check(groups.optional,proof.present)
    _check(groups.available,True)
    _check(groups.empty,False)
    _check(proof.shared,True)
    if proof.shared != proof.alias: fail('artifact depset alias changed')
    return [Answers(value='|'.join([str(bool(defaults)),str(bool(groups.optional)),str(bool(groups.available)),str(bool(groups.empty))])),proof]
read_files = rule(implementation=_read_files,attrs={'dep':attr.label(providers=[Files])})

def _flags(ctx):
    value = depset(ctx.attr.values)
    if ctx.attr.indirect: value = depset(transitive=[value])
    return [Flags(value=value)]
flags = rule(implementation=_flags,attrs={'values':attr.string_list(),'indirect':attr.bool()})
def _gate(ctx):
    flags = ctx.attr.dep[Flags].value
    if flags: fail('nonempty dependency flags require wrapper')
    return [Answers(value=ctx.var['COMPILATION_MODE'] + ';source-A')]
gate = rule(implementation=_gate,attrs={'dep':attr.label(providers=[Flags])})
"#;

const BUILD: &str = "load(':defs.bzl','matrix','read_matrix','files','read_files','flags','gate')\nmatrix(name='producer')\nread_matrix(name='middle',dep=':producer')\nread_matrix(name='matrix',dep=':middle')\nfiles(name='artifact',present=False)\nread_files(name='files',dep=':artifact')\nflags(name='flags',values=[],indirect=False)\ngate(name='gate',dep=':flags')\n";

fn workspace() -> PathBuf {
    let root = scratch();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    name: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_target_request_with_configuration(
        dice,
        root,
        &format!("@@//:{name}"),
        configuration,
        Arc::new(RootActivationTracker::default()),
    )
    .await
}

fn answer(result: &ConfiguredNodeResult) -> String {
    provider_value(result, &ProviderId::new("//:defs.bzl", "Answers").unwrap())
}

fn field_depset<'a>(
    fields: &'a SmallMap<compact_str::CompactString, slug_build_api_v2::AnalysisValue>,
    name: &str,
) -> &'a slug_build_api_v2::AnalysisDepset {
    match fields.get(name).unwrap().kind() {
        AnalysisValueKind::Depset(value) => value,
        other => panic!("expected depset in {name}, got {other:?}"),
    }
}

#[tokio::test]
async fn depset_truth_all_orders_falsey_elements_frozen_and_retained_dag_roundtrip() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = typed_action_test_configuration();
    let first = request(&dice, &root, "matrix", config.clone())
        .await
        .unwrap();
    assert_eq!(answer(&first), "orders=4");
    let proof = first
        .providers()
        .user(&ProviderId::new("//:defs.bzl", "Proof").unwrap())
        .unwrap();
    let AnalysisValueKind::List(rows) = proof.field("rows").unwrap().kind() else {
        panic!("matrix rows")
    };
    assert_eq!(rows.len(), 4);
    for row in rows.iter() {
        let AnalysisValueKind::Struct(fields) = row.kind() else {
            panic!("matrix row")
        };
        let empty = field_depset(fields, "empty");
        let nested_empty = field_depset(fields, "nested_empty");
        let shared = field_depset(fields, "shared");
        assert!(empty.is_empty() && nested_empty.is_empty());
        assert_eq!(nested_empty.depth(), 0);
        assert!(shared.shares_occurrence_with(field_depset(fields, "alias")));
        assert!(!shared.shares_occurrence_with(field_depset(fields, "independent")));
        let diamond = field_depset(fields, "diamond");
        assert_eq!(diamond.order(), shared.order());
        assert_eq!(diamond.depth(), 4);
        let children = diamond
            .successors()
            .map(|successor| match successor {
                slug_build_api_v2::AnalysisDepsetSuccessor::Transitive(child) => child,
                _ => panic!("diamond must retain its transitive branches"),
            })
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 2);
        for child in children {
            assert!(child.successors().any(|successor| matches!(successor,
                slug_build_api_v2::AnalysisDepsetSuccessor::Transitive(leaf) if leaf.shares_occurrence_with(shared))));
        }
    }
    assert_eq!(
        first,
        request(&dice, &root, "matrix", config).await.unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn depset_truth_native_default_files_output_groups_and_content_a_b_a() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = typed_action_test_configuration();
    let first = request(&dice, &root, "files", config.clone())
        .await
        .unwrap();
    assert_eq!(answer(&first), "False|False|True|False");
    fs::write(
        root.join("BUILD.bazel"),
        BUILD.replace("present=False", "present=True"),
    )
    .unwrap();
    let present = request(&dice, &root, "files", config.clone())
        .await
        .unwrap();
    assert_eq!(answer(&present), "True|True|True|False");
    assert_ne!(first, present);
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    assert_eq!(
        first,
        request(&dice, &root, "files", config.clone())
            .await
            .unwrap()
    );
    let producer = request(&dice, &root, "artifact", config.clone())
        .await
        .unwrap();
    fs::write(
        root.join("defs.bzl"),
        DEFS.replace("content-A", "content-B"),
    )
    .unwrap();
    let changed = request(&dice, &root, "artifact", config.clone())
        .await
        .unwrap();
    assert_eq!(producer.actions().len(), 1);
    assert_eq!(changed.actions().len(), 1);
    assert_ne!(
        producer.actions(),
        changed.actions(),
        "changed write content must invalidate the published action"
    );
    assert_eq!(
        answer(
            &request(&dice, &root, "files", config.clone())
                .await
                .unwrap()
        ),
        "False|False|True|False"
    );
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    assert_eq!(
        producer,
        request(&dice, &root, "artifact", config.clone())
            .await
            .unwrap()
    );
    assert_eq!(first, request(&dice, &root, "files", config).await.unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn depset_truth_generic_flag_gate_refuses_nonempty_repairs_and_source_config_a_b_a() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = typed_action_test_configuration();
    let first = request(&dice, &root, "gate", config.clone()).await.unwrap();
    assert_eq!(answer(&first), "fastbuild;source-A");
    for indirect in [false, true] {
        let empty = BUILD.replace(
            "indirect=False",
            if indirect {
                "indirect=True"
            } else {
                "indirect=False"
            },
        );
        fs::write(root.join("BUILD.bazel"), &empty).unwrap();
        assert_eq!(
            answer(&request(&dice, &root, "gate", config.clone()).await.unwrap()),
            "fastbuild;source-A"
        );
        // A single empty string is still an element, so the guard must refuse.
        fs::write(
            root.join("BUILD.bazel"),
            empty.replace("values=[]", "values=['']"),
        )
        .unwrap();
        let error = request(&dice, &root, "gate", config.clone())
            .await
            .unwrap_err();
        assert!(
            error.contains("nonempty dependency flags require wrapper"),
            "{error}"
        );
        fs::write(root.join("BUILD.bazel"), empty).unwrap();
        assert_eq!(
            answer(&request(&dice, &root, "gate", config.clone()).await.unwrap()),
            "fastbuild;source-A"
        );
    }
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    assert_eq!(
        first,
        request(&dice, &root, "gate", config.clone()).await.unwrap()
    );
    fs::write(root.join("defs.bzl"), DEFS.replace("source-A", "source-B")).unwrap();
    let changed = request(&dice, &root, "gate", config.clone()).await.unwrap();
    assert_eq!(answer(&changed), "fastbuild;source-B");
    assert_ne!(first, changed);
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    assert_eq!(
        first,
        request(&dice, &root, "gate", config.clone()).await.unwrap()
    );
    let base = config.slug_configuration().unwrap();
    let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::CompilationMode,
        Some("opt"),
        false,
    )]
    .into();
    let opt = ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    );
    let changed = request(&dice, &root, "gate", opt).await.unwrap();
    assert_eq!(answer(&changed), "opt;source-A");
    assert_ne!(first, changed);
    assert_eq!(first, request(&dice, &root, "gate", config).await.unwrap());
    fs::remove_dir_all(root).unwrap();
}
