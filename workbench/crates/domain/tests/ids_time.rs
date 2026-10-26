//! P01-T05 tests: typed identifiers and UTC timestamps (`planning/P01-T01-request.md` §5.1, §5.2, §6.3).
//!
//! The `compile_fail` / positive-control doctest pair for distinct identifier types lives in the
//! module documentation of `domain::ids`.

use domain::ids::{
    ApprovalId, ArtifactId, CheckId, ChunkId, ConversationId, EventId, FileId, FindingId, IdError,
    MessageId, RepoId, ReportId, RequestId, RunId, SourceId, SuppressionId, TaskId, WorkflowId,
};
use domain::time::{TimeError, UtcTimestamp};

const VALID_ID: &str = "0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6b";
const NIL_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Generates the full identifier test suite for one type, in its own module.
macro_rules! id_tests {
    ($($module:ident => $ty:ident),+ $(,)?) => {$(
        mod $module {
            use super::*;

            #[test]
            fn valid_parse_and_display_round_trip() {
                let id = $ty::parse(VALID_ID).unwrap();
                assert_eq!(id.to_string(), VALID_ID);
                assert_eq!(id.as_bytes()[0], 0x01);
                assert_eq!(id.as_bytes()[15], 0x6b);
                assert_eq!($ty::parse(&id.to_string()).unwrap(), id);
            }

            #[test]
            fn all_digits_round_trip() {
                let s = "01234567-89ab-cdef-0123-456789abcdef";
                let id = $ty::parse(s).unwrap();
                assert_eq!(id.to_string(), s);
                assert_eq!(
                    id.as_bytes(),
                    &[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]
                );
            }

            #[test]
            fn debug_prints_type_and_canonical() {
                let id = $ty::parse(VALID_ID).unwrap();
                assert_eq!(format!("{id:?}"), format!("{}({})", stringify!($ty), VALID_ID));
            }

            #[test]
            fn serde_round_trip() {
                let id = $ty::parse(VALID_ID).unwrap();
                let json = serde_json::to_string(&id).unwrap();
                assert_eq!(json, format!("\"{VALID_ID}\""));
                let back: $ty = serde_json::from_str(&json).unwrap();
                assert_eq!(back, id);
            }

            #[test]
            fn deserialize_goes_through_parse() {
                for bad in [
                    format!("\"{NIL_ID}\""),
                    format!("\"{}\"", VALID_ID.to_uppercase()),
                    format!("\"{}\"", &VALID_ID[..35]),
                    "42".to_string(),
                    "null".to_string(),
                ] {
                    assert!(serde_json::from_str::<$ty>(&bad).is_err(), "accepted {bad}");
                }
            }

            #[test]
            fn rejects_nil() {
                assert_eq!($ty::parse(NIL_ID), Err(IdError::Nil));
            }

            #[test]
            fn rejects_uppercase() {
                assert_eq!($ty::parse(&VALID_ID.to_uppercase()), Err(IdError::Format));
                assert_eq!(
                    $ty::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5A6b"),
                    Err(IdError::Format)
                );
            }

            #[test]
            fn rejects_wrong_lengths() {
                assert_eq!($ty::parse(&VALID_ID[..35]), Err(IdError::Length { found: 35 }));
                assert_eq!(
                    $ty::parse(&format!("{VALID_ID}0")),
                    Err(IdError::Length { found: 37 })
                );
                assert_eq!($ty::parse(""), Err(IdError::Length { found: 0 }));
                // 32 hex digits without hyphens (simple form).
                assert_eq!(
                    $ty::parse("0190a3b45c6d7e8f9a0b1c2d3e4f5a6b"),
                    Err(IdError::Length { found: 32 })
                );
            }

            #[test]
            fn rejects_non_hex() {
                assert_eq!(
                    $ty::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6g"),
                    Err(IdError::Format)
                );
                assert_eq!(
                    $ty::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6 "),
                    Err(IdError::Format)
                );
            }

            #[test]
            fn rejects_multibyte_input_of_36_bytes() {
                // 34 ASCII bytes plus one 2-byte character = 36 bytes.
                let s = format!("{}é", &VALID_ID[..34]);
                assert_eq!(s.len(), 36);
                assert_eq!($ty::parse(&s), Err(IdError::Format));
            }

            #[test]
            fn rejects_misplaced_hyphens() {
                assert_eq!(
                    $ty::parse("0190a3b45-c6d-7e8f-9a0b-1c2d3e4f5a6b"),
                    Err(IdError::Format)
                );
                assert_eq!(
                    $ty::parse("0190a3b4-5c6d-7e8f-9a0b1-c2d3e4f5a6b"),
                    Err(IdError::Format)
                );
                // 36 bytes with a hyphen replaced by a digit.
                assert_eq!(
                    $ty::parse("0190a3b405c6d-7e8f-9a0b-1c2d3e4f5a6b"),
                    Err(IdError::Format)
                );
                // Hyphen where a digit belongs, hyphens otherwise correct (still 36 bytes).
                assert_eq!(
                    $ty::parse("0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a--"),
                    Err(IdError::Format)
                );
            }

            #[test]
            fn rejects_whitespace() {
                assert_eq!(
                    $ty::parse(&format!(" {VALID_ID}")),
                    Err(IdError::Length { found: 37 })
                );
                assert_eq!(
                    $ty::parse(&format!("{VALID_ID}\n")),
                    Err(IdError::Length { found: 37 })
                );
                assert_eq!(
                    $ty::parse(&format!("\t{VALID_ID}\r\n")),
                    Err(IdError::Length { found: 39 })
                );
                // Whitespace replacing a character keeps the length at 36.
                assert_eq!(
                    $ty::parse(&format!(" {}", &VALID_ID[1..])),
                    Err(IdError::Format)
                );
                assert_eq!(
                    $ty::parse(&format!("{} ", &VALID_ID[..35])),
                    Err(IdError::Format)
                );
            }

            #[test]
            fn rejects_braces_and_urn() {
                assert!($ty::parse(&format!("{{{VALID_ID}}}")).is_err());
                assert!($ty::parse(&format!("urn:uuid:{VALID_ID}")).is_err());
                assert_eq!(
                    $ty::parse(&format!("{{{VALID_ID}}}")),
                    Err(IdError::Length { found: 38 })
                );
                assert_eq!(
                    $ty::parse(&format!("urn:uuid:{VALID_ID}")),
                    Err(IdError::Length { found: 45 })
                );
            }

            #[test]
            fn version_and_variant_bits_are_not_enforced() {
                // Version nibble 0, variant nibble f: structurally fine, accepted by design.
                let s = "00000000-0000-0000-f000-000000000001";
                assert_eq!($ty::parse(s).unwrap().to_string(), s);
            }

            #[test]
            fn ordering_and_hash_follow_bytes() {
                let a = $ty::parse("00000000-0000-0000-0000-000000000001").unwrap();
                let b = $ty::parse("00000000-0000-0000-0000-000000000002").unwrap();
                assert!(a < b);
                let mut set = std::collections::HashSet::new();
                set.insert(a);
                set.insert(a);
                set.insert(b);
                assert_eq!(set.len(), 2);
            }
        }
    )+};
}

