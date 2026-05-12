#!/bin/bash
# Sample RSS for a process, a pgrep pattern, or a command.
#
# Examples:
#   scripts/memory_smoke.sh --pgrep slugd
#   scripts/memory_smoke.sh --pid 12345
#   scripts/memory_smoke.sh --include-pgrep 'slugd\\[my-repo\\]' -- target/debug/slug build //...

set -euo pipefail

usage() {
    cat >&2 <<'EOF'
Usage:
  scripts/memory_smoke.sh [--interval SEC] --pid PID
  scripts/memory_smoke.sh [--interval SEC] --pgrep PATTERN
  scripts/memory_smoke.sh [--interval SEC] [--include-pgrep PATTERN] -- COMMAND [ARG...]
  scripts/memory_smoke.sh [--interval SEC] -- COMMAND [ARG...]

Reports peak and final RSS in KiB. In command mode, samples the command
process, its descendants, and any extra --include-pgrep matches while the
command is running.
EOF
}

interval=1
mode=
pid=
pattern=
include_pgrep=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --interval)
            interval="${2:?missing interval}"
            shift 2
            ;;
        --pid)
            mode=pid
            pid="${2:?missing pid}"
            shift 2
            ;;
        --pgrep)
            mode=pgrep
            pattern="${2:?missing pattern}"
            shift 2
            ;;
        --include-pgrep)
            include_pgrep+=("${2:?missing pattern}")
            shift 2
            ;;
        --)
            shift
            mode=command
            break
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            mode=command
            break
            ;;
    esac
done

if [[ -z "${mode}" ]]; then
    usage
    exit 2
fi

command_status=0
command_pid=
if [[ "${mode}" == "command" ]]; then
    if [[ $# -eq 0 ]]; then
        usage
        exit 2
    fi
    "$@" &
    command_pid=$!
fi

descendants() {
    local parent="$1"
    local child
    pgrep -P "${parent}" 2>/dev/null || true
    for child in $(pgrep -P "${parent}" 2>/dev/null || true); do
        descendants "${child}"
    done
}

current_pids() {
    case "${mode}" in
        pid)
            if ps -p "${pid}" >/dev/null 2>&1; then
                printf '%s\n' "${pid}"
            fi
            ;;
        pgrep)
            pgrep -f "${pattern}" 2>/dev/null || true
            ;;
        command)
            if ps -p "${command_pid}" >/dev/null 2>&1; then
                printf '%s\n' "${command_pid}"
                descendants "${command_pid}"
            fi
            ;;
    esac
    local include
    for include in "${include_pgrep[@]}"; do
        pgrep -f "${include}" 2>/dev/null || true
    done
}

sample() {
    local pids
    pids="$(current_pids | sort -n | uniq | paste -sd, -)"
    if [[ -z "${pids}" ]]; then
        return 1
    fi
    ps -o pid=,rss=,vsz=,nlwp=,etime=,comm= -p "${pids}" 2>/dev/null || true
}

peak_rss=0
final_rss=0
peak_line=
start_epoch="$(date +%s)"

while true; do
    rows="$(sample || true)"
    if [[ -n "${rows}" ]]; then
        total_rss="$(awk '{sum += $2} END {print sum + 0}' <<<"${rows}")"
        final_rss="${total_rss}"
        if (( total_rss > peak_rss )); then
            peak_rss="${total_rss}"
            peak_line="${rows}"
        fi
        printf '[%s] total_rss_kib=%s\n%s\n' "$(date --iso-8601=seconds)" "${total_rss}" "${rows}"
    fi

    if [[ "${mode}" == "command" ]]; then
        if ! kill -0 "${command_pid}" 2>/dev/null; then
            wait "${command_pid}" || command_status=$?
            break
        fi
    elif [[ -z "${rows}" ]]; then
        break
    fi

    sleep "${interval}"
done

elapsed_s="$(( $(date +%s) - start_epoch ))"
printf 'memory_smoke_summary elapsed_s=%s peak_rss_kib=%s final_rss_kib=%s\n' \
    "${elapsed_s}" "${peak_rss}" "${final_rss}"
if [[ -n "${peak_line}" ]]; then
    printf 'memory_smoke_peak_processes:\n%s\n' "${peak_line}"
fi

exit "${command_status}"
