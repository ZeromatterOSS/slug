# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-bep-zero-default-repair`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: exact Bazel 9 ProtoJSON absent-zero compatibility.

## Goal and required implementation

Edit only the gate/BEP-probe libraries, their focused tests, and the prime-stage
test's frozen gate-source digest. Change only the two exit-code reads to default
an absent field before strict validation:

```python
_count(exit_data.get("code", 0))
```

Pinned Bazel 9.2 uses protobuf 33.4 JSON without default-value inclusion;
`ExitCode.code` is `int32`, so zero is omitted and nonzero remains a JSON number.
Do not change `_count`, the already-correct omitted `hits` handling, commands,
schemas, stages, descriptor/lifecycle logic, or any execution/output behavior.

Production is capped at 12 net lines, tests at 68, total 80. Make successful
BEP fixtures omit code zero. Prove gate success and `BEP_READY`; reject supplied
null, false, true, string `"0"`, negative, subclasses, and malformed values;
retain valid explicit nonzero remote-error behavior and privacy/schema pins.

## Stops and budget

Run only focused offline tests, compilation, caps/scope/diff, and independent
review. No Bazel, network, ordinary/home RC, artifact, config, docs, or new
files. Any broader numeric coercion, public/lifecycle change, extra production
owner, or second material correction is `REPLAN`. A later packet owns one
transported BEP probe; cache/RBE and the 43-test gate remain open.