id_tests! {
    approval_id => ApprovalId,
    artifact_id => ArtifactId,
    check_id => CheckId,
    chunk_id => ChunkId,
    conversation_id => ConversationId,
    event_id => EventId,
    file_id => FileId,
    finding_id => FindingId,
    message_id => MessageId,
    repo_id => RepoId,
    report_id => ReportId,
    request_id => RequestId,
    run_id => RunId,
    source_id => SourceId,
    suppression_id => SuppressionId,
    task_id => TaskId,
    workflow_id => WorkflowId,
}

#[test]
fn id_error_display_never_contains_input() {
    let hostile = "SECRET-TOKEN-VALUE-0123456789-ABCDEF";
    assert_eq!(hostile.len(), 36);
    let inputs = [
        hostile.to_string(),
        "SECRET".to_string(),
        format!("{hostile}-and-more-secret"),
        VALID_ID.to_uppercase(),
        format!(" {VALID_ID}"),
        format!("{{{VALID_ID}}}"),
        NIL_ID.to_string(),
        "0190a3b4-5c6d-7e8f-9a0b-1c2d3e4f5a6g".to_string(),
        "0190a3b45c6d7e8f9a0b1c2d3e4f5a6b".to_string(),
    ];
    for input in inputs {
        let err = RunId::parse(&input).unwrap_err();
        let shown = err.to_string();
        assert!(!shown.is_empty());
        assert!(!shown.contains(&input), "Display echoed input: {shown}");
        for chunk in ["SECRET", "0190a3b4", "5c6d", "1c2d3e4f5a6", "00000000"] {
            assert!(!shown.contains(chunk), "Display leaked {chunk}: {shown}");
        }
        assert!(!format!("{err:?}").contains("SECRET"));
    }
    assert!(!IdError::Format.to_string().is_empty());
    assert!(!IdError::Nil.to_string().is_empty());
    let boxed: Box<dyn std::error::Error> = Box::new(IdError::Nil);
    assert!(!boxed.to_string().is_empty());
}

