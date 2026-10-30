//! P03-T05 tests: destination normalisation (`planning/P03-T01-request.md` §6 "Destination normalisation").
//!
//! Expected canonical forms and error variants are written out literally; none is derived from the code
//! under test. Each test says what would make it fail. All hosts are documentation names (`example.*`) or
//! private IPv4 addresses.

use application::permission::{
    DestinationError, MAX_DESTINATION_BYTES, NormalisedDestination, Scheme,
};
use domain::store::values::Destination;

/// `(input, canonical, store form)`.
const ACCEPTED: &[(&str, &str, &str)] = &[
    // Case folding: host lower-cased, scheme normalised, path case kept.
    (
        "https://example.com",
        "https://example.com:443",
        "https://example.com:443",
    ),
    (
        "HTTPS://Example.COM",
        "https://example.com:443",
        "https://example.com:443",
    ),
    (
        "https://EXAMPLE.com:8443/Path/CaSe",
        "https://example.com:8443/Path/CaSe",
        "https://example.com:8443/Path/CaSe",
    ),
    // Trailing dot: exactly one is removed.
    (
        "https://example.com.",
        "https://example.com:443",
        "https://example.com:443",
    ),
    (
        "https://example.com.:8443",
        "https://example.com:8443",
        "https://example.com:8443",
    ),
    // Default ports become explicit; an explicit default stays.
    (
        "ssh://example.org",
        "ssh://example.org:22",
        "example.org:22",
    ),
    (
        "ssh://example.org:22",
        "ssh://example.org:22",
        "example.org:22",
    ),
    (
        "https://example.com:443",
        "https://example.com:443",
        "https://example.com:443",
    ),
    (
        "ssh://git.example.org:2222",
        "ssh://git.example.org:2222",
        "git.example.org:2222",
    ),
    // Port bounds.
    (
        "https://example.com:1",
        "https://example.com:1",
        "https://example.com:1",
    ),
    (
        "https://example.com:65535",
        "https://example.com:65535",
        "https://example.com:65535",
    ),
    (
        "https://example.com:0443",
        "https://example.com:443",
        "https://example.com:443",
    ),
    // Resource path: bare slash dropped, rest kept byte for byte.
    (
        "https://example.com/",
        "https://example.com:443",
        "https://example.com:443",
    ),
    (
        "https://example.com/a/b/",
        "https://example.com:443/a/b/",
        "https://example.com:443/a/b/",
    ),
    (
        "https://example.com/a//b%20c",
        "https://example.com:443/a//b%20c",
        "https://example.com:443/a//b%20c",
    ),
    (
        "https://example.com/a@b",
        "https://example.com:443/a@b",
        "https://example.com:443/a@b",
    ),
    // ssh keeps the path in the canonical form only; the store grammar has no ssh path.
    (
        "ssh://example.org/team/repo.git",
        "ssh://example.org:22/team/repo.git",
        "example.org:22",
    ),
    // Canonical dotted-quad IPv4.
    (
        "https://10.0.0.1",
        "https://10.0.0.1:443",
        "https://10.0.0.1:443",
    ),
    (
        "ssh://192.168.1.254:2200",
        "ssh://192.168.1.254:2200",
        "192.168.1.254:2200",
    ),
    (
        "https://0.0.0.0",
        "https://0.0.0.0:443",
        "https://0.0.0.0:443",
    ),
    // Ordinary host shapes.
    (
        "https://localhost",
        "https://localhost:443",
        "https://localhost:443",
    ),
    (
        "https://a-b.example.com",
        "https://a-b.example.com:443",
        "https://a-b.example.com:443",
    ),
    (
        "https://1abc.example",
        "https://1abc.example:443",
        "https://1abc.example:443",
    ),
    (
        "https://x1.2y.example",
        "https://x1.2y.example:443",
        "https://x1.2y.example:443",
    ),
];

