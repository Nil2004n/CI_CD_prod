//! Session-only BYOK credential store (KEY-01, C2 = session-only; frozen interface
//! `planning/P03-T01-request.md` §5.6). Owner: W3, T07.
//!
//! The key lives in memory for the life of the process and nowhere else: never in argv, environment,
//! logs, stdout/stderr, store files or `Debug`. Format is validated locally only; a provider check is P16.
//! Clearing on drop and on rejected input is best effort with std only, not a secure erase. OS credential
//! store mode is not adopted (C2); FR-18–FR-23 do not apply while it is absent.

use std::io::{ErrorKind, Read};

use domain::contract::{CredentialSecret, CredentialStatus};

use crate::child_env::SecretProbe;

/// Fewest accepted key bytes.
pub const MIN_KEY_BYTES: usize = 16;
/// Most accepted key bytes (after removing one trailing line break).
pub const MAX_KEY_BYTES: usize = 4096;

/// Why key input was rejected. Fieldless: never contains input bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum KeyFormatError {
    /// Nothing (or only a line break) was read.
    Empty,
    /// Under [`MIN_KEY_BYTES`].
    TooShort,
    /// Over [`MAX_KEY_BYTES`].
    TooLong,
    /// A byte outside ASCII graphic `0x21..=0x7E` (spaces and inner line breaks included).
    Character,
    /// Standard input is a terminal; the key is accepted from a pipe only (C5).
    TerminalInput,
    /// Reading standard input failed.
    Read,
}

/// Holds at most one BYOK key.
pub trait CredentialStore: SecretProbe + std::fmt::Debug {
    /// The current status; never key material.
    fn status(&self) -> CredentialStatus;
    /// Stores `secret`, replacing (and clearing) any current key.
    fn set(&mut self, secret: CredentialSecret) -> CredentialStatus;
    /// Removes (and clears) the current key, if any.
    fn delete(&mut self) -> CredentialStatus;
}

/// The in-memory, process-lifetime store. `Debug` shows the status only (no key, no length). Not `Clone`
/// and not `Serialize`: the key cannot be copied or written out by accident.
///
/// # Compile-time guards
///
/// Positive control for the next case (compiles) - the status value is `Clone`:
///
/// ```
/// use application::credential::{CredentialStore, SessionCredentialStore};
/// let store = SessionCredentialStore::new();
/// let _ = Clone::clone(&store.status());
/// ```
///
/// Compile-fail case: `SessionCredentialStore` is not `Clone` (does not compile):
///
/// ```compile_fail
/// use application::credential::SessionCredentialStore;
/// let store = SessionCredentialStore::new();
/// let _ = Clone::clone(&store);
/// ```
///
/// Positive control for the next case (compiles) - the status value is `Serialize`:
///
/// ```
/// use application::credential::{CredentialStore, SessionCredentialStore};
/// let store = SessionCredentialStore::new();
/// let _ = serde_json::to_string(&store.status());
/// ```
///
/// Compile-fail case: `SessionCredentialStore` is not `Serialize` (does not compile):
///
/// ```compile_fail
/// use application::credential::SessionCredentialStore;
/// let store = SessionCredentialStore::new();
/// let _ = serde_json::to_string(&store);
/// ```
#[derive(Default)]
pub struct SessionCredentialStore {
    secret: Option<CredentialSecret>,
}

impl SessionCredentialStore {
    /// An empty store (`absent`).
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for SessionCredentialStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCredentialStore")
            .field("status", &self.status())
            .finish()
    }
}

impl SecretProbe for SessionCredentialStore {
    fn appears_in(&self, value: &[u8]) -> bool {
        match &self.secret {
            // A key is never empty (`MIN_KEY_BYTES`), but `windows(0)` panics, so the guard is explicit.
            Some(secret) if !secret.expose().is_empty() => {
                let key = secret.expose().as_bytes();
                value.windows(key.len()).any(|window| window == key)
            }
            _ => false,
        }
    }
}

impl CredentialStore for SessionCredentialStore {
    fn status(&self) -> CredentialStatus {
        match self.secret {
            Some(_) => CredentialStatus::SessionOnly,
            None => CredentialStatus::Absent,
        }
    }

    fn set(&mut self, secret: CredentialSecret) -> CredentialStatus {
        // Assigning drops the previous secret, whose `Drop` clears it (best effort).
        self.secret = Some(secret);
        CredentialStatus::SessionOnly
    }

    fn delete(&mut self) -> CredentialStatus {
        self.secret = None;
        CredentialStatus::Absent
    }
}

