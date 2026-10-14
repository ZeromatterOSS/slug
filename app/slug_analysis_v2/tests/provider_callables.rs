//! Original provider functions execute in the CURRENT configured evaluator.
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::RetainedArgCall;
use slug_build_api_v2::RetainedVectorSource;
use slug_build_api_v2::published_value::PublishedNode;
use slug_build_api_v2::published_value::PublishedValueRef;

use super::*;

const TYPES: &str = r#"
Answers = provider(fields=['value','shared','aliases'])
Plain = provider(fields=['value'])
def _initialize(callback, shared, constructor, raw):
    return {'callback': callback, 'shared': shared, 'aliases': (callback, {'callback': callback}, [callback, callback]), 'constructor': constructor, 'raw': raw}
Box, raw_box = provider(doc='ordinary provider initializer', fields=['callback','shared','aliases','constructor','raw'], init=_initialize)
"#;
const PRODUCER: &str = r#"
load("//:defs.bzl", "Box", "Plain", "raw_box")
def _produce(ctx):
    shared = [ctx.attr.marker, ctx.var['COMPILATION_MODE']]
    cpp = ctx.fragments.cpp
    if ctx.attr.leak:
        def callback(current, word='first', *, suffix='!'):
            return str(ctx.label)
    else:
        def callback(current, word='first', *, suffix='!'):
            if word == 'error': fail('actual provider callback failure')
            if cpp.compilation_mode() != shared[1]: fail('retained native configuration changed')
            lexical = Label(':lexical')
            mapped = Label('@alias//:marker')
            text = shared[0] + ':' + shared[1] + ':' + word + suffix
            output = current.actions.declare_file(current.label.name + '.from-provider')
            current.actions.write(output, text)
            return struct(text=text, shared=shared, output=output, lexical=lexical, mapped=mapped)
    def render(value): return shared[0] + ':' + value
    args = ctx.actions.args()
    args.add_all(shared, map_each=render, allow_closure=True)
    args.set_param_file_format('multiline')
    output = ctx.actions.declare_file('producer.arguments')
    ctx.actions.write(output=output, content=args)
    return [Box(callback=callback, shared=shared, constructor=Plain, raw=raw_box)]
produce = rule(implementation=_produce, fragments=['cpp'], attrs={'marker': attr.string(default='A'), 'leak': attr.bool()})
"#;
const CONSUMER: &str = r#"
load("//:defs.bzl", "Box", "Answers")
def _consume(ctx):
    info = ctx.attr.dep[Box]
    for callback in [info.aliases[0], info.aliases[1]['callback']] + info.aliases[2]:
        if callback != info.callback or {callback: True}[info.callback] != True:
            fail('provider callback alias changed')
    if ctx.attr.bad:
        info.callback(ctx, word='error')
        fail('original callback did not raise')
    result = info.callback(current=ctx)
    if result.lexical != Label('//test/moved:lexical') or result.mapped != Label('@@mapped+//:marker'):
        fail('original callback lost its lexical source or repository mapping')
    plain = info.constructor(value=result.text)
    raw = info.raw(shared=info.shared)
    if plain.value != result.text or raw.shared != info.shared:
        fail('original provider constructors changed')
    return [Answers(value=result.text, shared=result.shared, aliases=[result.shared,info.shared]), DefaultInfo(files=depset([result.output]))]
