#!/bin/bash
# A stand-in for claude, codex, cursor-agent, opencode and python3. The test copies
# this file under each program name and replaces @CFG@ with the directory that
# holds the call log and the behaviour files.
#
# One record per call goes to $CFG/calls.log, written after the standard input
# is read and before any output is written, so two stubs of one pipeline log
# in a fixed order: the reader of a pipe only logs once the writer has exited.
#   argv (one element per line, shell-quoted), the working directory, the
#   backend-relevant environment names (a key is logged as its length and
#   sha256), the length and sha256 of standard input.
# Behaviour comes from files beside the log: NAME.rc (exit status), NAME.out
# and NAME.err (what to print). The default for the python3 stub, which stands
# for scripts/transcript.py, is to print nothing.
CFG='@CFG@'
name="${0##*/}"
tmp="$CFG/.in.$$"
cat >"$tmp"
sha="$(sha256sum <"$tmp")"
sha="${sha%% *}"
len="$(wc -c <"$tmp")"
rm -f "$tmp"
{
    printf 'CALL %s\n' "$name"
    printf 'ARGC %s\n' "$#"
    for a in "$@"; do printf 'ARG %q\n' "$a"; done
    printf 'CWD %s\n' "$(pwd -P)"
    for v in MAX_THINKING_TOKENS CLAUDE_CODE_SUBAGENT_MODEL CURSOR_CONFIG_DIR OPENCODE_CONFIG \
        MAINTAINER_SETTINGS MAINTAINER_CURSOR_DIR; do
        if [ -n "${!v+x}" ]; then printf 'ENV %s=%s\n' "$v" "${!v}"; else printf 'ENV %s=<unset>\n' "$v"; fi
    done
    if [ -n "${GROQ_API_KEY+x}" ]; then
        k="$(printf '%s' "$GROQ_API_KEY" | sha256sum)"
        printf 'ENV GROQ_API_KEY=len:%s sha:%s\n' "${#GROQ_API_KEY}" "${k%% *}"
    else
        printf 'ENV GROQ_API_KEY=<unset>\n'
    fi
    printf 'STDIN %s %s\n' "$len" "$sha"
} >>"$CFG/calls.log"

if [ "$name" = python3 ]; then
    # What scripts/transcript.py does with its two arguments: append to the log
    # it was given and to the commands file.
    printf '[filter read %s bytes, sha256 %s]\n' "$len" "$sha" >>"$2"
    printf 'commands for %s\n' "$sha" >>"$3"
fi

if [ -f "$CFG/$name.out" ]; then
    cat "$CFG/$name.out"
elif [ "$name" != python3 ]; then
    printf '%s-out\n' "$name"
fi
if [ -f "$CFG/$name.err" ]; then
    cat "$CFG/$name.err" >&2
elif [ "$name" != python3 ]; then
    printf '%s-err\n' "$name" >&2
fi
rc=0
if [ -f "$CFG/$name.rc" ]; then read -r rc <"$CFG/$name.rc"; fi
exit "$rc"