#[test]
fn id_deserialize_error_does_not_echo_input() {
    let err =
        serde_json::from_str::<RunId>("\"SECRET-TOKEN-VALUE-0123456789-ABCDEF\"").unwrap_err();
    assert!(!err.to_string().contains("SECRET"));
}

#[test]
fn distinct_id_types_share_bytes_but_are_separate_types() {
    // Runtime counterpart of the compile_fail doctest in `domain::ids`.
    let run = RunId::parse(VALID_ID).unwrap();
    let repo = RepoId::parse(VALID_ID).unwrap();
    assert_eq!(run.as_bytes(), repo.as_bytes());
    assert_ne!(
        std::any::TypeId::of::<RunId>(),
        std::any::TypeId::of::<RepoId>()
    );
}

// ---------------------------------------------------------------------------------------------
// Timestamps
// ---------------------------------------------------------------------------------------------

/// Test helper (not a `#[test]` fn, so the clippy.toml test allowance does not apply to it).
#[allow(clippy::unwrap_used)]
fn ts(s: &str) -> UtcTimestamp {
    UtcTimestamp::parse_rfc3339(s).unwrap()
}

#[test]
fn epoch() {
    let t = ts("1970-01-01T00:00:00Z");
    assert_eq!(t.unix_seconds(), 0);
    assert_eq!(t.subsec_nanos(), 0);
    assert_eq!(t.to_rfc3339(), "1970-01-01T00:00:00Z");
    assert_eq!(UtcTimestamp::from_unix(0, 0).unwrap(), t);
}

#[test]
fn minimum_instant() {
    let t = ts("0001-01-01T00:00:00Z");
    assert_eq!(t.unix_seconds(), -62_135_596_800);
    assert_eq!(t.subsec_nanos(), 0);
    assert_eq!(t.to_rfc3339(), "0001-01-01T00:00:00Z");
}

#[test]
fn maximum_instant() {
    let s = "9999-12-31T23:59:59.999999999Z";
    let t = ts(s);
    assert_eq!(t.unix_seconds(), 253_402_300_799);
    assert_eq!(t.subsec_nanos(), 999_999_999);
    assert_eq!(t.to_rfc3339(), s);
}

#[test]
fn known_instants() {
    assert_eq!(ts("2000-01-01T00:00:00Z").unix_seconds(), 946_684_800);
    assert_eq!(ts("2024-02-29T12:34:56Z").unix_seconds(), 1_709_210_096);
    assert_eq!(ts("2038-01-19T03:14:08Z").unix_seconds(), 2_147_483_648);
    assert_eq!(ts("1969-12-31T23:59:59Z").unix_seconds(), -1);
    assert_eq!(ts("1900-03-01T00:00:00Z").unix_seconds(), -2_203_891_200);
}

#[test]
fn leap_day_valid_and_invalid() {
    assert_eq!(
        ts("2024-02-29T00:00:00Z").to_rfc3339(),
        "2024-02-29T00:00:00Z"
    );
    assert_eq!(
        ts("2000-02-29T00:00:00Z").to_rfc3339(),
        "2000-02-29T00:00:00Z"
    );
    assert_eq!(
        UtcTimestamp::parse_rfc3339("2023-02-29T00:00:00Z"),
        Err(TimeError::Range)
    );
    assert_eq!(
        UtcTimestamp::parse_rfc3339("1900-02-29T00:00:00Z"),
        Err(TimeError::Range)
    );
    assert_eq!(
        UtcTimestamp::parse_rfc3339("2100-02-29T00:00:00Z"),
        Err(TimeError::Range)
    );
    assert_eq!(
        UtcTimestamp::parse_rfc3339("2024-02-30T00:00:00Z"),
        Err(TimeError::Range)
    );
}

#[test]
fn day_of_month_limits() {
    assert!(UtcTimestamp::parse_rfc3339("2023-01-31T00:00:00Z").is_ok());
    assert_eq!(
        UtcTimestamp::parse_rfc3339("2023-04-31T00:00:00Z"),
        Err(TimeError::Range)
    );
    assert!(UtcTimestamp::parse_rfc3339("2023-04-30T00:00:00Z").is_ok());
    assert_eq!(
        UtcTimestamp::parse_rfc3339("2023-06-31T00:00:00Z"),
        Err(TimeError::Range)
    );
    assert_eq!(
        UtcTimestamp::parse_rfc3339("2023-01-00T00:00:00Z"),
        Err(TimeError::Range)
    );
}

