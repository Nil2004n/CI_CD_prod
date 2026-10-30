//! P03-T06 tests: the child-environment sanitiser (`planning/P03-T01-request.md` §5.5, §7, §8 "T06").
//!
//! Expected variable sets are written out literally (names, values, origins, order) and are not derived
//! from the code under test. Every group says what would make it fail. All secrets are synthetic and
//! recognisably fake; no assertion message prints a value. The deny table itself is private and is
//! covered by unit tests inside `child_env.rs`; this file reaches it through `build`.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use application::child_env::{
    ChildEnv, EnvError, HostEnv, Origin, SecretProbe, ToolDirs, ToolProfile, build,
};

// ---------------------------------------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------------------------------------

/// The fake session credential.
const KEY: &str = "pwt-test-FAKE-0123456789abcdef";

/// Substring search over a fixed fake secret.
struct SubstringProbe(&'static [u8]);

impl SecretProbe for SubstringProbe {
    fn appears_in(&self, value: &[u8]) -> bool {
        contains(value, self.0)
    }
}

fn key_probe() -> SubstringProbe {
    SubstringProbe(KEY.as_bytes())
}

/// Byte-wise substring test. An empty needle never matches (it would match everything).
fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

fn os(bytes: &[u8]) -> OsString {
    OsString::from_vec(bytes.to_vec())
}

fn pair(name: &str, value: &str) -> (OsString, OsString) {
    (OsString::from(name), OsString::from(value))
}

/// The known tool directories used by the exact-set tests.
fn dirs() -> ToolDirs {
    ToolDirs::new(
        Path::new("/w/home"),
        Path::new("/w/config"),
        Path::new("/w/cache"),
        Path::new("/w/tmp"),
    )
    .expect("absolute directories")
}

fn build_ok(profile: ToolProfile, dirs: &ToolDirs, host: &HostEnv) -> ChildEnv {
    build(profile, dirs, host, &key_probe()).expect("profile builds")
}

/// `(name, value, origin)` in output order. Origins come from `preview()`, which must line up with `vars()`.
fn snapshot(env: &ChildEnv) -> Vec<(String, String, Origin)> {
    let preview = env.preview();
    let vars: Vec<_> = env.vars().collect();
    assert_eq!(
        preview.len(),
        vars.len(),
        "preview and vars differ in length"
    );
    vars.iter()
        .zip(&preview)
        .map(|((name, value), line)| {
            assert_eq!(line.name.as_str(), name.to_str().expect("ASCII name"));
            (
                name.to_str().expect("ASCII name").to_owned(),
                value.to_str().expect("UTF-8 value").to_owned(),
                line.origin,
            )
        })
        .collect()
}

fn t(name: &str, value: &str, origin: Origin) -> (String, String, Origin) {
    (name.to_owned(), value.to_owned(), origin)
}

/// Raw bytes of all output values, for byte searches.
fn value_bytes(env: &ChildEnv) -> Vec<Vec<u8>> {
    env.vars().map(|(_, v)| v.as_bytes().to_vec()).collect()
}

/// Every name and value of `env` is free of every forbidden byte string.
fn assert_free_of(env: &ChildEnv, forbidden: &[Vec<u8>], what: &str) {
    for (name, value) in env.vars() {
        for needle in forbidden.iter().filter(|n| !n.is_empty()) {
            assert!(
                !contains(value.as_bytes(), needle),
                "{what}: a forbidden value appears in the value of {}",
                name.to_string_lossy()
            );
            assert!(
                !contains(name.as_bytes(), needle),
                "{what}: a forbidden value appears in a name"
            );
        }
    }
}

/// The directories a profile points at, for the SecretValue table: indices into [home, config, cache, tmp].
fn used_dirs(profile: ToolProfile) -> &'static [usize] {
    match profile {
        ToolProfile::Git => &[0, 1],
        ToolProfile::Yamllint | ToolProfile::Checkov => &[0, 1, 2, 3],
        ToolProfile::Gitleaks | ToolProfile::Internal => &[0, 3],
        ToolProfile::Actionlint | ToolProfile::Kubeconform => &[0, 2, 3],
    }
}

