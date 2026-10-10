# WP1094 no-op observation union reuse

Packet: WP-7-306-m7a-no-op-observation-union-r1
Status: accepted — broader compiler producer materialization follows
Baseline: 3dfeb8658789f01191e16ad4c49e829716f22aa8 (WP1093 accepted)
Owner: existing private Loading union_host_observations and immutable validated
PathObservationEpoch certificate owner. Compatibility: exact existing admitted
certificate/error/Need/current source semantics; reuse Slug-native. Linux priority;
Windows minimal/deferred.

Outcome: avoid row/map allocation and per-row Arc bumps for already-complete no-op
certificate unions during normal unresolved-parent retries. Research
[target/wp1093/next-work-frontier.md](../../../../target/wp1093/next-work-frontier.md)
SHAe28c945f652145b34b4c0e3c59766bfb94fefe040076628657fd45238154491e:
private helper bzl_module.rs2854–2870 always sends both valid epochs to from_shared;
Workspace path_observation.rs824–865 allocates Vec/stablesorts/coalesces and fresh
SortedMap; retained SortedMap FromIterator137–149 builds/sorts OrderedMap. Normal
external child completion5370 and route/prepared finishes use this union repeatedly.
Existing duplicate finisher algebra proves original resultArc survives, yet map
rebuild persists. No native no-op frequency/per-key CPU/cause attribution is known.

Decision: borrowed shortcut in existing private helper only. Rightempty/sameunderlying
map returnsleft.dupe; leftempty returnsright.dupe; otherwise require EVERY right
exactdemand lookup exists inleft and resultArc pointerequal OR full result.as_ref()
Eq exactly matching current compact_shared855–858. Then return originalleft epoch.
Any newdemand/unequal result falls through original from_shared chain unchanged;
no early conflict error, partial union or wholesale rightsuperset substitution.
Inputs alreadyconstructor-validated immutable maps; raw image/path/operation validation
stays. Current source/route/child computations and freshness always remain upstream;
no certificate cache across epochs, new DICE key/lock/publicAPI/runtime/sourceprofile.

Result Eq includes full variant/issuedpath/request/imagedata/Lstatmetadata/error,
never justbody/digest. BudgetedImageBlock Eq is semanticdata, notbankidentity: preserve
stable LEFT original resultArc/body/image/credit ownership for equal distinctowners,
matching oldconstructor; no strongerbank Eq or rightowner adoption. Returning owned
normalDupe keeps caller lifetime independent; borrows exist only during decision.
No new retained fields/body/string/AST/cache/pool/interner. Existing Arc/Dupe/Allocative
and SortedMap owners/cleanup reused; Stage9 shared epoch row/WP1089 is authority.
Extra borrowedlookup/Eq work on non-subsets is a tradeoff, not universal speedclaim.
Primary acceptance is actual no-op returnedmap identity/no per-row newowner; native
memory/time prefix remains qualified, not CPU benefit or wholeproducer fit.

Source allowlist only app/slug_loading_v2/src/bzl_module.rs smallprivatehelper plus
adjacent existing owneralgebra proof, and existing host_package_load_tests/
external_bzl_frontier_tests.rs if needed for compact normalNeed/currentepoch proof.
No Workspace/Bzlmod/Core/thirdowner changes, fixturetree or unrelated extraction.
Largeparent remains existingcohesive helper; estimated10–25policy/80–130proof lines,
review actualgrowth. Root may scheduling/Stage9/compactreceipt/helper evidence edits.

Proof: actualmap pointer on empty/same/rightsubset; equal pointerdistinct result
retainsleftArc/rowstrongcounts and original creditowner; addition complete sorted
union and conflicts same earliest-demand error as originalfallback. Host/materialized
A/B namespaces, distinctoperations and representativeimage fullEq discriminate
body-only shortcuts. Reuse observed_prepared_identity_and_finisher_algebra and current
externalNeed→Need/childrepair/sourceA/B/A/sourceerrors/cycle/polldrop/Weak controls.
Workspace sharedepoch equality/errorordering/partitionedrestore accepted unchanged;
add no duplicate fullmatrix/allocator/counter. No real additional sourcebody reads.

Gates: independent DESIGN before ownership activation; stoppedwriters/pinnedformat/
sourcefreeze/actualsource/prelaunch/FINAL. Serial absolute pinnednightly2025-09-14
lockedoffline/jobs1 Loadingno-run/CargoJSON/currentlist/batchedexactpreflight/nonzero
pass; direct Loading/Core/CLI production check, originalprofile nativefeature Core
no-run + freshshell ignoredsingleton selection. OrdinaryLoading opt0 recovery retained
only for smalltests if originalprep hitscap; nativeLoadingopt1 unchanged. ONE changed
source native same350orderedtriples/18env/heldnoninheritedFD/currentcaller+observerpins.
Reuse supervisedWP1093 helpers/version2; no Cargo/writers overlap; preparation60s
separate/no workloadcredit, splitcompleteddeps ifneeded. Format/archive/diff/plancheck,
compactreceipt and acceptedcommit. No identicalnative replay or diagnostic-onlypacket.

All WP1093 receiptcaps unchanged: native10s/RSS256MiB/AS512MiB/work7/default5/lifetime8/
cleanup3/live+transfer16MiB/decode+write64MiB/source256/registry128/object1MiB/admitted16MiB/
dir128/name1KiB/total16KiB/FSIZE16MiB/log1/report4MiB. No caprise. REPLAN if subset
projection cannot preserve exact Eq/leftcreditowner/conflictorder/currentsource or
needs new authority/API/key/locking; routine invocation/fixture fixes preserve failures.
Fullproducer/time/wholefit/Rustc/actions/certificates/bootstrap remain open. Goal ACTIVE.

Immediate predecessor WP1093 in3dfeb8658:8focused/originalprofileCore/FINAL pass,
ONE7.272s native supervisor222.273/selfreported223.352MiB of256, WorkDeadline/source251/
caller7:296/status0, before environment frontier. JAVA_HOME finite empty-snapshot
behavior proven separately inunit/normalguard, nativeadmission unobserved. WP1092
sharing in09b345f11 staysaccepted with coldextra parse/metadataretention qualifications.
Borrow/move/share and use existing scoped string interners when lifetime/cleanup fit.

Accepted: DESIGN/source/FINAL reviews, current 20 focused tests and production/native-profile checks pass. One changed native prefix stayed within 226.40 MiB of 256 MiB and reached local_jdk generated-file materialization refusal. [Receipt](wp1094-no-op-observation-union-2026-10-10.json) binds qualified evidence; full producer/whole-workload fit remains open.