#[test]
fn rejects_out_of_range_fields() {
    for s in [
        "0000-01-01T00:00:00Z",
        "2023-00-10T00:00:00Z",
        "2023-13-10T00:00:00Z",
        "2023-01-32T00:00:00Z",
        "2023-01-10T24:00:00Z",
        "2023-01-10T00:60:00Z",
        "2023-01-10T23:59:60Z", // leap second
        "2023-12-31T23:59:60.5Z",
    ] {
        assert_eq!(UtcTimestamp::parse_rfc3339(s), Err(TimeError::Range), "{s}");
    }
}

#[test]
fn rejects_offsets_and_wrong_case() {
    for s in [
        "2023-01-10T00:00:00+00:00",
        "2023-01-10T00:00:00-05:00",
        "2023-01-10T00:00:00+0000",
        "2023-01-10T00:00:00",
        "2023-01-10T00:00:00z",
        "2023-01-10t00:00:00Z",
        "2023-01-10 00:00:00Z",
        "2023-01-10T00:00:00.5+01:00",
        "2023-01-10T00:00:00ZZ",
        "2023-01-10T00:00:00Z+",
    ] {
        assert_eq!(
            UtcTimestamp::parse_rfc3339(s),
            Err(TimeError::Format),
            "{s}"
        );
    }
}

#[test]
fn rejects_whitespace() {
    for s in [
        " 2023-01-10T00:00:00Z",
        "2023-01-10T00:00:00Z ",
        "2023-01-10T00:00:00Z\n",
        "\t2023-01-10T00:00:00Z",
        "2023-01-10T 0:00:00Z",
        "2023-01-10T00:00:00. 5Z",
    ] {
        assert_eq!(
            UtcTimestamp::parse_rfc3339(s),
            Err(TimeError::Format),
            "{s:?}"
        );
    }
}

#[test]
fn rejects_bad_shape() {
    for s in [
        "",
        "Z",
        "2023",
        "2023-01-10",
        "2023-01-10T00:00Z",
        "2023-1-10T00:00:00Z",
        "23-01-10T00:00:00Z",
        "02023-01-10T00:00:00Z",
        "+2023-01-10T00:00:00Z",
        "2023/01/10T00:00:00Z",
        "2023-01-10T00-00-00Z",
        "2023-01-10T0a:00:00Z",
        "２０２３-01-10T00:00:00Z",
        "2023-01-10T00:00:00é",
        "2023-01-10T00:00:00.Z",
        "2023-01-10T00:00:00.abcZ",
        "2023-01-10T00:00:00,5Z",
    ] {
        assert_eq!(
            UtcTimestamp::parse_rfc3339(s),
            Err(TimeError::Format),
            "{s:?}"
        );
    }
}

#[test]
fn fraction_digit_limits() {
    for n in 1..=9usize {
        let s = format!("2023-01-10T00:00:00.{}Z", "1".repeat(n));
        let t = ts(&s);
        assert_eq!(
            t.subsec_nanos(),
            "111111111"[..n].parse::<u32>().unwrap() * 10u32.pow(9 - n as u32)
        );
    }
    assert_eq!(
        UtcTimestamp::parse_rfc3339("2023-01-10T00:00:00.1234567890Z"),
        Err(TimeError::Format)
    );
    assert_eq!(
        UtcTimestamp::parse_rfc3339("2023-01-10T00:00:00.000000000000Z"),
        Err(TimeError::Format)
    );
}

#[test]
fn fraction_normalisation() {
    assert_eq!(
        ts("2023-01-10T00:00:00.500Z").to_rfc3339(),
        "2023-01-10T00:00:00.5Z"
    );
    assert_eq!(
        ts("2023-01-10T00:00:00.5Z").to_rfc3339(),
        "2023-01-10T00:00:00.5Z"
    );
    assert_eq!(
        ts("2023-01-10T00:00:00.000Z").to_rfc3339(),
        "2023-01-10T00:00:00Z"
    );
    assert_eq!(
        ts("2023-01-10T00:00:00.0Z").to_rfc3339(),
        "2023-01-10T00:00:00Z"
    );
    assert_eq!(
        ts("2023-01-10T00:00:00.050Z").to_rfc3339(),
        "2023-01-10T00:00:00.05Z"
    );
    assert_eq!(
        ts("2023-01-10T00:00:00.000000001Z").to_rfc3339(),
        "2023-01-10T00:00:00.000000001Z"
    );
    assert_eq!(
        ts("2023-01-10T00:00:00.123456789Z").to_rfc3339(),
        "2023-01-10T00:00:00.123456789Z"
    );
    assert_eq!(ts("2023-01-10T00:00:00.500Z"), ts("2023-01-10T00:00:00.5Z"));
}

