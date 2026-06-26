def _nested_param_file_impl(ctx):
    out = ctx.actions.declare_output("nested_param_file.txt")

    main_args = ctx.actions.args()
    main_args.add("sh")
    main_args.add("-c")
    main_args.add("""
out="$1"
shift
if [ "$#" -ne 1 ]; then
    echo "expected one paramfile arg, got $#" >&2
    exit 1
fi
case "$1" in
    --cargo_manifest_args=@*) ;;
    *)
        echo "expected --cargo_manifest_args=@..., got $1" >&2
        exit 1
        ;;
esac
param="${1#--cargo_manifest_args=@}"
cat "$param" > "$out"
printf '\\n' >> "$out"
""")
    main_args.add("--")
    main_args.add(out.as_output())

    manifest_args = ctx.actions.args()
    manifest_args.use_param_file("--cargo_manifest_args=@%s", use_always = True)
    manifest_args.add("runfiles_dir")
    manifest_args.add("retain_a,retain_b")
    manifest_args.add("source=dest")

    ctx.actions.run([main_args, manifest_args], category = "nested_param_file")
    return [DefaultInfo(default_output = out)]

nested_param_file = rule(
    impl = _nested_param_file_impl,
    attrs = {},
)
