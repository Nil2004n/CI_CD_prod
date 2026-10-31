//! P03-T07 tests: the session credential store, `read_key` and KEY-01 hardening
//! (`planning/P03-T01-request.md` §1 KEY-01 rows, §5.6, §8 T07).
//!
//! Expected values are written out literally and are not derived from the code under test. Every test
//! says what would make it fail. All keys are synthetic (`pwt-test-FAKE-...` or runs of one letter) and no
//! assertion message prints a key. Clearing of the read buffer is tested inside `credential.rs`
//! (`#[cfg(test)] mod tests`, through a test-only hook), because the buffer is private.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "shared helpers outside #[test] fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::cell::Cell;
use std::io::{Cursor, Error, ErrorKind, Read};

use application::child_env::SecretProbe;
use application::credential::{
    CredentialStore, KeyFormatError, MAX_KEY_BYTES, MIN_KEY_BYTES, SessionCredentialStore, read_key,
};
use domain::contract::{AppEvent, CredentialSecret, CredentialStatus};

const FAKE_KEY: &str = "pwt-test-FAKE-key-0123456789";

fn secret(text: &str) -> CredentialSecret {
    CredentialSecret::new(text.to_owned())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// `read_key` on `bytes` from a pipe; the key text on success.
fn read(bytes: &[u8]) -> Result<String, KeyFormatError> {
    read_key(&mut Cursor::new(bytes.to_vec()), false).map(|s| s.expose().to_owned())
}

fn letters(len: usize) -> Vec<u8> {
    vec![b'K'; len]
}

fn with_suffix(len: usize, suffix: &[u8]) -> Vec<u8> {
    let mut v = letters(len);
    v.extend_from_slice(suffix);
    v
}

// ---------------------------------------------------------------------------------------------------------
// The store.
// ---------------------------------------------------------------------------------------------------------

/// Fails if a new store reports anything but `absent`, or `set` reports anything but `session_only`.
#[test]
fn new_store_is_absent_and_set_makes_it_session_only() {
    let mut store = SessionCredentialStore::new();
    assert_eq!(store.status(), CredentialStatus::Absent);
    assert_eq!(
        SessionCredentialStore::default().status(),
        CredentialStatus::Absent
    );
    assert_eq!(store.set(secret(FAKE_KEY)), CredentialStatus::SessionOnly);
    assert_eq!(store.status(), CredentialStatus::SessionOnly);
}

/// Fails if `set` keeps the first key (the probe would still find it) or stops at `session_only`.
#[test]
fn set_replaces_the_current_key() {
    let mut store = SessionCredentialStore::new();
    let first = "pwt-test-FAKE-first-0000";
    let second = "pwt-test-FAKE-second-1111";
    store.set(secret(first));
    assert!(store.appears_in(first.as_bytes()));
    assert_eq!(store.set(secret(second)), CredentialStatus::SessionOnly);
    assert_eq!(store.status(), CredentialStatus::SessionOnly);
    assert!(!store.appears_in(first.as_bytes()), "the old key is gone");
    assert!(store.appears_in(second.as_bytes()), "the new key is held");
}

/// Fails if `delete` leaves the key held or reports a status other than `absent`.
#[test]
fn delete_returns_to_absent_and_forgets_the_key() {
    let mut store = SessionCredentialStore::new();
    store.set(secret(FAKE_KEY));
    assert_eq!(store.delete(), CredentialStatus::Absent);
    assert_eq!(store.status(), CredentialStatus::Absent);
    assert!(!store.appears_in(FAKE_KEY.as_bytes()));
    // A key can be set again after a delete.
    assert_eq!(store.set(secret(FAKE_KEY)), CredentialStatus::SessionOnly);
}

/// Fails if deleting an empty store panics or reports anything but `absent`.
#[test]
fn delete_on_an_empty_store_is_absent() {
    let mut store = SessionCredentialStore::new();
    assert_eq!(store.delete(), CredentialStatus::Absent);
    assert_eq!(store.delete(), CredentialStatus::Absent);
    assert_eq!(store.status(), CredentialStatus::Absent);
}

/// The store works through the trait object too (the `Session` seam holds it that way).
/// Fails if the trait methods are not the ones the concrete type implements.
#[test]
fn store_works_through_the_trait() {
    let mut boxed: Box<dyn CredentialStore> = Box::new(SessionCredentialStore::new());
    assert_eq!(boxed.status(), CredentialStatus::Absent);
    assert_eq!(boxed.set(secret(FAKE_KEY)), CredentialStatus::SessionOnly);
    assert!(boxed.appears_in(FAKE_KEY.as_bytes()));
    assert_eq!(boxed.delete(), CredentialStatus::Absent);
}

// ---------------------------------------------------------------------------------------------------------
// SecretProbe::appears_in.
// ---------------------------------------------------------------------------------------------------------

/// Fails if an empty store ever reports a match (including on an empty value).
#[test]
fn appears_in_is_false_when_absent() {
    let store = SessionCredentialStore::new();
    assert!(!store.appears_in(FAKE_KEY.as_bytes()));
    assert!(!store.appears_in(b""));
    assert!(!store.appears_in(b"anything at all"));
}

/// Fails if the probe only matches whole values or only at the start/end.
#[test]
fn appears_in_finds_the_key_anywhere_in_a_value() {
    let mut store = SessionCredentialStore::new();
    store.set(secret(FAKE_KEY));
    assert!(store.appears_in(FAKE_KEY.as_bytes()), "whole value");
    let embedded = format!("prefix={FAKE_KEY}&suffix");
    assert!(store.appears_in(embedded.as_bytes()), "middle");
    assert!(store.appears_in(format!("x{FAKE_KEY}").as_bytes()), "end");
    assert!(store.appears_in(format!("{FAKE_KEY}x").as_bytes()), "start");
    let mut bytes = vec![0xFF, 0x00, 0xFE];
    bytes.extend_from_slice(FAKE_KEY.as_bytes());
    assert!(store.appears_in(&bytes), "inside non-UTF-8 bytes");
}

/// Fails if a prefix (or any strict part) of the key counts as a match, or the match is case-folded.
#[test]
fn appears_in_is_false_for_a_partial_key() {
    let mut store = SessionCredentialStore::new();
    store.set(secret(FAKE_KEY));
    let prefix = &FAKE_KEY.as_bytes()[..FAKE_KEY.len() - 1];
    assert!(!store.appears_in(prefix), "all but the last byte");
    assert!(
        !store.appears_in(&FAKE_KEY.as_bytes()[1..]),
        "all but the first byte"
    );
    assert!(!store.appears_in(&FAKE_KEY.as_bytes()[..MIN_KEY_BYTES - 1]));
    assert!(!store.appears_in(b""), "empty value");
    assert!(
        !store.appears_in(FAKE_KEY.to_uppercase().as_bytes()),
        "case differs"
    );
    // Same length as the key, one byte different at the end / start: a prefix-only comparison would match.
    let last_differs = format!("{}X", &FAKE_KEY[..FAKE_KEY.len() - 1]);
    assert!(
        !store.appears_in(last_differs.as_bytes()),
        "last byte differs"
    );
    let first_differs = format!("X{}", &FAKE_KEY[1..]);
    assert!(
        !store.appears_in(first_differs.as_bytes()),
        "first byte differs"
    );
    let split = format!("{}-{}", &FAKE_KEY[..10], &FAKE_KEY[10..]);
    assert!(!store.appears_in(split.as_bytes()), "key broken by a byte");
}

// ---------------------------------------------------------------------------------------------------------
// read_key: format table.
// ---------------------------------------------------------------------------------------------------------

/// Fails if any boundary is off by one, or if a trailing line break is not removed exactly once.
#[test]
fn read_key_length_and_line_break_table() {
    use KeyFormatError::{Character, Empty, TooLong, TooShort};
    let ok = |len: usize| Ok(String::from_utf8(letters(len)).unwrap());
    let cases: Vec<(&str, Vec<u8>, Result<String, KeyFormatError>)> = vec![
        ("empty input", vec![], Err(Empty)),
        ("only LF", b"\n".to_vec(), Err(Empty)),
        ("only CRLF", b"\r\n".to_vec(), Err(Empty)),
        ("15 bytes", letters(15), Err(TooShort)),
        ("15 bytes + LF", with_suffix(15, b"\n"), Err(TooShort)),
        ("15 bytes + CRLF", with_suffix(15, b"\r\n"), Err(TooShort)),
        ("1 byte", letters(1), Err(TooShort)),
        ("16 bytes", letters(16), ok(16)),
        ("16 bytes + LF", with_suffix(16, b"\n"), ok(16)),
        ("16 bytes + CRLF", with_suffix(16, b"\r\n"), ok(16)),
        ("4096 bytes", letters(4096), ok(4096)),
        ("4096 bytes + LF", with_suffix(4096, b"\n"), ok(4096)),
        ("4096 bytes + CRLF", with_suffix(4096, b"\r\n"), ok(4096)),
        ("4097 bytes", letters(4097), Err(TooLong)),
        ("4097 bytes + LF", with_suffix(4097, b"\n"), Err(TooLong)),
        (
            "4097 bytes + CRLF",
            with_suffix(4097, b"\r\n"),
            Err(TooLong),
        ),
        ("4099 bytes", letters(4099), Err(TooLong)),
        // Only one line break is removed: the rest is not graphic.
        ("16 bytes + LF LF", with_suffix(16, b"\n\n"), Err(Character)),
        (
            "16 bytes + CRLF LF",
            with_suffix(16, b"\r\n\n"),
            Err(Character),
        ),
        (
            "16 bytes + CR CRLF",
            with_suffix(16, b"\r\r\n"),
            Err(Character),
        ),
        (
            "16 bytes + CRLF CRLF",
            with_suffix(16, b"\r\n\r\n"),
            Err(Character),
        ),
        // A lone CR at the end is not a line break.
        ("16 bytes + lone CR", with_suffix(16, b"\r"), Err(Character)),
        ("lone CR", b"\r".to_vec(), Err(TooShort)),
        // Length is checked before characters.
        ("3 spaces", b"   ".to_vec(), Err(TooShort)),
        ("5000 spaces", vec![b' '; 5000], Err(TooLong)),
    ];
    for (name, input, expected) in cases {
        assert_eq!(read(&input), expected, "case: {name}");
    }
}

/// Fails if inner whitespace, controls, DEL or non-ASCII bytes are accepted.
#[test]
fn read_key_rejects_inner_whitespace_controls_and_non_ascii() {
    let rejected: Vec<(&str, Vec<u8>)> = vec![
        ("inner space", b"pwt-test-FAKE 0123456789".to_vec()),
        ("leading space", b" pwt-test-FAKE-0123456789".to_vec()),
        ("trailing space", b"pwt-test-FAKE-0123456789 ".to_vec()),
        ("inner LF", b"pwt-test-FAKE\n0123456789".to_vec()),
        ("inner CRLF", b"pwt-test-FAKE\r\n0123456789".to_vec()),
        ("inner CR", b"pwt-test-FAKE\r0123456789".to_vec()),
        ("tab", b"pwt-test-FAKE\t0123456789".to_vec()),
        ("DEL 0x7F", b"pwt-test-FAKE\x7f0123456789".to_vec()),
        ("NUL", b"pwt-test-FAKE\x000123456789".to_vec()),
        (
            "non-ASCII UTF-8",
            "pwt-test-FAKE-\u{e9}-0123456789".as_bytes().to_vec(),
        ),
        (
            "invalid UTF-8",
            b"pwt-test-FAKE-\xff\xfe-0123456789".to_vec(),
        ),
    ];
    for (name, input) in rejected {
        assert_eq!(read(&input), Err(KeyFormatError::Character), "case: {name}");
    }
}

/// Every byte value in the middle of an otherwise valid key: accepted iff it is `0x21..=0x7E`.
/// Fails if the accepted range is widened or narrowed by even one byte.
#[test]
fn read_key_accepts_exactly_the_ascii_graphic_range() {
    for byte in 0..=255u8 {
        let mut input = b"pwt-test-FAKE-".to_vec();
        input.push(byte);
        input.extend_from_slice(b"-end");
        let expected_ok = (0x21..=0x7E).contains(&byte);
        let result = read(&input);
        if expected_ok {
            assert_eq!(
                result.as_deref().map(str::len),
                Ok(input.len()),
                "byte {byte:#04x}"
            );
        } else {
            assert_eq!(result, Err(KeyFormatError::Character), "byte {byte:#04x}");
        }
    }
}

/// Fails if the key is altered on the way in (trimmed, lower-cased, truncated).
#[test]
fn read_key_returns_the_exact_key_text() {
    assert_eq!(read(FAKE_KEY.as_bytes()).as_deref(), Ok(FAKE_KEY));
    let with_lf = format!("{FAKE_KEY}\n");
    assert_eq!(read(with_lf.as_bytes()).as_deref(), Ok(FAKE_KEY));
    let with_crlf = format!("{FAKE_KEY}\r\n");
    assert_eq!(read(with_crlf.as_bytes()).as_deref(), Ok(FAKE_KEY));
    let punctuation = "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~AZaz09";
    assert_eq!(read(punctuation.as_bytes()).as_deref(), Ok(punctuation));
}

/// A key that `read_key` returned can be stored and found by the probe (the CLI path end to end).
#[test]
fn read_key_result_goes_into_the_store() {
    let key = read_key(&mut Cursor::new(format!("{FAKE_KEY}\n")), false);
    let mut store = SessionCredentialStore::new();
    assert!(key.is_ok());
    if let Ok(key) = key {
        assert_eq!(store.set(key), CredentialStatus::SessionOnly);
    }
    assert!(store.appears_in(FAKE_KEY.as_bytes()));
}

// ---------------------------------------------------------------------------------------------------------
// read_key: reader behaviour.
// ---------------------------------------------------------------------------------------------------------

/// Counts reads and panics on any: proves that a terminal is refused before anything is read.
struct MustNotRead<'a> {
    calls: &'a Cell<usize>,
}