#[test]
fn parse_then_format_is_identity_on_canonical_strings() {
    for s in [
        "1970-01-01T00:00:00Z",
        "0001-01-01T00:00:00Z",
        "0001-01-01T00:00:00.000000001Z",
        "9999-12-31T23:59:59.999999999Z",
        "2024-02-29T23:59:59.25Z",
        "1999-12-31T23:59:59Z",
        "2023-07-04T09:08:07.000123Z",
    ] {
        assert_eq!(ts(s).to_rfc3339(), s);
    }
}

#[test]
fn ordering() {
    let a = ts("2023-01-10T00:00:00Z");
    let b = ts("2023-01-10T00:00:00.000000001Z");
    let c = ts("2023-01-10T00:00:01Z");
    let before_epoch = ts("1969-12-31T23:59:59.9Z");
    assert!(a < b && b < c);
    assert!(before_epoch < ts("1970-01-01T00:00:00Z"));
    assert!(ts("0001-01-01T00:00:00Z") < before_epoch);
    assert!(c < ts("9999-12-31T23:59:59.999999999Z"));
    assert_eq!(a.max(c), c);
}

#[test]
fn from_unix_range_edges() {
    const MIN: i64 = -62_135_596_800;
    const MAX: i64 = 253_402_300_799;
    assert!(UtcTimestamp::from_unix(MIN, 0).is_ok());
    assert!(UtcTimestamp::from_unix(MAX, 999_999_999).is_ok());
    assert_eq!(UtcTimestamp::from_unix(MIN - 1, 0), Err(TimeError::Range));
    assert_eq!(
        UtcTimestamp::from_unix(MIN - 1, 999_999_999),
        Err(TimeError::Range)
    );
    assert_eq!(UtcTimestamp::from_unix(MAX + 1, 0), Err(TimeError::Range));
    assert_eq!(UtcTimestamp::from_unix(i64::MIN, 0), Err(TimeError::Range));
    assert_eq!(UtcTimestamp::from_unix(i64::MAX, 0), Err(TimeError::Range));
    assert_eq!(
        UtcTimestamp::from_unix(0, 1_000_000_000),
        Err(TimeError::Range)
    );
    assert_eq!(UtcTimestamp::from_unix(0, u32::MAX), Err(TimeError::Range));
    assert!(UtcTimestamp::from_unix(0, 999_999_999).is_ok());
    assert_eq!(
        UtcTimestamp::from_unix(MIN, 0).unwrap().to_rfc3339(),
        "0001-01-01T00:00:00Z"
    );
    assert_eq!(
        UtcTimestamp::from_unix(MAX, 999_999_999)
            .unwrap()
            .to_rfc3339(),
        "9999-12-31T23:59:59.999999999Z"
    );
}

#[test]
fn accessors() {
    let t = UtcTimestamp::from_unix(1_700_000_000, 123_000_000).unwrap();
    assert_eq!(t.unix_seconds(), 1_700_000_000);
    assert_eq!(t.subsec_nanos(), 123_000_000);
    assert_eq!(t.to_rfc3339(), "2023-11-14T22:13:20.123Z");
    assert_eq!(t.to_string(), "2023-11-14T22:13:20.123Z");
    assert_eq!(format!("{t:?}"), "UtcTimestamp(2023-11-14T22:13:20.123Z)");
}

#[test]
fn serde_round_trip() {
    for s in [
        "1970-01-01T00:00:00Z",
        "0001-01-01T00:00:00Z",
        "9999-12-31T23:59:59.999999999Z",
        "2024-02-29T01:02:03.4Z",
    ] {
        let t = ts(s);
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, format!("\"{s}\""));
        let back: UtcTimestamp = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }
    // Deserialisation normalises non-canonical but accepted input.
    let t: UtcTimestamp = serde_json::from_str("\"2023-01-10T00:00:00.500Z\"").unwrap();
    assert_eq!(
        serde_json::to_string(&t).unwrap(),
        "\"2023-01-10T00:00:00.5Z\""
    );
    for bad in [
        "\"2023-02-29T00:00:00Z\"",
        "\"2023-01-10T00:00:00+00:00\"",
        "\"\"",
        "17",
        "null",
    ] {
        assert!(serde_json::from_str::<UtcTimestamp>(bad).is_err(), "{bad}");
    }
}

