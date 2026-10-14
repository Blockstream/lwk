#!/usr/bin/env bash
#
# Run a docker-dependent CI step and tell infrastructure failures apart from
# real ones.
#
# Usage: ci_docker_guard.sh [--attempts N] <command> [args...]
#
# The command is run up to N times (default 1). If it still fails and the
# docker daemon (the docker:dind service) is unreachable, exit with
# INFRA_EXIT_CODE so that GitLab retries the job through `retry:exit_codes`.
# Otherwise exit with the command's own exit code, so real failures are not
# retried.
#
# The dind service dies when the runner node is being shut down; GitLab then
# reports the job as `script_failure`, which `retry:when` cannot match.

INFRA_EXIT_CODE=75

attempts=1
if [ "$1" = "--attempts" ]; then
    attempts="$2"
    shift 2
fi

if [ "$#" -eq 0 ]; then
    echo "usage: $0 [--attempts N] <command> [args...]" >&2
    exit 2
fi

for i in $(seq 1 "$attempts"); do
    "$@"
    code=$?
    if [ "$code" -eq 0 ]; then
        exit 0
    fi
    echo "ci_docker_guard: '$*' failed with exit code $code (attempt $i/$attempts)" >&2
    if ! docker info >/dev/null 2>&1; then
        echo "ci_docker_guard: docker daemon unreachable, exiting with $INFRA_EXIT_CODE to trigger a job retry" >&2
        exit "$INFRA_EXIT_CODE"
    fi
    if [ "$i" -lt "$attempts" ]; then
        sleep 5
    fi
done

exit "$code"