impl Read for MustNotRead<'_> {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        self.calls.set(self.calls.get() + 1);
        panic!("read_key read from a terminal");
    }
}

/// Fails if `read_key` touches the reader when `is_terminal` is true (the panic fails the test, and the
/// counter would be non-zero even if a caller swallowed the panic).
#[test]
fn terminal_input_is_refused_without_reading() {
    let calls = Cell::new(0);
    let mut reader = MustNotRead { calls: &calls };
    let result = read_key(&mut reader, true);
    assert_eq!(result.err(), Some(KeyFormatError::TerminalInput));
    assert_eq!(calls.get(), 0, "the reader must not be read");
}

/// Fails if the terminal check is skipped when the (would-be) data is a valid key.
#[test]
fn terminal_input_is_refused_even_when_the_data_is_valid() {
    let result = read_key(&mut Cursor::new(FAKE_KEY.as_bytes().to_vec()), true);
    assert_eq!(result.err(), Some(KeyFormatError::TerminalInput));
}

/// A reader whose every read fails.
struct Failing(ErrorKind);

impl Read for Failing {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Err(Error::new(self.0, "synthetic failure"))
    }
}

/// Fails if an I/O error is reported as anything but `Read` (e.g. as `Empty`), or panics.
#[test]
fn io_errors_are_reported_as_read() {
    for kind in [
        ErrorKind::BrokenPipe,
        ErrorKind::PermissionDenied,
        ErrorKind::UnexpectedEof,
        ErrorKind::Other,
    ] {
        let result = read_key(&mut Failing(kind), false);
        assert_eq!(result.err(), Some(KeyFormatError::Read), "kind {kind:?}");
    }
}