#[test]
fn time_error_display_never_contains_input() {
    let inputs = [
        "SECRET-not-a-timestamp",
        "2023-02-29T00:00:00Z",
        "2023-13-45T99:99:99Z",
        "2023-01-10T00:00:00+00:00",
        " 2023-01-10T00:00:00Z ",
        "2023-01-10T00:00:00.1234567890Z",
        "SECRETSECRETSECRETSECRETSECRETSECRET",
    ];
    for input in inputs {
        let err = UtcTimestamp::parse_rfc3339(input).unwrap_err();
        let shown = err.to_string();
        assert!(!shown.is_empty());
        assert!(
            !shown.contains(input.trim()),
            "Display echoed input: {shown}"
        );
        assert!(!shown.contains("SECRET"));
        assert!(!shown.contains("2023"));
        assert!(!format!("{err:?}").contains("SECRET"));
    }
    assert_ne!(TimeError::Format.to_string(), TimeError::Range.to_string());
    let boxed: Box<dyn std::error::Error> = Box::new(TimeError::Range);
    assert!(!boxed.to_string().is_empty());
}

#[test]
fn now_is_within_supported_range_and_close_to_system_clock() {
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let now = UtcTimestamp::now();
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    assert!(now.unix_seconds() >= before && now.unix_seconds() <= after);
    assert!(now.subsec_nanos() < 1_000_000_000);
    assert_eq!(UtcTimestamp::parse_rfc3339(&now.to_rfc3339()).unwrap(), now);
}

/// Deterministic xorshift64 generator for the seeded round-trip test.
struct XorShift64(u64);

impl XorShift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

#[test]
fn seeded_from_unix_round_trips() {
    const MIN: i64 = -62_135_596_800;
    const MAX: i64 = 253_402_300_799;
    let span = (MAX - MIN + 1) as u64;
    let mut rng = XorShift64(0x9E37_79B9_7F4A_7C15);
    let mut count = 0u32;
    while count < 20_000 {
        let secs = MIN + (rng.next_u64() % span) as i64;
        // Mix whole seconds, short fractions and full nanosecond precision.
        let nanos = match rng.next_u64() % 4 {
            0 => 0,
            1 => ((rng.next_u64() % 1000) * 1_000_000) as u32,
            2 => (rng.next_u64() % 1_000_000_000) as u32,
            _ => 999_999_999,
        };
        let t = UtcTimestamp::from_unix(secs, nanos).unwrap();
        let text = t.to_rfc3339();
        let back = UtcTimestamp::parse_rfc3339(&text).unwrap();
        assert_eq!(back, t, "{text}");
        assert_eq!(back.unix_seconds(), secs);
        assert_eq!(back.subsec_nanos(), nanos);
        assert_eq!(back.to_rfc3339(), text);
        count += 1;
    }
    assert!(count >= 10_000);
}

#[test]
fn every_day_boundary_round_trips_across_all_years() {
    // Walk the first second of every day from 0001 to 9999 at a coarse stride to hit all
    // month/leap boundaries, comparing against an independent day-count.
    const MIN: i64 = -62_135_596_800;
    const MAX: i64 = 253_402_300_799;
    let mut secs = MIN;
    while secs <= MAX {
        let t = UtcTimestamp::from_unix(secs, 0).unwrap();
        assert_eq!(UtcTimestamp::parse_rfc3339(&t.to_rfc3339()).unwrap(), t);
        secs += 86_400 * 7 + 1;
    }
    // Every date of a few leap/non-leap boundary years, formatted and re-parsed day by day.
    for year in [1900i64, 2000, 2023, 2024, 2100, 9999] {
        let start = ts(&format!("{year:04}-01-01T00:00:00Z")).unix_seconds();
        let end_exclusive = start + if is_leap(year) { 366 } else { 365 } * 86_400;
        let mut s = start;
        while s < end_exclusive {
            let text = UtcTimestamp::from_unix(s, 0).unwrap().to_rfc3339();
            assert!(text.starts_with(&format!("{year:04}-")), "{text}");
            s += 86_400;
        }
        let last = UtcTimestamp::from_unix(end_exclusive - 1, 0)
            .unwrap()
            .to_rfc3339();
        assert_eq!(last, format!("{year:04}-12-31T23:59:59Z"));
    }
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