/// Read limit: the longest accepted key, a `\r\n`, and one byte more so that over-long input is seen.
const READ_LIMIT: usize = MAX_KEY_BYTES + 3;

/// The raw read buffer. Overwritten with zeros when dropped, so every exit path of [`read_key`] (success,
/// each rejection, I/O error, even a panic) clears it. Best effort: this zeroes the buffer this code owns,
/// it cannot reach copies the allocator, the OS or the caller's reader made.
struct ScratchBuffer(Vec<u8>);

impl ScratchBuffer {
    fn zeroed(len: usize) -> Self {
        Self(vec![0; len])
    }
}

impl Drop for ScratchBuffer {
    fn drop(&mut self) {
        #[cfg(test)]
        let nonzero_before = self.0.iter().filter(|byte| **byte != 0).count();
        // Clearing to length 0 and growing back to the capacity writes zeros over the whole allocation,
        // including bytes beyond the length; `black_box` keeps the writes from being optimised away.
        let capacity = self.0.capacity();
        self.0.clear();
        self.0.resize(capacity, 0);
        std::hint::black_box(&self.0);
        #[cfg(test)]
        clear_probe::record(nonzero_before, &self.0);
    }
}

/// Fills `buf` from `input` until it is full or `input` ends; returns the bytes read. Retries on
/// `Interrupted`. Never reads more than `buf.len()` bytes from `input`.
fn fill(input: &mut dyn Read, buf: &mut [u8]) -> Result<usize, KeyFormatError> {
    let mut filled = 0;
    while filled < buf.len() {
        match input.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Err(KeyFormatError::Read),
        }
    }
    Ok(filled)
}

/// Reads one key from `input` (a pipe): at most `MAX_KEY_BYTES + 3` bytes are read, one trailing `\n` or
/// `\r\n` is removed, then the format is checked. Refuses terminal input without reading. The read buffer is
/// cleared on every path (best effort, not a secure erase).
pub fn read_key(
    input: &mut dyn Read,
    is_terminal: bool,
) -> Result<CredentialSecret, KeyFormatError> {
    if is_terminal {
        return Err(KeyFormatError::TerminalInput);
    }
    let mut scratch = ScratchBuffer::zeroed(READ_LIMIT);
    let filled = fill(input, &mut scratch.0)?;
    let mut key = &scratch.0[..filled];
    if let Some(rest) = key.strip_suffix(b"\r\n") {
        key = rest;
    } else if let Some(rest) = key.strip_suffix(b"\n") {
        key = rest;
    }
    if key.is_empty() {
        return Err(KeyFormatError::Empty);
    }
    if key.len() < MIN_KEY_BYTES {
        return Err(KeyFormatError::TooShort);
    }
    if key.len() > MAX_KEY_BYTES {
        return Err(KeyFormatError::TooLong);
    }
    if !key.iter().all(u8::is_ascii_graphic) {
        return Err(KeyFormatError::Character);
    }
    // ASCII graphic bytes are valid UTF-8, so the conversion cannot fail; the arm is a safe fallback.
    match std::str::from_utf8(key) {
        Ok(text) => Ok(CredentialSecret::new(text.to_owned())),
        Err(_) => Err(KeyFormatError::Character),
    }
}

/// Test-only observation of the clearing step (a hook, compiled out of production builds). It sees the
/// scratch buffer after it was overwritten. That proves this code zeroed the buffer it owns; it cannot
/// prove that no copy survives in allocator or OS memory.
#[cfg(test)]
mod clear_probe {
    use std::cell::RefCell;