/// The exact variable set of each profile for `dirs()` (request §7), in output order (ascending by name).
fn expected(profile: ToolProfile) -> Vec<(String, String, Origin)> {
    use Origin::{AppDir, Fixed};
    match profile {
        ToolProfile::Git => vec![
            t("GIT_ATTR_NOSYSTEM", "1", Fixed),
            t("GIT_CONFIG_GLOBAL", "/dev/null", Fixed),
            t("GIT_CONFIG_NOSYSTEM", "1", Fixed),
            t("GIT_TERMINAL_PROMPT", "0", Fixed),
            t("HOME", "/w/home", AppDir),
            t("LANG", "C.UTF-8", Fixed),
            t("PATH", "/usr/bin:/bin", Fixed),
            t("XDG_CONFIG_HOME", "/w/config", AppDir),
        ],
        ToolProfile::Yamllint => vec![
            t("HOME", "/w/home", AppDir),
            t("LANG", "C.UTF-8", Fixed),
            t("PATH", "/usr/bin:/bin", Fixed),
            t("TMPDIR", "/w/tmp", AppDir),
            t("XDG_CACHE_HOME", "/w/cache", AppDir),
            t("XDG_CONFIG_HOME", "/w/config", AppDir),
        ],
        ToolProfile::Gitleaks | ToolProfile::Internal => vec![
            t("HOME", "/w/home", AppDir),
            t("LANG", "C.UTF-8", Fixed),
            t("PATH", "/usr/bin:/bin", Fixed),
            t("TMPDIR", "/w/tmp", AppDir),
        ],
        ToolProfile::Checkov => vec![
            t("CKV_SKIP_PACKAGE_UPDATE_CHECK", "True", Fixed),
            t("HOME", "/w/home", AppDir),
            t("LANG", "C.UTF-8", Fixed),
            t("PATH", "/usr/bin:/bin", Fixed),
            t("TMPDIR", "/w/tmp", AppDir),
            t("XDG_CACHE_HOME", "/w/cache", AppDir),
            t("XDG_CONFIG_HOME", "/w/config", AppDir),
        ],
        ToolProfile::Actionlint | ToolProfile::Kubeconform => vec![
            t("HOME", "/w/home", AppDir),
            t("LANG", "C.UTF-8", Fixed),
            t("PATH", "/usr/bin:/bin", Fixed),
            t("TMPDIR", "/w/tmp", AppDir),
            t("XDG_CACHE_HOME", "/w/cache", AppDir),
        ],
    }
}

/// Names that look like credentials or that steer a child process (more than 20), each with a unique value.
const KEYLIKE_NAMES: [&str; 40] = [
    "OPENROUTER_API_KEY",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_ACCESS_KEY_ID",
    "AWS_SESSION_TOKEN",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "GITLAB_TOKEN",
    "CI_JOB_TOKEN",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "ALL_PROXY",
    "NO_PROXY",
    "GIT_ASKPASS",
    "SSH_ASKPASS",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_KEY_0",
    "GIT_CONFIG_VALUE_0",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "PYTHONPATH",
    "PYTHONSTARTUP",
    "SSH_AUTH_SOCK",
    "GPG_TTY",
    "DOCKER_HOST",
    "NPM_TOKEN",
    "PYPI_TOKEN",
    "BC_API_KEY",
    "PRISMA_API_KEY",
    "AZURE_CLIENT_SECRET",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GCP_PROJECT",
    "DB_PASSWORD",
    "SESSION_COOKIE",
    "openrouter_api_key",
    "Github_Token",
    "x-auth-header",
    "MY_SECRET",
    "PASSWD_FILE",
];

/// Names a profile sets itself; the host holds other values for them, which must lose.
const SHADOWED_NAMES: [&str; 12] = [
    "HOME",
    "PATH",
    "LANG",
    "TMPDIR",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_TERMINAL_PROMPT",
    "GIT_ATTR_NOSYSTEM",
    "CKV_SKIP_PACKAGE_UPDATE_CHECK",
    "LC_ALL",
];