/// `(input, expected error)`.
const REJECTED: &[(&str, DestinationError)] = &[
    // Scheme required, https and ssh only.
    ("", DestinationError::Scheme),
    ("example.com", DestinationError::Scheme),
    ("example.com:443", DestinationError::Scheme),
    ("//example.com", DestinationError::Scheme),
    ("://example.com", DestinationError::Scheme),
    ("https:/example.com", DestinationError::Scheme),
    ("http://example.com", DestinationError::Scheme),
    ("ftp://example.com", DestinationError::Scheme),
    ("git+ssh://example.com", DestinationError::Scheme),
    ("ssh+git://example.com", DestinationError::Scheme),
    ("file:///etc/passwd", DestinationError::Scheme),
    // Userinfo.
    ("https://user@example.com", DestinationError::Userinfo),
    ("https://user:pw@example.com", DestinationError::Userinfo),
    ("https://@example.com", DestinationError::Userinfo),
    (
        "https://example.com@evil.example",
        DestinationError::Userinfo,
    ),
    ("ssh://git@example.org", DestinationError::Userinfo),
    // Query and fragment, also with an empty value and after a path.
    ("https://example.com?x=1", DestinationError::Query),
    ("https://example.com?", DestinationError::Query),
    ("https://example.com/p?x=1", DestinationError::Query),
    ("https://example.com#f", DestinationError::Query),
    ("https://example.com/p#f", DestinationError::Query),
    ("ssh://example.org/r?x", DestinationError::Query),
    // Wildcard.
    ("https://*.example.com", DestinationError::Wildcard),
    ("https://ex*mple.com", DestinationError::Wildcard),
    ("https://*", DestinationError::Wildcard),
    ("ssh://*.example.org:22", DestinationError::Wildcard),
    // Punycode, in any label and any case.
    ("https://xn--bcher-kva.example", DestinationError::Punycode),
    ("https://XN--bcher-kva.example", DestinationError::Punycode),
    ("https://a.xn--p1ai", DestinationError::Punycode),
    ("https://www.xn--zca.example.", DestinationError::Punycode),
    // Non-ASCII host.
    ("https://b\u{fc}cher.example", DestinationError::NonAscii),
    ("https://\u{4f8b}\u{3048}.jp", DestinationError::NonAscii),
    ("https://example.com\u{a0}", DestinationError::NonAscii),
    ("https://\u{ff45}xample.com", DestinationError::NonAscii),
    // IPv6 literals and unbracketed IPv6.
    ("https://[::1]", DestinationError::IpLiteral),
    ("https://[::1]:443", DestinationError::IpLiteral),
    ("https://[2001:db8::1]/x", DestinationError::IpLiteral),
    ("ssh://[fe80::1]:22", DestinationError::IpLiteral),
    ("https://::1", DestinationError::IpLiteral),
    ("https://example.com:80:80", DestinationError::IpLiteral),
    // IPv4 shortcuts and non-canonical forms.
    ("https://127.1", DestinationError::IpLiteral),
    ("https://0x7f.0.0.1", DestinationError::IpLiteral),
    ("https://0x7f000001", DestinationError::IpLiteral),
    ("https://2130706433", DestinationError::IpLiteral),
    ("https://017.0.0.1", DestinationError::IpLiteral),
    ("https://127.0.0.01", DestinationError::IpLiteral),
    ("https://127.000.000.001", DestinationError::IpLiteral),
    ("https://127.0.0.1.5", DestinationError::IpLiteral),
    ("https://1.2.3", DestinationError::IpLiteral),
    ("https://256.1.1.1", DestinationError::IpLiteral),
    ("https://example.123", DestinationError::IpLiteral),
    // Ports.
    ("https://example.com:0", DestinationError::Port),
    ("https://example.com:00000", DestinationError::Port),
    ("https://example.com:65536", DestinationError::Port),
    ("https://example.com:99999", DestinationError::Port),
    ("https://example.com:100000", DestinationError::Port),
    ("https://example.com:000443", DestinationError::Port),
    ("https://example.com:abc", DestinationError::Port),
    ("https://example.com:", DestinationError::Port),
    ("https://example.com:-1", DestinationError::Port),
    ("https://example.com:+80", DestinationError::Port),
    ("https://example.com:0x50", DestinationError::Port),
    ("https://example.com:44 3", DestinationError::Syntax),
    ("ssh://example.org:65536", DestinationError::Port),
    // Syntax: authority and host.
    ("https://", DestinationError::Syntax),
    ("https:///path", DestinationError::Syntax),
    ("https://:443", DestinationError::Syntax),
    ("https://.", DestinationError::Syntax),
    ("https://example.com..", DestinationError::Syntax),
    ("https://example..com", DestinationError::Syntax),
    ("https://.example.com", DestinationError::Syntax),
    ("https://-bad.example", DestinationError::Syntax),
    ("https://bad-.example", DestinationError::Syntax),
    ("https://exa_mple.com", DestinationError::Syntax),
    ("https://exa mple.com", DestinationError::Syntax),
    ("https://exa\tmple.com", DestinationError::Syntax),
    ("https://example.com\\evil", DestinationError::Syntax),
    ("https://exa%6dple.com", DestinationError::Syntax),
    // Syntax: resource path.
    ("https://example.com/a b", DestinationError::Syntax),
    ("https://example.com/a\tb", DestinationError::Syntax),
    ("https://example.com/\u{e9}", DestinationError::Syntax),
    ("https://example.com/a\\b", DestinationError::Syntax),
    ("https://example.com/%zz", DestinationError::Syntax),
    ("https://example.com/%4", DestinationError::Syntax),
    ("https://example.com/%", DestinationError::Syntax),
    ("https://example.com/%2f", DestinationError::Syntax),
    ("https://example.com/%2F", DestinationError::Syntax),
    ("https://example.com/%5c", DestinationError::Syntax),
    ("https://example.com/%00", DestinationError::Syntax),
    ("https://example.com/%0a", DestinationError::Syntax),
    ("https://example.com/%7f", DestinationError::Syntax),
    ("https://example.com/%3f", DestinationError::Syntax),
    ("https://example.com/%23", DestinationError::Syntax),
    ("https://example.com/%40", DestinationError::Syntax),
    ("ssh://example.org/a b", DestinationError::Syntax),
];