    /// What the clearing step saw.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(super) struct Report {
        /// Non-zero bytes in the buffer just before clearing.
        pub(super) nonzero_before: usize,
        /// Buffer capacity after clearing (the whole allocation was checked).
        pub(super) capacity: usize,
        /// Every byte of the allocation is zero after clearing.
        pub(super) all_zero_after: bool,
    }

    thread_local! {
        static LAST: RefCell<Option<Report>> = const { RefCell::new(None) };
    }

    pub(super) fn record(nonzero_before: usize, after: &Vec<u8>) {
        let report = Report {
            nonzero_before,
            capacity: after.capacity(),
            all_zero_after: after.len() == after.capacity() && after.iter().all(|byte| *byte == 0),
        };
        LAST.with(|last| *last.borrow_mut() = Some(report));
    }

    pub(super) fn reset() {
        LAST.with(|last| *last.borrow_mut() = None);
    }

    pub(super) fn take() -> Option<Report> {
        LAST.with(|last| last.borrow_mut().take())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Error, ErrorKind, Read};

    use super::{KeyFormatError, MAX_KEY_BYTES, MIN_KEY_BYTES, READ_LIMIT, clear_probe, read_key};

    /// Runs `read_key` on `bytes` and returns the result kind plus what the clearing step saw.
    fn run(bytes: &[u8]) -> (Result<(), KeyFormatError>, Option<clear_probe::Report>) {
        clear_probe::reset();
        let result = read_key(&mut Cursor::new(bytes.to_vec()), false).map(|_| ());
        (result, clear_probe::take())
    }

    fn key_of(len: usize) -> Vec<u8> {
        vec![b'K'; len]
    }

    /// Fails if `ScratchBuffer::drop` stops zeroing (e.g. the `clear`/`resize` pair is removed): the
    /// report would show `all_zero_after == false` for any input with non-zero bytes.
    fn assert_cleared(bytes: &[u8], expected: Result<(), KeyFormatError>) {
        let (result, report) = run(bytes);
        assert_eq!(result, expected, "result for a {}-byte input", bytes.len());
        let Some(report) = report else {
            panic!(
                "the clearing step did not run for a {}-byte input",
                bytes.len()
            );
        };
        assert_eq!(report.capacity, READ_LIMIT);
        assert!(report.all_zero_after, "buffer not zero after read_key");
        let expected_nonzero = bytes.iter().take(READ_LIMIT).filter(|b| **b != 0).count();
        assert_eq!(
            report.nonzero_before, expected_nonzero,
            "the hook must see the read bytes before clearing (else the check is vacuous)"
        );
    }

    #[test]
    fn buffer_is_zero_after_success() {
        assert_cleared(&key_of(MIN_KEY_BYTES), Ok(()));
        assert_cleared(b"pwt-test-FAKE-0123456789\r\n", Ok(()));
        assert_cleared(&key_of(MAX_KEY_BYTES), Ok(()));
    }

    #[test]
    fn buffer_is_zero_after_each_rejection() {
        assert_cleared(b"\n", Err(KeyFormatError::Empty));
        assert_cleared(b"\r\n", Err(KeyFormatError::Empty));
        assert_cleared(&key_of(MIN_KEY_BYTES - 1), Err(KeyFormatError::TooShort));
        assert_cleared(&key_of(MAX_KEY_BYTES + 1), Err(KeyFormatError::TooLong));
        assert_cleared(&key_of(10_000), Err(KeyFormatError::TooLong));
        assert_cleared(b"pwt-test-FAKE 0123456789", Err(KeyFormatError::Character));
        assert_cleared(
            b"pwt-test-FAKE-0123456789\r",
            Err(KeyFormatError::Character),
        );
    }

    #[test]
    fn buffer_is_zero_after_empty_input() {
        // No bytes at all: nothing to see before clearing, but the clearing step still runs.
        let (result, report) = run(b"");
        assert_eq!(result, Err(KeyFormatError::Empty));
        let Some(report) = report else {
            panic!("the clearing step did not run for empty input");
        };
        assert_eq!(report.nonzero_before, 0);
        assert!(report.all_zero_after);
    }

    /// Yields some key bytes, then an error.
    struct FailAfter {
        data: Vec<u8>,
        sent: bool,
    }

    impl Read for FailAfter {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.sent {
                return Err(Error::new(ErrorKind::BrokenPipe, "synthetic failure"));
            }
            self.sent = true;
            let n = self.data.len().min(buf.len());
            buf[..n].copy_from_slice(&self.data[..n]);
            Ok(n)
        }
    }

    #[test]
    fn buffer_is_zero_after_io_error_with_partial_data() {
        clear_probe::reset();
        let mut reader = FailAfter {
            data: b"pwt-test-FAKE-0123456789".to_vec(),
            sent: false,
        };
        let result = read_key(&mut reader, false).map(|_| ());
        assert_eq!(result, Err(KeyFormatError::Read));
        let Some(report) = clear_probe::take() else {
            panic!("the clearing step did not run after an I/O error");
        };
        assert_eq!(
            report.nonzero_before, 24,
            "partial key bytes were in the buffer"
        );
        assert!(report.all_zero_after);
    }

    #[test]
    fn terminal_input_creates_no_buffer() {
        clear_probe::reset();
        let result = read_key(&mut Cursor::new(key_of(32)), true).map(|_| ());
        assert_eq!(result, Err(KeyFormatError::TerminalInput));
        assert_eq!(
            clear_probe::take(),
            None,
            "a terminal refusal must not even allocate the read buffer"
        );
    }
}