/// The seeded host environment (no `TZ`) and every seeded value, key-bearing ones included.
fn seeded_host() -> (HostEnv, Vec<Vec<u8>>) {
    let mut pairs: Vec<(OsString, OsString)> = Vec::new();
    for (i, name) in KEYLIKE_NAMES.iter().enumerate() {
        pairs.push(pair(name, &format!("seedval-key-{i:02}")));
    }
    for (i, name) in SHADOWED_NAMES.iter().enumerate() {
        pairs.push(pair(name, &format!("seedval-shadow-{i:02}")));
    }
    pairs.extend([
        // Multi-line and `=`-containing values, an empty value, a name that holds `=`, an empty name.
        pair("MULTI", "line-one\nseedval-line-two\r\nline-three"),
        pair("EQ", "a=b=seedval-eq"),
        pair("EMPTYVAL", ""),
        pair("WEIRD=NAME", "seedval-weird"),
        pair("", "seedval-emptyname"),
        // Duplicates of an innocent name and of a name a profile sets.
        pair("DUP", "seedval-dup-first"),
        pair("DUP", "seedval-dup-second"),
        pair("HOME", "seedval-home-again"),
        // The session key under an innocent name, and inside a longer value.
        pair("FOO", KEY),
        pair("EMBED", &format!("prefix-{KEY}-suffix")),
        pair("LOOKS_FINE", &format!("seedval-{KEY}")),
        // NUL inside a value.
        (OsString::from("NULVAL"), os(b"a\0seedval-nul")),
        // Non-UTF-8 names and values.
        (os(b"BAD\xff\xfeNAME"), OsString::from("seedval-badname")),
        (OsString::from("BADVAL"), os(b"seedval-\xff\xfe-badval")),
        (os(b"\xc0\xaf"), os(b"seedval-\x80")),
    ]);
    let values = pairs.iter().map(|(_, v)| v.as_bytes().to_vec()).collect();
    (HostEnv::from_pairs(pairs), values)
}

// ---------------------------------------------------------------------------------------------------------
// Profiles.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn profile_names_round_trip() {
    // Would fail if `as_str`/`parse`/`ALL` disagreed or `parse` accepted another spelling.
    assert_eq!(ToolProfile::ALL.len(), 7);
    let names: Vec<&str> = ToolProfile::ALL.iter().map(|p| p.as_str()).collect();
    assert_eq!(
        names,
        [
            "git",
            "yamllint",
            "gitleaks",
            "checkov",
            "actionlint",
            "kubeconform",
            "internal"
        ]
    );
    for profile in ToolProfile::ALL {
        assert_eq!(ToolProfile::parse(profile.as_str()), Some(profile));
    }
    for bad in ["", "Git", "GIT", "git ", "scanner", "trivy"] {
        assert_eq!(ToolProfile::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn each_profile_has_the_exact_variable_set() {
    // Names, values, origins and order are literal. Would fail if a variable were added, dropped,
    // renamed, re-valued, given another origin, or if the output order changed.
    for profile in ToolProfile::ALL {
        let env = build_ok(profile, &dirs(), &HostEnv::empty());
        assert_eq!(snapshot(&env), expected(profile), "{profile:?}");
    }
}

#[test]
fn output_order_does_not_depend_on_host_order() {
    // Would fail if the order followed the host order (or a hash order) instead of ascending names.
    let (host, _) = seeded_host();
    let mut pairs: Vec<(OsString, OsString)> = host
        .iter()
        .map(|(k, v)| (k.to_os_string(), v.to_os_string()))
        .collect();
    pairs.push(pair("TZ", "UTC"));
    let forward = HostEnv::from_pairs(pairs.clone());
    pairs.reverse();
    let backward = HostEnv::from_pairs(pairs);
    for profile in ToolProfile::ALL {
        let a = snapshot(&build_ok(profile, &dirs(), &forward));
        let b = snapshot(&build_ok(profile, &dirs(), &backward));
        assert_eq!(a, b, "{profile:?}");
        let names: Vec<&String> = a.iter().map(|(n, _, _)| n).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names, sorted, "{profile:?}: ascending and unique");
    }
}

#[test]
fn app_dirs_come_from_tool_dirs_under_root() {
    // `ToolDirs::under` puts the four directories at root/tools/<profile>/{home,config,cache,tmp}.
    // Would fail if the layout, the profile segment or a variable-to-directory mapping changed.
    for profile in ToolProfile::ALL {
        let root = Path::new("/srv/pwt-data");
        let dirs = ToolDirs::under(root, profile).expect("valid root");
        let env = build_ok(profile, &dirs, &HostEnv::empty());
        let base = format!("/srv/pwt-data/tools/{}", profile.as_str());
        for (name, value, origin) in snapshot(&env) {
            let want = match name.as_str() {
                "HOME" => Some("home"),
                "XDG_CONFIG_HOME" => Some("config"),
                "XDG_CACHE_HOME" => Some("cache"),
                "TMPDIR" => Some("tmp"),
                _ => None,
            };
            match want {
                Some(dir) => {
                    assert_eq!(value, format!("{base}/{dir}"), "{profile:?} {name}");
                    assert_eq!(origin, Origin::AppDir);
                }
                None => assert_eq!(origin, Origin::Fixed, "{profile:?} {name}"),
            }
        }
    }
}

#[test]
fn tool_dirs_equal_the_explicit_form() {
    // Would fail if `under` built another path than the documented one.
    let under = ToolDirs::under(Path::new("/r"), ToolProfile::Git).expect("valid");
    let explicit = ToolDirs::new(
        Path::new("/r/tools/git/home"),
        Path::new("/r/tools/git/config"),
        Path::new("/r/tools/git/cache"),
        Path::new("/r/tools/git/tmp"),
    )
    .expect("valid");
    assert_eq!(under, explicit);
    let other = ToolDirs::under(Path::new("/r"), ToolProfile::Yamllint).expect("valid");
    assert_ne!(under, other);
}

// ---------------------------------------------------------------------------------------------------------
// Host environment.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn seeded_host_is_large_and_awkward() {
    // Guards the fixture: the packet asks for at least 20 key-like names, duplicates, non-UTF-8 names and
    // values, multi-line and `=` values and the key under innocent names. Would fail if the fixture shrank.
    let (host, values) = seeded_host();
    let entries: Vec<(&OsStr, &OsStr)> = host.iter().collect();
    assert!(KEYLIKE_NAMES.len() >= 20);
    assert!(entries.len() >= 60);
    assert!(entries.iter().any(|(k, _)| k.to_str().is_none()));
    assert!(entries.iter().any(|(_, v)| v.to_str().is_none()));
    assert!(entries.iter().any(|(_, v)| v.as_bytes().contains(&b'\n')));
    assert!(
        entries
            .iter()
            .any(|(_, v)| v.as_bytes().windows(2).any(|w| w == b"=b"))
    );
    assert!(
        entries
            .iter()
            .any(|(k, v)| *k == "FOO" && v.as_bytes() == KEY.as_bytes())
    );
    assert!(entries.iter().any(|(k, v)| *k == "EMBED"
        && v.as_bytes() != KEY.as_bytes()
        && contains(v.as_bytes(), KEY.as_bytes())));
    assert_eq!(entries.iter().filter(|(k, _)| *k == "DUP").count(), 2);
    assert!(entries.iter().any(|(k, _)| k.as_bytes().contains(&b'=')));
    assert!(values.iter().any(|v| v.as_slice() == KEY.as_bytes()));
}