fn parse(input: &str) -> Result<NormalisedDestination, DestinationError> {
    NormalisedDestination::parse(input)
}

/// Fails if an accepted input gives another canonical or store form, or a rejected input another error:
/// changing the case folding, the trailing-dot rule, a default port, the path rules or the error mapping
/// changes a row.
#[test]
fn accepted_inputs_normalise_to_the_pinned_forms() {
    assert!(ACCEPTED.len() >= 20);
    for (input, canonical, store) in ACCEPTED {
        let parsed = parse(input).unwrap_or_else(|e| panic!("{input:?} rejected: {e:?}"));
        assert_eq!(parsed.canonical(), *canonical, "canonical of {input:?}");
        assert_eq!(
            parsed.to_store().as_str(),
            *store,
            "store form of {input:?}"
        );
    }
}

/// Fails if a rejected input is accepted or reported as a less specific variant.
#[test]
fn rejected_inputs_give_the_most_specific_error() {
    assert!(REJECTED.len() >= 90);
    for (input, expected) in REJECTED {
        assert_eq!(
            parse(input).map(|d| d.canonical()),
            Err(*expected),
            "input {input:?}"
        );
    }
}

/// Fails if the table above covers fewer than all ten error variants (a variant that could not be produced
/// would be dead).
#[test]
fn every_error_variant_is_produced_by_some_input() {
    let all = [
        DestinationError::Syntax,
        DestinationError::Scheme,
        DestinationError::Userinfo,
        DestinationError::NonAscii,
        DestinationError::Punycode,
        DestinationError::Wildcard,
        DestinationError::IpLiteral,
        DestinationError::Port,
        DestinationError::Query,
        DestinationError::TooLong,
    ];
    let long = format!("https://example.com/{}", "a".repeat(MAX_DESTINATION_BYTES));
    for variant in all {
        let produced = REJECTED.iter().any(|(_, e)| *e == variant) || parse(&long) == Err(variant);
        assert!(produced, "{variant:?} is never produced");
    }
}

/// Fails if a lower-case-only or one-dot-only host rule is relaxed: exactly one trailing dot goes, two are
/// an empty label.
#[test]
fn exactly_one_trailing_dot_is_removed() {
    assert_eq!(
        parse("https://example.com.").map(|d| d.host().to_owned()),
        Ok("example.com".to_owned())
    );
    assert_eq!(
        parse("https://example.com.."),
        Err(DestinationError::Syntax)
    );
    assert_eq!(parse("https://."), Err(DestinationError::Syntax));
}

