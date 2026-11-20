//! `backend_by_name`: the four names and nothing else (plan decision D18).

use mx_backends::backend_by_name;

#[test]
fn the_four_shipped_names_find_a_backend_that_reports_the_same_name() {
    let mut found = 0;
    for name in ["claude", "codex", "cursor", "opencode"] {
        let backend = backend_by_name(name).unwrap_or_else(|| panic!("no backend {name}"));
        assert_eq!(backend.name(), name);
        found += 1;
    }
    assert_eq!(found, 4);
}

#[test]
fn every_other_string_finds_no_backend() {
    let refused = [
        "",
        " ",
        "Claude",
        "CLAUDE",
        "claude ",
        " claude",
        "claud",
        "claude2",
        "clau",
        "c",
        "co",
        "cod",
        "codex.sh",
        "claude.sh",
        "../x",
        "..",
        "../backends/codex",
        "claude/../codex",
        "claude/",
        "/claude",
        "./claude",
        "backends/claude",
        "claude\n",
        "\nclaude",
        "claude\ncodex",
        "claude\0",
        "claude\0codex",
        "cursor-agent",
        "open",
        "opencode\n",
        "opencode/..",
        "nosuch",
        "all",
        "*",
        "claude*",
        "c*",
        "[c]laude",
    ];
    let mut checked = 0;
    for name in refused {
        assert!(backend_by_name(name).is_none(), "{name:?} must not name a backend");
        checked += 1;
    }
    assert_eq!(checked, 37, "every refused name must be tried");
}

#[test]
fn a_name_that_starts_like_a_backend_is_not_that_backend() {
    for name in ["claudex", "codexx", "cursors", "opencodes", "claudecode", "codex-cli"] {
        assert!(backend_by_name(name).is_none(), "{name} matched by prefix");
    }
}

#[test]
fn a_name_that_ends_like_a_backend_is_not_that_backend() {
    for name in ["xclaude", "mycodex", "-cursor", "reopencode"] {
        assert!(backend_by_name(name).is_none(), "{name} matched by suffix");
    }
}
