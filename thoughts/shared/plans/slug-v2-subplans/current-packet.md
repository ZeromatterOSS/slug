# Current Slug V2 Work Packet

Packet: WP-7A-run-registry-precompiled-unsandboxed-validation-r1

Status: SELECTED after the exact-path matrix isolated managed-sandbox Unix bind
denial. Registry source is restored; its frozen compiled artifacts remain exact.
Complete output-conflict R2 remains untouched.

## Accepted environment attribution

Direct foreground `--serve` inside the managed sandbox failed binding the exact
87-byte socket in0.02s. The complementary approved invocation used the identical
binary, workspace and socket outside the sandbox; it emitted normal started
stderr, created the socket and remained live until the fixed2.00s SIGTERM. Peak
RSS was16128KiB, aggregate logs187 bytes, no process survived and the verified
socket was removed.

Evidence `/tmp/slug-daemon-exact-path.TwxPXM` hashes are stdout
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`, stderr
`3e96d8e39419b5c445c9041058f3411d1efb9d13eb59c482c012ae6817c965a5`, and time
`4fe9e3c541e7c8d9d30af617dfaa60dd6a2b7f946d9d7d876d3b17329a77ef9e`.
The exact controlled difference is the managed sandbox boundary, so it—not path,
workspace, binary or registry semantics—caused both integration daemon bind
failures. No external network or command request ran.

## Frozen validation recovery

The compiled registry candidate corresponds to accepted `build.rs` plus frozen
four-file source hashes already recorded in Stage5. Artifact SHA-256 values are:

- `target/debug/slug`: `d3f0dc23c0b394f37ac80c27cadde63bd2adf7a7a37538524d538c0b131417f9`
- CLI integration: `5338c19c1fc465dd19c607bf15ae5ac48d5be7f6016c36529545e1f2136f36f4`
- Server unit: `14bb5e4dbd1cebf16875efbf2226717ee9f23c73daff307e9037609d452fd31a`

First reverify these hashes, then invoke the already-compiled CLI integration
binary outside the sandbox once, selecting exactly
`equality_form_registry_reaches_one_shot_and_daemon_run`, with a12s wall deadline
and1s kill grace. It may create only its test-owned workspace/output base and
loopback Unix daemon; invalid `file://bad` must stop before analysis, launch,
remote execution or network. The loopback executor token remains syntactic only.
No credential environment or `.bazelrc` access.

If and only if that passes, invoke the already-compiled Server binary in the
ordinary sandbox once for exactly
`run_wire_carries_only_build_inputs_and_bounded_launch_authorization`, also <=12s.
Then mechanically reapply the four formatted source changes without compiling or
formatting and require byte-identical source hashes:

- Commands run `3af8a7d6978f3cd4c1844ecfe3097071f38c66fd64593e740b4df34d73df7144`
- CLI run `eeafc8aa5d1b0fd209e90e2c7669b4ada131e9194682f1d4b82a794c922ef52f`
- CLI integration `92994cc9a6f862484c4f7e93f719664e6e75c336a5d03dda070ee692d791bc34`
- Server tests `a5e7bcbe204761b39706ff15bb3365b94380bb34f325fe8831535c866caabbab`

Run only one combined affected default check under30s, then independent terminal
review. Any artifact/source hash drift, test/check failure, timeout, remote
connection, survivor or second edit is `REPLAN`; restore the four registry files.
No compilation, automatic retry, full suite, replay, acquisition, fixture/R2
change or timeout extension. Every test remains <=12s and <=15s absolute.

On acceptance commit/push registry parity alone, then design the complete-R2
authentic fixture application anew. Preserve complete R2 SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e` and old
probe SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
