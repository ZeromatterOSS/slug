#!/usr/bin/env bash
# Opt-in native diagnostic. Never invokes Slug CLI/Bazel or acquires inputs.
set -euo pipefail
cd -- "$(dirname -- "$0")/../.."
for probe_tool in unshare prlimit strace perl timeout; do
    command -v "$probe_tool" >/dev/null
done
probe_scratch=$(mktemp -d /tmp/slug-sentinel-demand.XXXXXX)
printf 'Diagnostic evidence: %s/logs\n' "$probe_scratch"
mkdir "$probe_scratch/logs"
trap 'for probe_tree in workspace registry mirror; do
    if [[ -d "$probe_scratch/$probe_tree" ]]; then
        rm -rf -- "$probe_scratch/$probe_tree"
    fi
done' EXIT
probe_perl() {
    local -a probe_guard=()
    if [[ "$1" == run ]]; then probe_guard=(timeout --kill-after=2 58); fi
    "${probe_guard[@]}" perl - "$probe_scratch" "$1" <<'PERL'
use strict;
use warnings;
use JSON::PP qw(decode_json encode_json);
use Digest::SHA qw(sha256_hex);
use File::Path qw(make_path);
use File::Basename qw(dirname);
use Fcntl qw(F_GETFD F_SETFD);
use Cwd qw(abs_path);
use IO::Select;
use POSIX qw(setsid WNOHANG);
use Time::HiRes qw(time sleep);
require 'syscall.ph';
my ($scratch, $mode) = @ARGV;
$scratch =~ m{\A/tmp/slug-sentinel-demand\.[A-Za-z0-9]{6}\z} or die "scratch path\n";
my $logs = "$scratch/logs";
sub read_small {
    my ($path, $cap) = @_;
    open my $in, '<:raw', $path or die "$path: $!\n";
    -f $in or die "not regular: $path\n";
    my $bytes = '';
    while (1) {
        my $n = read $in, my $buf, 65536;
        defined $n or die "read $path: $!\n";
        last unless $n;
        length($bytes) + $n <= $cap or die "input cap: $path\n";
        $bytes .= $buf;
    }
    close $in or die "close $path: $!\n";
    return $bytes;
}
sub write_new {
    my ($path, $bytes) = @_;
    make_path(dirname($path));
    open my $out, '>:raw', $path or die "$path: $!\n";
    print {$out} $bytes or die "write $path: $!\n";
    close $out or die "close $path: $!\n";
}
sub observer_channel {
    my $name = 'slug-native-probe-observer';
    my $fd = syscall(SYS_memfd_create(), $name, 3);
    $fd >= 0 or die "memfd_create: $!\n";
    open my $handle, '+<&=', $fd or die "adopt memfd: $!\n";
    truncate($handle, 512) or die "truncate memfd: $!\n";
    fcntl($handle, 1033, 7) == 0 or die "seal memfd: $!\n";
    return $handle;
}
sub decode_observer {
    my ($raw) = @_;
    return {available => JSON::PP::false, reason => 'wrong-size'} unless length($raw) == 512;
    my @word = unpack('Q<64', $raw);
    my $magic = unpack('Q<', 'SLGOBS01');
    return {available => JSON::PP::false, reason => 'not-installed'}
        if !grep($_ != 0, @word);
    return {available => JSON::PP::false, reason => 'bad-header'}
        unless $word[0] == $magic && $word[1] == 1;
    return {available => JSON::PP::false, reason => 'bad-phase-selector'}
        unless $word[7] <= 2;
    return {available => JSON::PP::false, reason => 'bad-activity-selector'}
        unless $word[22] <= 2;
    my $frame = sub {
        my ($selector, $base, $stride, $activity) = @_;
        return undef unless $selector == 1 || $selector == 2;
        my $start = $base + ($selector - 1) * $stride;
        if (!$activity) {
            return {phase => $word[$start] & 0xffff,
                status => ($word[$start] >> 16) & 0xffff,
                monotonic_ns => $word[$start + 1], process_cpu_ns => $word[$start + 2],
                clock_validity => $word[$start + 3]};
        }
        my ($full, $copied) = @word[$start + 4, $start + 5];
        return {invalid => 'tag-length'} if $copied > 64 || $copied > $full;
        my $tag = join('', map { pack('Q<', $word[$start + $_]) } 6..13);
        $tag = substr($tag, 0, $copied);
        return {event_kind => $word[$start], monotonic_ns => $word[$start + 1],
            process_cpu_ns => $word[$start + 2], clock_validity => $word[$start + 3],
            full_tag_bytes => $full, copied_tag_bytes => $copied,
            tag_prefix_hex => unpack('H*', $tag)};
    };
    my $phase = $frame->($word[7], 14, 4, 0);
    return {available => JSON::PP::false, reason => 'bad-phase-frame'}
        if $phase && ($phase->{phase} < 1 || $phase->{phase} > 18 ||
            $phase->{status} < 1 || $phase->{status} > 3 || $phase->{clock_validity} > 3);
    my $activity = $frame->($word[22], 23, 14, 1);
    return {available => JSON::PP::false, reason => 'bad-activity-frame'}
        if $activity && ($activity->{invalid} || $activity->{event_kind} > 5 ||
            $activity->{clock_validity} > 3);
    return {available => JSON::PP::true, installed_pid => $word[2], disabled => $word[3],
        overflow_flags => $word[4], dropped_activity_samples => $word[5],
        activity_claim => $word[6], counters_are_independent => JSON::PP::true,
        counters => [@word[8..13]],
        phase => $phase, activity => $activity,
        interpretation => 'latest committed samples only; unsampled interval unknown'};
}
sub cap_observer_output {
    my ($telemetry, $json) = @_;
    return ($json, undef) if length($telemetry) + length($json) <= 8192;
    return (encode_json({available => JSON::PP::false, reason => 'output-cap'}) . "\n",
        'telemetry output cap');
}

# All child exits, including exceptions, pass through one bounded finalizer.
syscall(SYS_prctl(), 36, 1, 0, 0, 0) == 0 or die "subreaper: $!\n";
sub supervise {
    my ($name, $deadline, $caps, $launch, $inject, $with_observer, $json_padding) = @_;
    $^F = 1024;
    my $observer = $with_observer ? observer_channel() : undef;
    my $observer_number = $observer ? fileno($observer) : undef;
    my (@readers, @writers);
    for (0..2) {
        pipe(my $reader, my $writer) or die "pipe: $!\n";
        push @readers, $reader; push @writers, $writer;
    }
    my $started = time;
    my $pid = fork();
    defined $pid or die "fork: $!\n";
    if (!$pid) {
        setsid() >= 0 or die "setsid: $!\n";
        close $_ for @readers;
        open STDOUT, '>&', $writers[0] or die "stdout: $!\n";
        open STDERR, '>&', $writers[1] or die "stderr: $!\n";
        close $writers[0]; close $writers[1];
        if ($observer) {
            fcntl($observer, F_SETFD, 0) == 0 or die "inherit observer: $!\n";
        }
        $launch->(fileno($writers[2]), $observer ? fileno($observer) : undef);
        die "launch returned\n";
    }
    close $_ for @writers;
    my $select = IO::Select->new(@readers);
    my %index = map { fileno($readers[$_]) => $_ } 0..2;
    my @buffers = ('', '', '');
    my ($status, $stop, @reaped);
    local $SIG{TERM} = local $SIG{INT} = sub { $stop //= 'supervisor interrupted'; };
    my $drain = sub {
        for my $reader ($select->can_read(0.005)) {
            my $n = sysread $reader, my $bytes, 4096;
            if (!defined $n) {
                next if $!{EINTR};
                $stop //= "pipe read: $!";
                $select->remove($reader); close $reader; next;
            }
            if (!$n) { $select->remove($reader); close $reader; next; }
            my $i = $index{fileno($reader)};
            my $available = $caps->[$i] - length($buffers[$i]);
            $buffers[$i] .= substr($bytes, 0, $available);
            $stop //= "stream $i overflow" if $n > $available;
        }
    };
    eval {
        while (!defined($status) && !$stop) {
            $drain->();
            die "injected supervisor exception\n"
                if $inject && $buffers[0] =~ /^DESCENDANT_READY \d+$/m;
            my $reaped = waitpid($pid, WNOHANG);
            if ($reaped == $pid) { $status = $?; push @reaped, $pid; }
            elsif ($reaped == -1) { die "leader wait: $!\n"; }
            $stop //= 'wall deadline' if !defined($status) && time - $started >= $deadline;
        }
    };
    $stop //= $@ if $@;
    # No throwing path bypasses this finalizer; all waits and drains are bounded.
    kill 'KILL', -$pid;
    kill 'KILL', $pid unless defined $status; # Covers pre-setsid interruption.
    my $cleanup_started = time;
    my $no_children = 0;
    while (time - $cleanup_started < 2) {
        my $drained = eval { $drain->(); 1 };
        $stop //= "cleanup drain: $@" unless $drained;
        while (1) {
            my $reaped = waitpid(-1, WNOHANG);
            if ($reaped > 0) {
                $status = $? if $reaped == $pid;
                push @reaped, $reaped;
            } else {
                $no_children = 1 if $reaped == -1 && $!{ECHILD};
                last;
            }
        }
        last if $no_children && !$select->count;
        sleep 0.005 unless $select->count;
    }
    my $open_pipes = $select->count;
    close $_ for $select->handles;
    my $group_alive = kill 0, -$pid;
    my $clean = $no_children && !$open_pipes && !$group_alive;
    $stop //= 'cleanup incomplete' unless $clean;
    my $telemetry = '';
    my $telemetry_error;
    if ($observer) {
        if ($clean) {
            sysseek($observer, 0, 0) == 0 or $telemetry_error = "telemetry seek: $!";
            while (length($telemetry) < 512 && !defined($telemetry_error)) {
                my $n = sysread($observer, my $bytes, 512 - length($telemetry));
                if (!defined $n) { next if $!{EINTR}; $telemetry_error = "telemetry read: $!"; last; }
                if (!$n) { $telemetry_error = 'telemetry short read'; last; }
                $telemetry .= $bytes;
            }
        }
        close $observer or $telemetry_error //= "telemetry close: $!";
    }
    my $result = {
        raw_status => $status, stop => $stop, elapsed_seconds => time - $started,
        cleanup_complete => $clean ? JSON::PP::true : JSON::PP::false,
        reaped_pids => \@reaped, group_alive => $group_alive,
        no_children => $no_children, open_pipes => $open_pipes,
        telemetry_error => $telemetry_error,
    };
    my @names = qw(stdout stderr trace);
    write_new("$logs/$name.$names[$_]", $buffers[$_]) for 0..2;
    if ($with_observer) {
        write_new("$logs/$name.telemetry.raw", $telemetry);
        my $decoded = $clean && !$telemetry_error ? decode_observer($telemetry) :
            {available => JSON::PP::false,
                reason => $telemetry_error // 'process-tree-not-quiescent'};
        my $json = encode_json($decoded) . "\n";
        $json .= 'x' x $json_padding if $json_padding;
        my $cap_error;
        ($json, $cap_error) = cap_observer_output($telemetry, $json);
        $telemetry_error //= $cap_error;
        write_new("$logs/$name.telemetry.json", $json);
    }
    $stop //= $telemetry_error if defined $telemetry_error;
    $result->{stop} = $stop;
    $result->{telemetry_error} = $telemetry_error;
    write_new("$logs/$name.supervision.json", encode_json($result) . "\n");
    return ($result, \@buffers, $telemetry, $observer_number);
}
sub isolated {
    my ($observer_fd, @command) = @_;
    %ENV = (PATH => '/usr/bin:/bin', SLUG_SENTINEL_SCRATCH => $scratch);
    $ENV{SLUG_SENTINEL_OBSERVER_FD} = $observer_fd if defined $observer_fd;
    exec 'unshare', '--user', '--map-root-user', '--net',
        'prlimit', '--as=2147483648', '--cpu=15', '--fsize=16777216', @command;
    die "exec: $!\n";
}
if ($mode eq 'self-check') {
    decode_observer("\0" x 512)->{reason} eq 'not-installed' or die "zero decode\n";
    decode_observer('short')->{reason} eq 'wrong-size' or die "short decode\n";
    my @bad_frame = (0) x 64;
    @bad_frame[0, 1, 7] = (unpack('Q<', 'SLGOBS01'), 1, 3);
    decode_observer(pack('Q<64', @bad_frame))->{reason} eq 'bad-phase-selector'
        or die "bad frame decode\n";
    $bad_frame[7] = 1;
    decode_observer(pack('Q<64', @bad_frame))->{reason} eq 'bad-phase-frame'
        or die "bad phase decode\n";
    @bad_frame = (0) x 64;
    @bad_frame[0, 1, 22, 23] = (unpack('Q<', 'SLGOBS01'), 1, 1, 6);
    decode_observer(pack('Q<64', @bad_frame))->{reason} eq 'bad-activity-frame'
        or die "bad activity decode\n";
    my ($capped, $cap_error) = cap_observer_output("\0" x 512, 'x' x 8192);
    my $cap_stop;
    $cap_stop //= $cap_error;
    $cap_stop eq 'telemetry output cap' && decode_json($capped)->{reason} eq 'output-cap'
        or die "telemetry cap did not stop\n";
    my $output_failure = eval { write_new($logs, 'must fail'); 1 };
    !$output_failure && $@ or die "output failure did not fail closed\n";
    my $write = 'open my $o,"+<&=$ENV{SLUG_SENTINEL_OBSERVER_FD}" or die $!; ' .
        'sysseek($o,0,0)==0 or die $!; syswrite($o,pack("H*","8877665544332211"),8)==8 or die $!; ';
    for my $case (qw(normal deadline exception)) {
        my $body = $write . ($case eq 'normal' ? 'print "NORMAL\n"; exit 0;' :
            '$|=1; my $pid=fork(); defined($pid) or die $!; ' .
            'if (!$pid) { print "DESCENDANT_READY $$\n"; sleep 30; exit 0; } sleep 30;');
        my ($r, $buffers, $telemetry, $observer_number) = supervise(
            "self-check-$case", $case eq 'deadline' ? 0.25 : 2,
            [8192,8192,57344],
            sub { my ($trace_fd, $observer_fd) = @_; isolated($observer_fd, '/usr/bin/perl', '-e', $body) },
            $case eq 'exception', 1);
        $r->{cleanup_complete} && $r->{elapsed_seconds} < 5 or die "$case cleanup failed\n";
        length($telemetry) == 512 && substr($telemetry, 0, 8) eq pack('H*', '8877665544332211')
            or die "$case observer inheritance/retention failed\n";
        syscall(SYS_fcntl(), $observer_number, F_GETFD, 0) == -1 && $!{EBADF}
            or die "$case parent observer fd remained open\n";
        decode_observer($telemetry)->{reason} eq 'bad-header'
            or die "$case corrupt observer decode\n";
        if ($case eq 'normal') {
            !$r->{stop} && $r->{raw_status} == 0 && $buffers->[0] eq "NORMAL\n" or die "normal failed\n";
        } else {
            my $expected = $case eq 'deadline' ? 'wall deadline' : "injected supervisor exception\n";
            ($r->{stop} // '') eq $expected or die "$case wrong boundary\n";
            $buffers->[0] =~ /^DESCENDANT_READY (\d+)$/m or die "$case no live descendant\n";
            my $descendant = $1;
            grep($_ == $descendant, @{$r->{reaped_pids}}) or die "$case descendant not reaped\n";
            !kill(0, $descendant) or die "$case descendant survived\n";
        }
        print "$case: cleanup verified\n";
    }
    my ($cap_result, undef, undef, $cap_fd) = supervise(
        'self-check-telemetry-cap', 2, [8192,8192,57344],
        sub { my ($trace_fd, $observer_fd) = @_; isolated(
            $observer_fd, '/usr/bin/perl', '-e', $write . 'exit 0;') }, 0, 1, 8192);
    $cap_result->{cleanup_complete} &&
        $cap_result->{stop} eq 'telemetry output cap' &&
        $cap_result->{telemetry_error} eq 'telemetry output cap' &&
        syscall(SYS_fcntl(), $cap_fd, F_GETFD, 0) == -1 && $!{EBADF}
        or die "telemetry cap supervision did not fail closed\n";
    exit 0;
}
if ($mode eq 'compile') {
    my ($r, $buffers) = supervise('compiler', 60, [16777216,16777216,65536], sub {
        $ENV{PATH} = '/home/wgray/.rustup/toolchains/nightly-2025-09-14-x86_64-unknown-linux-gnu/bin:/usr/bin:/bin';
        exec 'timeout', '--kill-after=1', '60', 'cargo', 'test', '-q', '-p', 'slug_cli_v2',
            '--features', 'native-probe-observer', '--lib', '--no-run', '--message-format=json';
        die "cargo exec: $!\n";
    }, 0, 0);
    my ($errors, $finished, @executables) = (0, 0);
    for my $line (split /\n/, $buffers->[0]) {
        my $row = eval { decode_json($line) };
        if (!$row) { $r->{stop} //= 'invalid/incomplete compiler JSON'; next; }
        ++$errors if $row->{reason} eq 'compiler-message' && $row->{message}{level} eq 'error';
        $finished = $row->{success} if $row->{reason} eq 'build-finished';
        next unless $row->{reason} eq 'compiler-artifact';
        $r->{stop} //= 'unexpected slug binary target' if $row->{target}{name} eq 'slug' &&
            grep($_ eq 'bin', @{$row->{target}{kind}});
        push @executables, $row->{executable} if $row->{target}{name} eq 'slug_cli_v2' &&
            $row->{profile}{test} && $row->{executable} && grep($_ eq 'lib', @{$row->{target}{kind}});
    }
    write_new("$logs/compiler.selection.json", encode_json({
        errors => $errors, build_finished_success => $finished,
        executables => \@executables, supervision => $r}) . "\n");
    print encode_json($r), "\n";
    !$r->{stop} && $r->{raw_status} == 0 && !$errors && $finished && @executables == 1
        or die "compilation stopped; see $logs/compiler.*; no retry\n";
    write_new("$logs/executable.json", encode_json($executables[0]));
    exit 0;
}
$mode eq 'run' or die "invalid mode\n";
my $executable = abs_path(decode_json(read_small("$logs/executable.json", 16384)));
defined($executable) && -x $executable or die "missing compiler-reported executable\n";
# Pinned empty-workspace catalog, not the Bazel repository's root lockfile.
open my $git, '-|', 'git', '-C', '/home/wgray/bazel', 'show',
    '8220c6198837d5c13d53fea211cf3282aa12408a:src/test/tools/bzlmod/MODULE.bazel.lock'
    or die "git show: $!\n";
my $lock = '';
while (read $git, my $chunk, 65536) {
    $lock .= $chunk;
    length($lock) <= 1048576 or die "lock cap\n";
}
close $git or die "git show failed\n";
sha256_hex($lock) eq 'd7cbba1d746f5522d7dde4a2f7ea7a24d8f0befdf23d7cb4984689b48781049a'
    or die "pinned catalog hash\n";
my $rows = decode_json($lock)->{registryFileHashes};
keys(%$rows) == 184 or die "catalog row count\n";
my $cas = '/home/wgray/.cache/bazel/_bazel_wgray/cache/repos/v1/content_addressable/sha256';
my ($total, $metadata, $inventory) = (0, 0, '');
sub stage {
    my ($relative, $bytes, $provenance) = @_;
    $total += length($bytes);
    $total <= 1048576 or die "total fixture byte cap\n";
    write_new("$scratch/$relative", $bytes);
    $inventory .= join("\0", $relative, sha256_hex($bytes), length($bytes), $provenance) . "\n";
}
sub verified {
    my ($path, $hash, $size) = @_;
    my $bytes = read_small($path, 1048576);
    sha256_hex($bytes) eq $hash or die "hash mismatch: $path\n";
    !defined($size) || length($bytes) == $size or die "size mismatch: $path\n";
    return $bytes;
}
for my $url (sort keys %$rows) {
    next if $url eq 'https://bcr.bazel.build/bazel_registry.json';
    $url =~ m{\Ahttps://bcr\.bazel\.build/(modules/[A-Za-z0-9_.+-]+/[A-Za-z0-9_.+-]+/(?:MODULE\.bazel|source\.json))\z}
        or die "catalog URL\n";
    my $relative = $1;
    my $hash = $rows->{$url};
    $hash =~ /\A[0-9a-f]{64}\z/ or die "catalog hash\n";
    stage("registry/$relative", verified("$cas/$hash/file", $hash), $url);
    ++$metadata;
}
$metadata == 183 or die "metadata count\n";
stage('registry/bazel_registry.json', encode_json({mirrors => ["file://$scratch/mirror"]}),
    'test-owned file-mirror transport policy; original registry config excluded');
my @payloads = (
    ['platforms', '1.0.0', '3384eb1c30762704fbe38e440204e114154086c8fc8a8c2e3e28441028c019a8', 7879, undef],
    ['rules_shell', '0.6.1', 'e6b87c89bd0b27039e3af2c5da01147452f240f75d505f5b6880874f31036307', 23916,
     '/home/wgray/.cache/slug/downloads/sha256-5rh8ib0LJwOeOvLF2gEUdFLyQPddUF9baICHTzEDYwc_'],
);
for my $payload (@payloads) {
    my ($name, $version, $hash, $size, $path) = @$payload;
    my $descriptor = decode_json(read_small("$scratch/registry/modules/$name/$version/source.json", 1048576));
    my $url = $descriptor->{url};
    $url =~ m{\Ahttps://(github\.com/[A-Za-z0-9_./+-]+)\z} or die "archive URL\n";
    my $mirror = $1;
    $mirror !~ m{(?:\A|/)\.\.?(/|\z)} or die "archive traversal\n";
    stage("mirror/$mirror", verified($path // "$cas/$hash/file", $hash, $size), $url);
}
my $patch_hash = '5f0700eaa9a33770aae4ae8b06bec8e433f518eb50711378c8cd3a5d7854ff2d';
stage('registry/modules/rules_shell/0.6.1/patches/module_dot_bazel_version.patch',
    verified("$cas/$patch_hash/file", $patch_hash, 320), 'pinned rules_shell version patch');
# Exact R2 root BUILD/defs; only its fake-platform override/bodies are omitted.
my $candidate = verified('/tmp/slug-conflict-r2.XZJWwv/candidate.patch',
    '90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e');
$candidate =~ /\+    const BUILD: &str = r#"(.*?)\+"#;/s or die "R2 BUILD\n";
my $build = $1;
$build =~ s/^\+//mg;
$candidate =~ /\+            r##"(def _toolchain.*?)\+"##,/s or die "R2 defs\n";
my $defs = $1;
$defs =~ s/^\+//mg;
stage('workspace/BUILD.bazel', $build, 'preserved R2 root BUILD');
stage('workspace/defs.bzl', $defs, 'preserved R2 root defs');
stage('workspace/MODULE.bazel',
    "module(name = 'conflicts')\nregister_execution_platforms('//:platform')\nregister_toolchains('//:registration')\nbazel_dep(name = 'platforms', version = '1.0.0')\n",
    'preserved R2 root MODULE minus fake-platform override');
write_new("$logs/inventory", $inventory);
write_new("$logs/inventory.summary", "metadata=$metadata total_bytes=$total sha256=" . sha256_hex($inventory) . "\n");
my $absent = "$scratch/mirror/github.com/abseil/abseil-cpp/releases/download/20250814.1/abseil-cpp-20250814.1.tar.gz";
!-e $absent && !-l $absent or die "abseil mirror must remain absent\n";


my ($supervision, $buffers) = supervise('probe', 15, [8192,8192,57344], sub {
    my ($trace_fd, $observer_fd) = @_;
    chdir "$scratch/workspace" or die "chdir: $!\n";
    isolated($observer_fd, '/usr/bin/time', '-f', 'peak_rss_kib=%M', '-o', "$logs/resources",
        'strace', '-f', '-s', '4096', '-e', 'trace=openat,openat2', '-P', $absent,
        '-o', "/proc/self/fd/$trace_fd", $executable, '--ignored', '--exact',
        'payload_demand_probe::authentic_sentinel_demand', '--nocapture');
}, 0, 1);
my $demand = $buffers->[2] =~ /(?:openat|openat2)\([^\n]*"\Q$absent\E"/;
my $success = defined($supervision->{raw_status}) && $supervision->{raw_status} == 0 &&
    !$supervision->{stop} && $buffers->[0] =~ /^SLUG_SENTINEL_NATIVE_SUCCESS_PUBLISHED_0$/m;
my $result = $supervision->{stop} ? 'inconclusive' : $demand ? 'positive-demand' :
    $success ? 'sentinel-only-non-demand' : 'inconclusive';
my $summary = encode_json({result => $result, supervision => $supervision,
    abseil_open_syscall => $demand ? JSON::PP::true : JSON::PP::false,
    native_success_published_zero => $success ? JSON::PP::true : JSON::PP::false});
write_new("$logs/result.json", "$summary\n");
print "$summary\n";
print "stderr:\n$buffers->[1]";
exit($result eq 'inconclusive' ? 2 : 0);
PERL
}
timeout 5 env -i PATH=/usr/bin:/bin unshare --user --map-root-user --net \
    prlimit --as=2147483648 --cpu=2 --fsize=16777216 \
    strace -f -e trace=openat,openat2 -P /dev/null /usr/bin/true \
    >"$probe_scratch/logs/preflight.stdout" 2>"$probe_scratch/logs/preflight.stderr"
probe_perl self-check
if [[ $# == 1 ]] && [[ "$1" == "--self-check" ]]; then exit 0; fi
[[ $# == 0 ]] || { [[ $# == 1 && "$1" == "--compile-only" ]] || {
    printf 'usage: %s [--self-check|--compile-only]\n' "$0" >&2; exit 2;
}; }
probe_perl compile
if [[ $# == 1 ]] && [[ "$1" == "--compile-only" ]]; then exit 0; fi
probe_perl run