/// Fails if the label bound moves: 63 bytes are fine, 64 are not; the same for an interior and the last
/// label.
#[test]
fn label_length_bound_is_63_bytes() {
    let ok = "a".repeat(63);
    let bad = "a".repeat(64);
    assert!(parse(&format!("https://{ok}.example")).is_ok());
    assert!(parse(&format!("https://example.{ok}")).is_ok());
    assert_eq!(
        parse(&format!("https://{bad}.example")),
        Err(DestinationError::Syntax)
    );
    assert_eq!(
        parse(&format!("https://example.{bad}")),
        Err(DestinationError::Syntax)
    );
}

/// Fails if the host bound moves: 253 bytes are fine (also with a trailing dot, 254 bytes of input), 254
/// are not.
#[test]
fn host_length_bound_is_253_bytes() {
    let l63 = "a".repeat(63);
    let host253 = format!("{l63}.{l63}.{l63}.{}", "b".repeat(61));
    assert_eq!(host253.len(), 253);
    let parsed = parse(&format!("https://{host253}")).expect("253-byte host");
    assert_eq!(parsed.host(), host253);
    assert!(
        parse(&format!("https://{host253}.")).is_ok(),
        "trailing dot is not counted"
    );
    let host254 = format!("{l63}.{l63}.{l63}.{}", "b".repeat(62));
    assert_eq!(host254.len(), 254);
    assert_eq!(
        parse(&format!("https://{host254}")),
        Err(DestinationError::Syntax)
    );
}

/// Fails if the input bound is not MAX_DESTINATION_BYTES, if TooLong is not reported first, or if a
/// within-bound input whose store form is too long is accepted.
#[test]
fn over_length_input_is_too_long_and_first() {
    assert_eq!(MAX_DESTINATION_BYTES, 2048);
    let prefix = "https://example.com/";
    let at_bound = format!(
        "{prefix}{}",
        "a".repeat(MAX_DESTINATION_BYTES - prefix.len())
    );
    assert_eq!(at_bound.len(), MAX_DESTINATION_BYTES);
    // The explicit default port adds ":443", so the store form exceeds the P02 Destination bound.
    assert_eq!(parse(&at_bound), Err(DestinationError::TooLong));
    // With an explicit port the input has the same length as the store form and is accepted.
    let with_port = format!(
        "https://example.com:443/{}",
        "a".repeat(MAX_DESTINATION_BYTES - "https://example.com:443/".len())
    );
    assert_eq!(with_port.len(), MAX_DESTINATION_BYTES);
    assert!(parse(&with_port).is_ok());
    let over = format!("{prefix}{}", "a".repeat(MAX_DESTINATION_BYTES));
    assert_eq!(parse(&over), Err(DestinationError::TooLong));
    // TooLong wins over every other fault.
    let bad_scheme = format!("http://u@h?*{}", "a".repeat(MAX_DESTINATION_BYTES));
    assert_eq!(parse(&bad_scheme), Err(DestinationError::TooLong));
}

/// Fails if the documented precedence of rejection reasons changes (request §6 and the comment on
/// `NormalisedDestination::parse`).
#[test]
fn rejection_precedence_is_pinned() {
    let cases: &[(&str, DestinationError)] = &[
        // Scheme before Userinfo/Query/everything else.
        ("ftp://u@h?x*", DestinationError::Scheme),
        // Userinfo before Query.
        ("https://u@h?x", DestinationError::Userinfo),
        // Query before the host checks.
        ("https://*.xn--a?x", DestinationError::Query),
        ("https://b\u{fc}.example/p#f", DestinationError::Query),
        // NonAscii before Wildcard, Wildcard before IpLiteral, IpLiteral (bracket) before Punycode.
        ("https://b\u{fc}*.example", DestinationError::NonAscii),
        ("https://*.[::1]", DestinationError::Wildcard),
        ("https://[::1].xn--a", DestinationError::IpLiteral),
        // Punycode before the structural host checks and before IPv4.
        ("https://xn--a..b", DestinationError::Punycode),
        ("https://xn--a.127.1", DestinationError::Punycode),
        // Host faults before port faults.
        ("https://127.1:0", DestinationError::IpLiteral),
        ("https://a..b:0", DestinationError::Syntax),
        // Port faults before path faults.
        ("https://example.com:0/a b", DestinationError::Port),
    ];
    for (input, expected) in cases {
        assert_eq!(
            parse(input).map(|d| d.canonical()),
            Err(*expected),
            "input {input:?}"
        );
    }
}