#[test]
fn seeded_host_changes_nothing_for_any_profile() {
    // With a host full of credentials, proxies, loader variables and values for the very names a profile
    // sets, the output equals the output for an empty host, for every profile. Would fail if any host
    // entry (or host value for a name the profile sets) reached the output.
    let (host, _) = seeded_host();
    for profile in ToolProfile::ALL {
        let with = build_ok(profile, &dirs(), &host);
        let without = build_ok(profile, &dirs(), &HostEnv::empty());
        assert_eq!(snapshot(&with), snapshot(&without), "{profile:?}");
        assert_eq!(snapshot(&with), expected(profile), "{profile:?}");
    }
}

#[test]
fn no_seeded_value_or_key_appears_in_any_output() {
    // Byte search of every output name and value for every seeded value and for the key. Would fail if a
    // seeded value were inherited, appended, or embedded in a fixed or directory value.
    let (host, mut forbidden) = seeded_host();
    forbidden.push(KEY.as_bytes().to_vec());
    for profile in ToolProfile::ALL {
        let env = build_ok(profile, &dirs(), &host);
        assert_free_of(&env, &forbidden, &format!("{profile:?}"));
    }
}

#[test]
fn byte_search_control_can_fail() {
    // The sweeps above only mean something if the search finds a value that is present. Here `TZ` is
    // inherited by `internal`, and the same helpers must see it. Would fail if `contains` never matched.
    let host = HostEnv::from_pairs([pair("TZ", "seedval-tz-clean")]);
    let env = build_ok(ToolProfile::Internal, &dirs(), &host);
    let found = value_bytes(&env)
        .iter()
        .any(|v| contains(v, b"seedval-tz-clean"));
    assert!(found);
    let result = std::panic::catch_unwind(|| {
        assert_free_of(&env, &[b"seedval-tz-clean".to_vec()], "control");
    });
    assert!(
        result.is_err(),
        "assert_free_of must reject a present value"
    );
    assert!(contains(b"abcdef", b"cde"));
    assert!(!contains(b"abcdef", b"cdf"));
    assert!(!contains(b"abc", b""));
}

