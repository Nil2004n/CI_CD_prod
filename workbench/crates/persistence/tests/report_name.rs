//! P02-T10 tests for report stems (Data Schema §13, request §6.4): exact format, whole-second UTC
//! resolution, distinctness by run ID, and the length and character bounds that keep every generated file
//! name (and the temp names derived from it) inside `paths::MAX_REPORT_NAME_BYTES`.
//!
//! The no-overwrite behaviour of reports is tested with the writer in `tests/writer.rs`.

#![allow(
    clippy::expect_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-expect-in-tests"
)]

use domain::ids::RunId;
use domain::store::values::Slug;
use domain::time::UtcTimestamp;
use persistence::atomic::is_temp_file_name;
use persistence::paths::{MAX_REPORT_NAME_BYTES, StorePath};
use persistence::report_name::{ReportStem, report_stem};

fn at(text: &str) -> UtcTimestamp {
    UtcTimestamp::parse_rfc3339(text).expect("fixture timestamp")
}

fn slug(text: &str) -> Slug {
    Slug::parse(text).expect("fixture slug")
}

fn run(n: u32) -> RunId {
    RunId::parse(&format!("00000000-0000-4000-8000-{n:012x}")).expect("fixture run id")
}

#[test]
fn the_stem_has_the_documented_shape() {
    let stem = report_stem(at("2026-10-16T05:25:00Z"), &slug("demo-repo"), run(20));
    // Fails on any change of order, separator, case or stamp layout.
    assert_eq!(
        stem.as_str(),
        "20261016T052500Z_demo-repo_00000000-0000-4000-8000-000000000014"
    );
    assert_eq!(stem.to_string(), stem.as_str(), "Display and as_str agree");
    let copy: ReportStem = stem.clone();
    assert_eq!(copy, stem);
}

#[test]
fn the_stamp_is_utc_whole_seconds_truncated_not_rounded() {
    let s = slug("a");
    let r = run(1);
    // The fraction is dropped, never rounded up into the next second (or minute, or year).
    assert_eq!(
        report_stem(at("2026-12-31T23:59:59.999999999Z"), &s, r)
            .as_str()
            .get(..16),
        Some("20261231T235959Z")
    );
    assert_eq!(
        report_stem(at("2026-10-16T05:25:00.5Z"), &s, r),
        report_stem(at("2026-10-16T05:25:00Z"), &s, r),
        "two instants in the same second give the same stem"
    );
    assert_ne!(
        report_stem(at("2026-10-16T05:25:01Z"), &s, r),
        report_stem(at("2026-10-16T05:25:00Z"), &s, r)
    );
}

#[test]
fn the_stamp_is_fixed_width_across_the_supported_range() {
    let s = slug("a");
    let r = run(1);
    for (text, stamp) in [
        ("0001-01-01T00:00:00Z", "00010101T000000Z"),
        ("1969-12-31T23:59:59Z", "19691231T235959Z"),
        ("1970-01-01T00:00:00Z", "19700101T000000Z"),
        ("2024-02-29T12:34:56Z", "20240229T123456Z"),
        ("9999-12-31T23:59:59.999999999Z", "99991231T235959Z"),
    ] {
        let stem = report_stem(at(text), &s, r);
        assert_eq!(stem.as_str().get(..16), Some(stamp), "{text}");
        assert_eq!(stem.as_str().get(16..17), Some("_"), "{text}");
    }
}