/// Fails if an accessor returns a field other than the normalised one.
#[test]
fn accessors_expose_the_normalised_parts() {
    let https = parse("HTTPS://Example.COM./Repo%20x").expect("https");
    assert_eq!(https.scheme(), Scheme::Https);
    assert_eq!(https.host(), "example.com");
    assert_eq!(https.port(), 443);
    assert_eq!(https.resource(), Some("/Repo%20x"));
    let ssh = parse("ssh://example.org").expect("ssh");
    assert_eq!(ssh.scheme(), Scheme::Ssh);
    assert_eq!(ssh.port(), 22);
    assert_eq!(ssh.resource(), None);
    assert_eq!(
        parse("https://example.com/").expect("slash").resource(),
        None
    );
}

/// Fails if the canonical form is not a fixed point of `parse` (a different canonical form would give a
/// different fingerprint for the same destination), or differs between https and ssh inputs.
#[test]
fn canonical_round_trips_for_https_and_ssh() {
    for (input, canonical, _) in ACCEPTED {
        let first = parse(input).expect("accepted");
        let again = parse(canonical).expect("canonical form parses");
        assert_eq!(first, again, "round trip of {input:?}");
        assert_eq!(again.canonical(), *canonical);
    }
    assert_ne!(
        parse("https://example.com:22").expect("https").canonical(),
        parse("ssh://example.com:22").expect("ssh").canonical()
    );
}

/// Fails if `to_store` produces a value the P02 `Destination` grammar rejects or changes on re-parsing, or
/// if the https store form drops the scheme or the ssh one gains one.
#[test]
fn store_form_is_accepted_by_the_p02_destination_parser() {
    for (input, _, store) in ACCEPTED {
        let dest = parse(input).expect("accepted").to_store();
        let reparsed = Destination::parse(dest.as_str()).expect("store form is valid");
        assert_eq!(reparsed, dest, "re-parse of {input:?}");
        assert_eq!(dest.as_str(), *store);
        assert_eq!(
            dest.as_str().starts_with("https://"),
            input.to_ascii_lowercase().starts_with("https://")
        );
        assert!(!dest.as_str().contains("ssh://"));
    }
}

/// Fails if two spellings of one destination do not compare equal (the evaluator compares set members),
/// or if distinct destinations compare equal.
#[test]
fn equality_follows_the_normalised_form() {
    assert_eq!(
        parse("https://Example.com.").expect("a"),
        parse("HTTPS://example.com:443/").expect("b")
    );
    assert_ne!(
        parse("https://example.com").expect("a"),
        parse("https://example.com/x").expect("b")
    );
    assert_ne!(
        parse("https://example.com").expect("a"),
        parse("ssh://example.com:443").expect("b")
    );
    assert_ne!(
        parse("ssh://example.org/a").expect("a"),
        parse("ssh://example.org/b").expect("b")
    );
    // The ssh store form drops the path, but the values differ, so the fingerprint can tell them apart.
    assert_eq!(
        parse("ssh://example.org/a").expect("a").to_store(),
        parse("ssh://example.org/b").expect("b").to_store()
    );
}

/// Fails if `parse` panics on odd input (NUL, lone separators, a very long run, multi-byte boundaries).
#[test]
fn odd_inputs_are_rejected_without_panicking() {
    let odd = [
        "\u{0}",
        "https://\u{0}",
        "https://example.com/\u{0}",
        "https://[",
        "https://]",
        "https://:",
        "https://::",
        "https:////",
        "https://%",
        "://",
        "ssh://",
        "https://example.com/%e9%",
        "https://\u{1f600}",
        "https://example.com/\u{1f600}",
    ];
    for input in odd {
        assert!(parse(input).is_err(), "input {input:?}");
    }
    assert!(parse(&"/".repeat(3000)).is_err());
}