#[test]
fn clean_tz_is_inherited_by_internal_only() {
    // Would fail if a scanner profile inherited TZ, or if internal did not.
    for tz in [
        "Europe/Berlin",
        "UTC",
        "EST5EDT,M3.2.0,M11.1.0",
        "<+03>-3",
        "a=b",
    ] {
        let host = HostEnv::from_pairs([pair("TZ", tz), pair("HOME", "/host")]);
        for profile in ToolProfile::ALL {
            let got = snapshot(&build_ok(profile, &dirs(), &host));
            let mut want = expected(profile);
            if profile == ToolProfile::Internal {
                want.push(t("TZ", tz, Origin::Inherited));
            }
            // `Origin` is not `Ord`; sort by name only.
            let by_name = |v: &mut Vec<(String, String, Origin)>| v.sort_by(|a, b| a.0.cmp(&b.0));
            let mut got_sorted = got.clone();
            by_name(&mut got_sorted);
            let mut want_sorted = want;
            by_name(&mut want_sorted);
            assert_eq!(got_sorted, want_sorted, "{profile:?} {tz}");
            assert_eq!(
                got, got_sorted,
                "{profile:?}: output is already in name order"
            );
        }
    }
}

#[test]
fn tz_holding_the_key_is_dropped_everywhere() {
    // Would fail if the probe were not applied to inherited values (exact key, embedded key, key at the
    // start and at the end).
    let values = [
        KEY.to_owned(),
        format!("prefix-{KEY}-suffix"),
        format!("{KEY}/Europe"),
        format!("Europe/{KEY}"),
    ];
    for tz in &values {
        let host = HostEnv::from_pairs([pair("TZ", tz)]);
        for profile in ToolProfile::ALL {
            let env = build_ok(profile, &dirs(), &host);
            assert_eq!(
                snapshot(&env),
                expected(profile),
                "{profile:?}: TZ must be dropped"
            );
            assert_free_of(&env, &[KEY.as_bytes().to_vec()], &format!("{profile:?}"));
        }
    }
}

#[test]
fn only_a_tz_that_passes_is_inherited_and_the_first_occurrence_decides() {
    // Duplicates: the first TZ decides. If it passes it wins over a later one; if it fails the name is
    // dropped and a later clean duplicate is not used instead. Would fail if the last occurrence won, if
    // a later duplicate replaced a dropped one, or if a failing first value leaked.
    let internal = |pairs: Vec<(OsString, OsString)>| {
        snapshot(&build_ok(
            ToolProfile::Internal,
            &dirs(),
            &HostEnv::from_pairs(pairs),
        ))
    };
    let base = expected(ToolProfile::Internal);
    let with_tz = |tz: &str| {
        let mut v = base.clone();
        v.push(t("TZ", tz, Origin::Inherited));
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };

    assert_eq!(
        internal(vec![pair("TZ", "UTC"), pair("TZ", "Asia/Tokyo")]),
        with_tz("UTC")
    );
    assert_eq!(
        internal(vec![pair("TZ", "UTC"), pair("TZ", KEY)]),
        with_tz("UTC"),
        "a failing duplicate after a good first one changes nothing"
    );
    assert_eq!(internal(vec![pair("TZ", KEY), pair("TZ", "UTC")]), base);
    assert_eq!(
        internal(vec![(OsString::from("TZ"), os(b"a\0b")), pair("TZ", "UTC")]),
        base,
        "NUL in the first value drops the name"
    );
}

#[test]
fn tz_with_nul_or_near_miss_names_is_not_inherited() {
    // Would fail if NUL were passed on, or if a different name were matched loosely (case, suffix, `=`,
    // non-UTF-8).
    let base = expected(ToolProfile::Internal);
    let near: Vec<(OsString, OsString)> = vec![
        (OsString::from("TZ"), os(b"UTC\0")),
        pair("tz", "UTC"),
        pair("Tz", "UTC"),
        pair("TZDIR", "/usr/share/zoneinfo"),
        pair("TZ=UTC", "x"),
        pair("TZ ", "UTC"),
        pair(" TZ", "UTC"),
        pair("", "UTC"),
        (os(b"TZ\xff"), OsString::from("UTC")),
        (os(b"T\xffZ"), OsString::from("UTC")),
    ];
    for entry in near {
        let host = HostEnv::from_pairs([entry]);
        assert_eq!(
            snapshot(&build_ok(ToolProfile::Internal, &dirs(), &host)),
            base
        );
    }
}

