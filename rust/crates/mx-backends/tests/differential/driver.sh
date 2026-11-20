#!/usr/bin/env bash
# One backend function, run the way lib/run.sh runs it: the backend file is
# sourced, `set -uo pipefail` is on, PATH has $HOME/.local/bin and
# $HOME/.cargo/bin in front, and backend_run starts in $REPO_PATH.
#
# The differential test (tests/differential.rs) calls this once per operation.
#
# Usage: driver.sh OP BACKEND_FILE [PROMPT MODEL LOG EXTRA RC_FILE]
#   OP is declare, wall, check or run. RC_FILE receives the exit status of
#   backend_run (a file, so the standard output of the run stays the agent's).
set -uo pipefail
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

op="$1"
file="$2"
prompt="${3:-}"
model="${4:-}"
log="${5:-}"
extra="${6:-}"
rc_file="${7:-}"

# shellcheck source=/dev/null
. "$file" || exit 97

case "$op" in
    declare)
        printf 'name=%s\n' "$(backend_name)"
        if declare -F backend_rehearsal >/dev/null; then echo "rehearsal=yes"; else echo "rehearsal=no"; fi
        if declare -F backend_allowed_tasks >/dev/null; then
            printf 'allowed=%s\n' "$(backend_allowed_tasks)"
        else
            echo "allowed=any"
        fi
        ;;
    wall)
        if declare -F backend_rehearsal_wall >/dev/null; then
            backend_rehearsal_wall
            printf 'rc=%s\n' "$?"
        else
            echo "rc=none"
        fi
        printf 'MAINTAINER_SETTINGS=%s\n' "${MAINTAINER_SETTINGS-<unset>}"
        printf 'MAINTAINER_CURSOR_DIR=%s\n' "${MAINTAINER_CURSOR_DIR-<unset>}"
        ;;
    check)
        msg="$(backend_check)"
        printf 'rc=%s\n' "$?"
        printf 'msg=%s\n' "$msg"
        ;;
    run)
        cd "$REPO_PATH" || exit 98
        backend_run "$prompt" "$model" "$log" "$extra"
        printf '%s\n' "$?" >"$rc_file"
        ;;
    *)
        echo "driver.sh: unknown operation $op" >&2
        exit 99
        ;;
esac