/// Valid key bytes, then an error: the partial data must not be accepted.
/// Fails if the error after partial data is swallowed and the partial key is used.
#[test]
fn io_error_after_partial_data_is_read_error() {
    struct Then {
        first: Option<Vec<u8>>,
    }
    impl Read for Then {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.first.take() {
                Some(data) => {
                    buf[..data.len()].copy_from_slice(&data);
                    Ok(data.len())
                }
                None => Err(Error::new(ErrorKind::BrokenPipe, "synthetic failure")),
            }
        }
    }
    let mut reader = Then {
        first: Some(FAKE_KEY.as_bytes().to_vec()),
    };
    assert_eq!(
        read_key(&mut reader, false).err(),
        Some(KeyFormatError::Read)
    );
}

/// Delivers at most `chunk` bytes per call.
struct Chunked {
    data: Vec<u8>,
    pos: usize,
    chunk: usize,
    interrupt_first: bool,
}

impl Read for Chunked {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.interrupt_first {
            self.interrupt_first = false;
            return Err(Error::new(ErrorKind::Interrupted, "synthetic interrupt"));
        }
        let n = self.chunk.min(buf.len()).min(self.data.len() - self.pos);
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Fails if `read_key` treats a short read as the end of input, or does not loop to EOF.
#[test]
fn short_chunks_are_assembled() {
    for chunk in [1, 2, 3, 7, 64, 4096] {
        let data = with_suffix(MAX_KEY_BYTES, b"\r\n");
        let mut reader = Chunked {
            data,
            pos: 0,
            chunk,
            interrupt_first: false,
        };
        let key = read_key(&mut reader, false).map(|s| s.expose().to_owned());
        assert_eq!(
            key,
            Ok(String::from_utf8(letters(MAX_KEY_BYTES)).unwrap()),
            "chunk {chunk}"
        );
    }
}

/// Fails if an `Interrupted` error aborts the read instead of being retried.
#[test]
fn interrupted_reads_are_retried() {
    let mut reader = Chunked {
        data: format!("{FAKE_KEY}\n").into_bytes(),
        pos: 0,
        chunk: 5,
        interrupt_first: true,
    };
    let key = read_key(&mut reader, false).map(|s| s.expose().to_owned());
    assert_eq!(key.as_deref(), Ok(FAKE_KEY));
}

/// A reader that never ends and counts the bytes it handed out.
struct Endless {
    served: usize,
}

impl Read for Endless {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        buf.fill(b'K');
        self.served += buf.len();
        Ok(buf.len())
    }
}