#[test]
fn non_utf8_tz_name_does_not_stop_a_later_clean_tz() {
    // A dropped non-UTF-8 name is not a "first occurrence of TZ". Would fail if it consumed the name.
    let host = HostEnv::from_pairs([(os(b"TZ\xff"), OsString::from("x")), pair("TZ", "UTC")]);
    let got = snapshot(&build_ok(ToolProfile::Internal, &dirs(), &host));
    assert!(got.contains(&t("TZ", "UTC", Origin::Inherited)));
}

#[test]
fn host_debug_shows_only_the_entry_count() {
    // Would fail if `HostEnv`'s Debug printed a name or value.
    let (host, values) = seeded_host();
    let shown = format!("{host:?}");
    assert!(
        shown.starts_with("HostEnv(<") && shown.ends_with(" entries>)"),
        "{shown}"
    );
    for v in values.iter().filter(|v| v.len() > 3) {
        assert!(!contains(shown.as_bytes(), v));
    }
    assert_eq!(format!("{:?}", HostEnv::empty()), "HostEnv(<0 entries>)");
}

// ---------------------------------------------------------------------------------------------------------
// Directories and fixed values.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn key_in_a_used_directory_is_a_secret_value_error() {
    // For each profile and each of the four directories carrying the key: SecretValue if the profile sets
    // that directory, otherwise Ok, and then the key is nowhere in the output. Would fail if a directory
    // were not probed, or if an unused one were probed.
    let bad = format!("/w/{KEY}/dir");
    for profile in ToolProfile::ALL {
        for position in 0..4 {
            let paths: Vec<PathBuf> = (0..4)
                .map(|i| {
                    if i == position {
                        PathBuf::from(&bad)
                    } else {
                        PathBuf::from(format!("/w/ok{i}"))
                    }
                })
                .collect();
            let dirs = ToolDirs::new(&paths[0], &paths[1], &paths[2], &paths[3])
                .expect("the key does not make a path invalid");
            let result = build(profile, &dirs, &HostEnv::empty(), &key_probe());
            if used_dirs(profile).contains(&position) {
                assert_eq!(
                    result.err(),
                    Some(EnvError::SecretValue),
                    "{profile:?} dir {position}"
                );
            } else {
                let env = result.expect("an unused directory is not set, so it is not probed");
                assert_free_of(&env, &[KEY.as_bytes().to_vec()], &format!("{profile:?}"));
            }
        }
    }
}

#[test]
fn key_in_the_root_reaches_every_profile_through_under() {
    // Would fail if `under` dropped the root or the probe skipped derived paths.
    let root = PathBuf::from(format!("/srv/{KEY}"));
    for profile in ToolProfile::ALL {
        let dirs = ToolDirs::under(&root, profile).expect("valid root");
        assert_eq!(
            build(profile, &dirs, &HostEnv::empty(), &key_probe()).err(),
            Some(EnvError::SecretValue),
            "{profile:?}"
        );
    }
}

#[test]
fn key_in_a_non_utf8_directory_is_still_found() {
    // The probe sees raw path bytes. Would fail if paths were converted lossily before probing.
    let mut bytes = b"/w/\xff-".to_vec();
    bytes.extend_from_slice(KEY.as_bytes());
    let bad = PathBuf::from(OsString::from_vec(bytes));
    let dirs = ToolDirs::new(
        &bad,
        Path::new("/w/c"),
        Path::new("/w/a"),
        Path::new("/w/t"),
    )
    .expect("non-UTF-8 is allowed");
    assert_eq!(
        build(
            ToolProfile::Internal,
            &dirs,
            &HostEnv::empty(),
            &key_probe()
        )
        .err(),
        Some(EnvError::SecretValue)
    );
}

#[test]
fn non_utf8_directory_is_passed_through_byte_for_byte() {
    // Would fail if the path were lossily converted, quoted or normalised.
    let raw: &[u8] = b"/w/\xff\xfe/home";
    let dirs = ToolDirs::new(
        Path::new(OsStr::from_bytes(raw)),
        Path::new("/w/c"),
        Path::new("/w/a"),
        Path::new("/w/t"),
    )
    .expect("non-UTF-8 is allowed");
    let env = build_ok(ToolProfile::Internal, &dirs, &HostEnv::empty());
    let home = env.vars().find(|(k, _)| *k == "HOME").expect("HOME is set");
    assert_eq!(home.1.as_bytes(), raw);
}