consume = rule(implementation=_consume, attrs={'dep': attr.label(providers=[Box]), 'bad': attr.bool()})
"#;
const BUILD: &str = "load('//test/moved:producer.bzl','produce')\nload('//consumer:consumer.bzl','consume')\nproduce(name='producer')\nproduce(name='leaky',leak=True)\nconsume(name='consumer',dep=':producer')\nconsume(name='failure',dep=':producer',bad=True)\n";
fn workspace() -> PathBuf {
    let root = scratch();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\nbazel_dep(name='mapped',version='1.0.0',repo_name='alias')\nlocal_path_override(module_name='mapped',path='mapped')\n").unwrap();
    for directory in ["test/moved", "consumer", "mapped"] {
        fs::create_dir_all(root.join(directory)).unwrap();
    }
    fs::write(root.join("defs.bzl"), TYPES).unwrap();
    fs::write(root.join("test/moved/producer.bzl"), PRODUCER).unwrap();
    fs::write(
        root.join("test/moved/BUILD.bazel"),
        "exports_files(['lexical'])\n",
    )
    .unwrap();
    fs::write(root.join("test/moved/lexical"), "lexical source marker\n").unwrap();
    fs::write(root.join("consumer/consumer.bzl"), CONSUMER).unwrap();
    fs::write(root.join("consumer/BUILD.bazel"), "").unwrap();
    fs::write(
        root.join("mapped/MODULE.bazel"),
        "module(name='mapped',version='1.0.0')\n",
    )
    .unwrap();
    fs::write(root.join("mapped/REPO.bazel"), "").unwrap();
    fs::write(root.join("mapped/.bazelignore"), "").unwrap();
    fs::write(
        root.join("mapped/BUILD.bazel"),
        "exports_files(['marker'])\n",
    )
    .unwrap();
    fs::write(root.join("mapped/marker"), "mapping marker\n").unwrap();
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    root
}
async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<ConfiguredNodeResult, String> {
    // The local override is a current request input, just like the root file
    // epoch. The plain analyze_request helper supplies host repositories only.
    root_target_request_with_inputs(
        dice,
        root,
        &format!("@@//:{target}"),
        configuration,
        Arc::new(RootActivationTracker::default()),
        root_epoch(root),
        &[("mapped+", "mapped")],
    )
    .await
    .map(|result| result.as_ref().clone())
}
fn field<'a>(result: &'a ConfiguredNodeResult, provider: &str, name: &str) -> &'a AnalysisValue {
    result
        .providers()
        .user(&ProviderId::new("//:defs.bzl", provider).unwrap())
        .unwrap()
        .field(name)
        .unwrap()
}
fn answer(result: &ConfiguredNodeResult) -> &str {
    field(result, "Answers", "value").as_str().unwrap()
}
fn captures(function: &PublishedValueRef) -> Vec<PublishedValueRef> {
    let PublishedNode::Function(row) = function.node() else {
        panic!("actual function descriptor")
    };
    function
        .arena()
        .edges(row.captures)
        .unwrap()
        .iter()
        .map(|edge| {
            let cell = function.arena().resolve(*edge).unwrap();
            let PublishedNode::Cell {
                contents: Some(contents),
            } = cell.node()
            else {
                panic!("bound actual capture")
            };
            cell.arena().resolve(*contents).unwrap()
        })
        .collect()
}

#[tokio::test]
async fn original_provider_functions_and_constructors_use_current_actions_and_shared_args_roots() {
    let root = workspace();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let configuration = typed_action_test_configuration();
    let producer = request(&dice, &root, "producer", configuration.clone())
        .await
        .unwrap();
    let shared = field(&producer, "Box", "shared").original_origin().unwrap();
    let callback = field(&producer, "Box", "callback")
        .original_origin()
        .unwrap();
    let PublishedNode::Function(descriptor) = callback.node() else {
        panic!("original function row")
    };
    assert_eq!(
        descriptor.code.source.data().root.label,
        CanonicalLabel::parse("@@//test/moved:producer.bzl").unwrap()
    );
    assert_eq!(
        descriptor.code.source.data().root.workspace_path,
        root.join("test/moved/producer.bzl")
    );
    assert!(
        descriptor
            .code
            .source
            .data()
            .root
            .repository_mapping
            .iter()
            .any(|(apparent, canonical)| apparent.as_str() == "alias"
                && canonical.as_str() == "mapped+"),
        "original producer module mapping: {:?}",
        descriptor.code.source.data().root.repository_mapping,
    );
    assert!(
        callback
            .arena()
            .sources()
            .iter()
            .any(|source| source.data().root == descriptor.code.source.data().root)
    );
    assert!(
        captures(callback)
            .iter()
            .any(|capture| capture.identity() == shared.identity()),
        "the provider child and actual function capture must be one arena node"
    );
    let write = producer.actions()[0].args_write_spec().unwrap();
    let RetainedArgCall::AddAll(vector) = &write.recipe().calls()[0] else {
        panic!("mapped Args")
    };
    let RetainedVectorSource::Mapped { mapper, .. } = vector.source() else {
        panic!("retained original mapper")
    };
    assert!(
        captures(mapper.published_callable())
            .iter()
            .any(|capture| capture.identity() == shared.identity()),
        "provider and Args capture roots share the same pure child"
    );
    assert_eq!(write.render_content().unwrap(), "A:A\nA:fastbuild\n");
    let consumer = request(&dice, &root, "consumer", configuration)
        .await
        .unwrap();
    assert_eq!(answer(&consumer), "A:fastbuild:first!");
    assert_eq!(consumer.actions().len(), 1);
    assert!(
        consumer.actions()[0].outputs()[0]
            .path()
            .ends_with("/consumer.from-provider")
    );
    assert_eq!(
        consumer.actions()[0].kind(),
        &ActionKind::Write {
            content: "A:fastbuild:first!".into(),
            is_executable: false
        }
    );
    let shared = field(&consumer, "Answers", "shared");
    let AnalysisValueKind::List(aliases) = field(&consumer, "Answers", "aliases").kind() else {
        panic!("list aliases")
    };
    assert_eq!(shared.container_identity(), aliases[0].container_identity());
    assert_eq!(
        aliases[0].container_identity(),
        aliases[1].container_identity()
    );
    drop(dice);
    fs::remove_dir_all(root).unwrap();
    assert_eq!(write.render_content().unwrap(), "A:A\nA:fastbuild\n");
}