/// Fails if `read_key` reads past `MAX_KEY_BYTES + 3` = 4099 bytes (it would never return on this reader,
/// or `served` would exceed the bound).
#[test]
fn the_read_stops_at_the_bound() {
    let mut reader = Endless { served: 0 };
    let result = read_key(&mut reader, false);
    assert_eq!(result.err(), Some(KeyFormatError::TooLong));
    assert_eq!(MAX_KEY_BYTES + 3, 4099);
    assert_eq!(
        reader.served, 4099,
        "exactly the bound is consumed from an endless input"
    );
}

/// Input far larger than the bound, from a finite reader: only the bound is consumed.
/// Fails if the unread rest is consumed too.
#[test]
fn input_much_larger_than_the_cap_is_not_consumed() {
    let mut cursor = Cursor::new(letters(1_000_000));
    let result = read_key(&mut cursor, false);
    assert_eq!(result.err(), Some(KeyFormatError::TooLong));
    assert_eq!(cursor.position(), 4099);
}

/// A short input is read to its end and no further.
/// Fails if the position after the read differs from the input length (an over-read cannot happen on a
/// cursor, so this guards the loop-until-EOF behaviour: stopping early would leave the position short).
#[test]
fn a_short_input_is_read_to_its_end() {
    let mut cursor = Cursor::new(format!("{FAKE_KEY}\n"));
    assert!(read_key(&mut cursor, false).is_ok());
    assert_eq!(cursor.position() as usize, FAKE_KEY.len() + 1);
}