/// Splitting the stamp back into a timestamp reproduces the input truncated to its second, for instants
/// spread over the whole supported range (a property check against the domain's own calendar code, not a
/// copy of the formatting code).
#[test]
fn the_stamp_round_trips_through_the_calendar_for_many_instants() {
    const MIN_SECS: i64 = -62_135_596_800;
    // Number of whole seconds from 0001-01-01T00:00:00Z to 9999-12-31T23:59:59Z inclusive.
    const SPAN: u64 = 253_402_300_799 + 62_135_596_800 + 1;
    let s = slug("a");
    let r = run(1);
    // xorshift64: deterministic, no dependency.
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    for _ in 0..5000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let offset = i64::try_from(state % (SPAN - 1)).expect("fits");
        let secs = MIN_SECS + offset;
        let nanos = u32::try_from(state % 1_000_000_000).expect("fits");
        let instant = UtcTimestamp::from_unix(secs, nanos).expect("in range");
        let stem = report_stem(instant, &s, r);
        let stamp = stem.as_str().get(..16).expect("16-byte stamp");
        let rebuilt = format!(
            "{}-{}-{}T{}:{}:{}Z",
            &stamp[0..4],
            &stamp[4..6],
            &stamp[6..8],
            &stamp[9..11],
            &stamp[11..13],
            &stamp[13..15]
        );
        assert_eq!(stamp.as_bytes().get(15), Some(&b'Z'));
        assert_eq!(
            at(&rebuilt).unix_seconds(),
            secs,
            "{secs}.{nanos} -> {stamp}"
        );
    }
}

#[test]
fn same_second_same_repo_differ_by_run_id_and_an_exact_repeat_is_equal() {
    let t = at("2026-10-16T05:25:00Z");
    let s = slug("demo-repo");
    let a = report_stem(t, &s, run(1));
    let b = report_stem(t, &s, run(2));
    assert_ne!(a, b, "fails if the run ID is left out of the stem");
    assert_ne!(a.as_str(), b.as_str());
    // An exact repeat gives the same stem; the writer then refuses it (tests/writer.rs).
    assert_eq!(a, report_stem(t, &s, run(1)));
    // A different repository in the same second also differs.
    assert_ne!(a, report_stem(t, &slug("other-repo"), run(1)));
}

#[test]
fn the_longest_stem_fits_every_name_the_store_derives_from_it() {
    let longest = slug(&"a".repeat(48));
    let stem = report_stem(at("2026-10-16T05:25:00Z"), &longest, run(1));
    assert_eq!(stem.as_str().len(), 16 + 1 + 48 + 1 + 36);
    for ext in ["json", "md", "pdf"] {
        let name = format!("{stem}.{ext}");
        assert!(name.len() <= MAX_REPORT_NAME_BYTES, "{name}");
        // Accepted as a backup/ path, and so are the temp names the atomic writer derives from it.
        let path = StorePath::report(&name).expect("generated names are valid report names");
        assert_eq!(path.as_str(), format!("backup/{name}"));
        let temp = format!(".{name}.prev.tmp-0123456789abcdef");
        assert!(is_temp_file_name(&temp));
        assert!(temp.len() <= 255, "{} bytes exceed NAME_MAX", temp.len());
    }
}

#[test]
fn slug_input_is_bounded_and_ascii_so_the_stem_cannot_grow_or_change_shape() {
    assert!(Slug::parse(&"a".repeat(48)).is_ok());
    assert!(
        Slug::parse(&"a".repeat(49)).is_err(),
        "fails if the slug bound is lifted"
    );
    for hostile in [
        "répo",
        "日本語",
        "ｒｅｐｏ",
        "repo\u{202e}",
        "repo/../x",
        "repo name",
        "Repo",
        "repo_name",
        "-repo",
        "repo-",
        "re--po",
        "",
    ] {
        assert!(
            Slug::parse(hostile).is_err(),
            "{hostile:?} must not become part of a file name"
        );
    }
    // `_` separates the stem's parts and is not a slug character, so the three parts stay unambiguous.
    let stem = report_stem(at("2026-10-16T05:25:00Z"), &slug("a-b-c"), run(7));
    assert_eq!(stem.as_str().split('_').count(), 3);
}

#[test]
fn every_stem_is_a_valid_report_file_name_without_a_leading_dot() {
    for text in ["demo-repo", "a", "x1-y2-z3", &"z".repeat(48)] {
        for t in [
            "0001-01-01T00:00:00Z",
            "2026-10-16T05:25:00Z",
            "9999-12-31T23:59:59Z",
        ] {
            let stem = report_stem(at(t), &slug(text), run(300));
            assert!(
                stem.as_str()
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_')),
                "{stem}"
            );
            assert!(StorePath::report(&format!("{stem}.json")).is_ok(), "{stem}");
            assert!(!stem.as_str().starts_with('.'));
        }
    }
}