#[tokio::test]
async fn original_provider_callback_errors_and_captured_producer_context_fail_at_actual_boundaries()
{
    let root = workspace();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let configuration = typed_action_test_configuration();
    let error = request(&dice, &root, "failure", configuration.clone())
        .await
        .unwrap_err();
    assert!(
        error.contains("actual provider callback failure") && error.contains("producer.bzl"),
        "{error}"
    );
    let error = request(&dice, &root, "leaky", configuration.clone())
        .await
        .unwrap_err();
    assert!(
        error.contains("ctx") && (error.contains("unsupported") || error.contains("capability")),
        "{error}"
    );
    assert_eq!(
        answer(
            &request(&dice, &root, "consumer", configuration)
                .await
                .unwrap()
        ),
        "A:fastbuild:first!"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn provider_function_source_defaults_and_configuration_change_and_restore_publication() {
    let root = workspace();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let configuration = typed_action_test_configuration();
    let original = request(&dice, &root, "consumer", configuration.clone())
        .await
        .unwrap();
    let original_provider = request(&dice, &root, "producer", configuration.clone())
        .await
        .unwrap();
    fs::write(
        root.join("test/moved/producer.bzl"),
        PRODUCER.replace("word='first'", "word='changed'"),
    )
    .unwrap();
    let defaults = request(&dice, &root, "consumer", configuration.clone())
        .await
        .unwrap();
    assert_eq!(answer(&defaults), "A:fastbuild:changed!");
    assert_ne!(original, defaults);
    fs::write(root.join("test/moved/producer.bzl"), PRODUCER).unwrap();
    assert_eq!(
        original,
        request(&dice, &root, "consumer", configuration.clone())
            .await
            .unwrap()
    );
    fs::write(
        root.join("test/moved/producer.bzl"),
        format!("{PRODUCER}\n"),
    )
    .unwrap();
    let source = request(&dice, &root, "consumer", configuration.clone())
        .await
        .unwrap();
    assert_eq!(answer(&original), answer(&source));
    let source_provider = request(&dice, &root, "producer", configuration.clone())
        .await
        .unwrap();
    assert_ne!(
        original_provider, source_provider,
        "unchanged callback outputs still retain changed original source in callable publication"
    );
    fs::write(root.join("test/moved/producer.bzl"), PRODUCER).unwrap();
    assert_eq!(
        original_provider,
        request(&dice, &root, "producer", configuration.clone())
            .await
            .unwrap()
    );
    assert_eq!(
        original,
        request(&dice, &root, "consumer", configuration.clone())
            .await
            .unwrap()
    );
    let changed = cc_configure_features::config(&[(NativeCommandOption::CompilationMode, "opt")]);
    let configured = request(&dice, &root, "consumer", changed).await.unwrap();
    assert_eq!(answer(&configured), "A:opt:first!");
    assert_ne!(original, configured);
    assert_eq!(
        original,
        request(&dice, &root, "consumer", configuration)
            .await
            .unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn observed_provider_callback_tracks_root_module_mapping_change_and_restore() {
    async fn observed(
        dice: &Arc<Dice>,
        root: &PathBuf,
        configuration: ConfigurationKey,
    ) -> Result<Arc<ConfiguredNodeResult>, String> {
        observed_root_target_request_with_inputs(
            dice,
            root,
            "@@//:consumer",
            configuration,
            Arc::new(RootActivationTracker::default()),
            root_epoch(root),
            &[("mapped+", "mapped")],
        )
        .await
    }

    let root = workspace();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let configuration = typed_action_test_configuration();
    let module = fs::read_to_string(root.join("MODULE.bazel")).unwrap();
    let original = observed(&dice, &root, configuration.clone()).await.unwrap();
    assert_eq!(answer(&original), "A:fastbuild:first!");

    // Keep the same canonical repository and current materialization. Only the
    // apparent mapping visible to the original producer module changes.
    let renamed = module.replace("repo_name='alias'", "repo_name='renamed'");
    assert_ne!(module, renamed);
    fs::write(root.join("MODULE.bazel"), renamed).unwrap();
    let error = observed(&dice, &root, configuration.clone())
        .await
        .unwrap_err();
    assert!(
        error.contains("repository '@alias' is not visible from @@//test/moved:producer.bzl"),
        "the original callback must resolve Label in its updated lexical mapping: {error}"
    );

    fs::write(root.join("MODULE.bazel"), module).unwrap();
    let restored = observed(&dice, &root, configuration).await.unwrap();
    assert_eq!(original, restored);
    fs::remove_dir_all(root).unwrap();
}

#[path = "provider_callables/typed_depset.rs"]
mod typed_depset;