// ---------------------------------------------------------------------------------------------------------
// No key or length in Debug, errors or events.
// ---------------------------------------------------------------------------------------------------------

/// A 37-byte key so that its length "37" is distinctive.
const KEY_37: &str = "pwt-test-FAKE-debug-0123456789-abcdef";

/// Fails if `Debug` of the store or the secret shows key bytes, the length in decimal, or any digit.
#[test]
fn debug_shows_neither_key_nor_length() {
    assert_eq!(KEY_37.len(), 37);
    let mut store = SessionCredentialStore::new();
    let empty = format!("{store:?}");
    store.set(secret(KEY_37));
    let held = format!("{store:?}");
    let alternate = format!("{store:#?}");
    let bare = format!("{:?}", secret(KEY_37));

    assert_eq!(empty, "SessionCredentialStore { status: Absent }");
    assert_eq!(held, "SessionCredentialStore { status: SessionOnly }");
    assert_eq!(bare, "CredentialSecret(<redacted>)");
    for (name, text) in [
        ("empty", &empty),
        ("held", &held),
        ("alternate", &alternate),
        ("secret", &bare),
    ] {
        assert!(!text.contains(KEY_37), "{name}: key text");
        assert!(!text.contains("pwt-test"), "{name}: key fragment");
        assert!(!text.contains("37"), "{name}: length");
        assert!(
            !text.bytes().any(|b| b.is_ascii_digit()),
            "{name}: any digit"
        );
    }
}

/// Fails if a format error carries input (it is fieldless) or if its `Debug` names the rejected input.
#[test]
fn key_format_errors_carry_no_input() {
    let text = format!("{:?}", read(b"pwt-test-FAKE secret").unwrap_err());
    assert_eq!(text, "Character");
    for error in [
        KeyFormatError::Empty,
        KeyFormatError::TooShort,
        KeyFormatError::TooLong,
        KeyFormatError::Character,
        KeyFormatError::TerminalInput,
        KeyFormatError::Read,
    ] {
        assert!(!format!("{error:?}").contains("pwt"));
    }
}

/// Fails if the event serialises the key, or anything but the status.
#[test]
fn credential_status_event_has_no_key_material() {
    let mut store = SessionCredentialStore::new();
    let status = store.set(secret(KEY_37));
    let json = serde_json::to_string(&AppEvent::CredentialStatusChanged { status }).unwrap();
    assert_eq!(
        json,
        r#"{"event_type":"CredentialStatusChanged","status":"session_only"}"#
    );
    assert!(!contains(json.as_bytes(), KEY_37.as_bytes()));
    assert!(!contains(json.as_bytes(), b"pwt-test"));
    // The same for delete.
    let status = store.delete();
    let json = serde_json::to_string(&AppEvent::CredentialStatusChanged { status }).unwrap();
    assert_eq!(
        json,
        r#"{"event_type":"CredentialStatusChanged","status":"absent"}"#
    );
}