#[test]
fn key_in_a_fixed_value_is_a_secret_value_error() {
    // A probe that matches a constant of the profile makes the build fail closed, for exactly the profiles
    // that set that constant. Would fail if fixed values were not probed or the wrong ones were.
    struct Case {
        probe: &'static [u8],
        failing: &'static [ToolProfile],
    }
    let all = &ToolProfile::ALL;
    let cases = [
        Case {
            probe: b"/usr/bin:/bin",
            failing: all,
        },
        Case {
            probe: b"C.UTF-8",
            failing: all,
        },
        Case {
            probe: b"True",
            failing: &[ToolProfile::Checkov],
        },
        Case {
            probe: b"/dev/null",
            failing: &[ToolProfile::Git],
        },
        Case {
            probe: b"1",
            failing: &[ToolProfile::Git],
        },
        Case {
            probe: b"0",
            failing: &[ToolProfile::Git],
        },
        Case {
            probe: b"pwt-never-present",
            failing: &[],
        },
    ];
    for case in cases {
        let probe = SubstringProbe(case.probe);
        for profile in ToolProfile::ALL {
            let result = build(profile, &dirs(), &HostEnv::empty(), &probe);
            if case.failing.contains(&profile) {
                assert_eq!(result.err(), Some(EnvError::SecretValue), "{profile:?}");
            } else {
                assert!(result.is_ok(), "{profile:?}");
            }
        }
    }
}

#[test]
fn tool_dirs_accept_and_reject_by_rule() {
    // Absolute paths without a `..` component are valid, whatever else they contain. Would fail if a rule
    // were dropped, widened to substring matching on `..`, or applied to only some of the four paths.
    let ok = [
        "/", "/a", "/a/b", "/a/..b", "/a/b..", "/a/.../b", "/a/./b", "//a", "/a b/c",
    ];
    let bad = [
        "",
        ".",
        "..",
        "a",
        "a/b",
        "./a",
        "../a",
        "~",
        "/a/..",
        "/..",
        "/../a",
        "/a/../b",
        "/a/b/..",
        "/a/b/../../c",
    ];
    for position in 0..4 {
        for path in ok {
            let mut p = ["/p0", "/p1", "/p2", "/p3"];
            p[position] = path;
            let result = ToolDirs::new(
                Path::new(p[0]),
                Path::new(p[1]),
                Path::new(p[2]),
                Path::new(p[3]),
            );
            assert!(result.is_ok(), "{path:?} at {position}");
        }
        for path in bad {
            let mut p = ["/p0", "/p1", "/p2", "/p3"];
            p[position] = path;
            let result = ToolDirs::new(
                Path::new(p[0]),
                Path::new(p[1]),
                Path::new(p[2]),
                Path::new(p[3]),
            );
            assert_eq!(
                result.err(),
                Some(EnvError::InvalidDir),
                "{path:?} at {position}"
            );
        }
    }
}

#[test]
fn tool_dirs_reject_nul_bytes() {
    // A NUL cannot be passed to a child. Would fail if NUL paths were accepted.
    let bad = PathBuf::from(os(b"/a\0b"));
    for position in 0..4 {
        let mut p: [&Path; 4] = [
            Path::new("/p0"),
            Path::new("/p1"),
            Path::new("/p2"),
            Path::new("/p3"),
        ];
        p[position] = &bad;
        assert_eq!(
            ToolDirs::new(p[0], p[1], p[2], p[3]).err(),
            Some(EnvError::InvalidDir)
        );
    }
}

#[test]
fn under_rejects_a_relative_or_parent_root() {
    // Would fail if `under` joined first and checked nothing, or checked only the joined result of a
    // different root.
    for profile in ToolProfile::ALL {
        for root in ["", "relative", "./x", "../x", "/a/../b", "/..", "/a/.."] {
            assert_eq!(
                ToolDirs::under(Path::new(root), profile).err(),
                Some(EnvError::InvalidDir),
                "{root:?} {profile:?}"
            );
        }
        assert!(ToolDirs::under(Path::new("/"), profile).is_ok());
        assert!(ToolDirs::under(Path::new("/srv/data"), profile).is_ok());
    }
}

// ---------------------------------------------------------------------------------------------------------
// Preview and Debug.
// ---------------------------------------------------------------------------------------------------------

#[test]
fn preview_has_names_and_origins_in_output_order() {
    // Would fail if the preview changed order, lost an entry, or mislabelled an origin.
    let host = HostEnv::from_pairs([pair("TZ", "Europe/Berlin")]);
    for profile in ToolProfile::ALL {
        let env = build_ok(profile, &dirs(), &host);
        let lines: Vec<(String, Origin)> = env
            .preview()
            .iter()
            .map(|l| (l.name.as_str().to_owned(), l.origin))
            .collect();
        let mut want: Vec<(String, Origin)> = expected(profile)
            .into_iter()
            .map(|(n, _, o)| (n, o))
            .collect();
        if profile == ToolProfile::Internal {
            want.push(("TZ".to_owned(), Origin::Inherited));
            want.sort_by(|a, b| a.0.cmp(&b.0));
        }
        assert_eq!(lines, want, "{profile:?}");
    }
}

#[test]
fn preview_and_debug_contain_no_value() {
    // Byte search of the preview (names as text) and of `{:?}` / `{:#?}` for every output value. Would
    // fail if a value were shown in either form. Values shorter than 2 bytes are pinned by the exact
    // Debug text in the next test, since a one-byte search would match names by accident.
    let host = HostEnv::from_pairs([pair("TZ", "Europe/Berlin")]);
    for profile in ToolProfile::ALL {
        let env = build_ok(profile, &dirs(), &host);
        let values: Vec<Vec<u8>> = value_bytes(&env)
            .into_iter()
            .filter(|v| v.len() >= 2)
            .collect();
        assert!(!values.is_empty());
        let mut shown = String::new();
        for line in env.preview() {
            shown.push_str(line.name.as_str());
            shown.push('\n');
        }
        shown.push_str(&format!("{env:?}\n{env:#?}\n"));
        for value in &values {
            assert!(
                !contains(shown.as_bytes(), value),
                "{profile:?}: a value appears in the preview or Debug text"
            );
        }
        assert!(
            contains(shown.as_bytes(), b"HOME"),
            "control: names are shown"
        );
    }
}

#[test]
fn debug_text_is_names_and_origins_only() {
    // Exact text for a profile with one-byte values (git) and one with an inherited entry (internal).
    // Would fail if the format changed or a value (`1`, `0`, a path) appeared.
    let git = build_ok(ToolProfile::Git, &dirs(), &HostEnv::empty());
    assert_eq!(
        format!("{git:?}"),
        "ChildEnv { vars: [\"GIT_ATTR_NOSYSTEM/Fixed\", \"GIT_CONFIG_GLOBAL/Fixed\", \
         \"GIT_CONFIG_NOSYSTEM/Fixed\", \"GIT_TERMINAL_PROMPT/Fixed\", \"HOME/AppDir\", \
         \"LANG/Fixed\", \"PATH/Fixed\", \"XDG_CONFIG_HOME/AppDir\"] }"
    );
    let host = HostEnv::from_pairs([pair("TZ", "Europe/Berlin")]);
    let internal = build_ok(ToolProfile::Internal, &dirs(), &host);
    assert_eq!(
        format!("{internal:?}"),
        "ChildEnv { vars: [\"HOME/AppDir\", \"LANG/Fixed\", \"PATH/Fixed\", \"TMPDIR/AppDir\", \
         \"TZ/Inherited\"] }"
    );
}

#[test]
fn env_error_debug_has_no_payload() {
    // `EnvError` is fieldless, so a failed build shows no value. Would fail if an error gained a payload
    // that echoed a value.
    let bad = format!("/w/{KEY}");
    let dirs = ToolDirs::new(
        Path::new(&bad),
        Path::new("/w/c"),
        Path::new("/w/a"),
        Path::new("/w/t"),
    )
    .expect("valid path");
    let err = build(
        ToolProfile::Internal,
        &dirs,
        &HostEnv::empty(),
        &key_probe(),
    )
    .expect_err("SecretValue");
    assert_eq!(format!("{err:?}"), "SecretValue");
    assert_eq!(format!("{:?}", EnvError::DeniedName), "DeniedName");
    assert_eq!(format!("{:?}", EnvError::InvalidDir), "InvalidDir");
}
