//! Session-only BYOK credential store (KEY-01, C2 = session-only; frozen interface
//! `planning/P03-T01-request.md` §5.6). Owner: W3, T07.
//!
//! The key lives in memory for the life of the process and nowhere else: never in argv, environment,
//! logs, stdout/stderr, store files or `Debug`. Format is validated locally only; a provider check is P16.
//! Clearing on drop and on rejected input is best effort with std only, not a secure erase. OS credential
//! store mode is not adopted (C2); FR-18–FR-23 do not apply while it is absent.

use std::io::Read;

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

/// The in-memory, process-lifetime store. `Debug` shows the status only.
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
        let _ = (&self.secret, f);
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T07")
        }
    }
}

impl SecretProbe for SessionCredentialStore {
    fn appears_in(&self, value: &[u8]) -> bool {
        let _ = value;
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T07")
        }
    }
}

impl CredentialStore for SessionCredentialStore {
    fn status(&self) -> CredentialStatus {
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T07")
        }
    }

    fn set(&mut self, secret: CredentialSecret) -> CredentialStatus {
        let _ = secret;
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T07")
        }
    }

    fn delete(&mut self) -> CredentialStatus {
        #[expect(clippy::todo, reason = "B0 stub")]
        {
            todo!("P03-T07")
        }
    }
}

/// Reads one key from `input` (a pipe): at most `MAX_KEY_BYTES + 3` bytes are read, one trailing `\n` or
/// `\r\n` is removed, then the format is checked. Refuses terminal input without reading. The read buffer is
/// cleared on every path.
pub fn read_key(
    input: &mut dyn Read,
    is_terminal: bool,
) -> Result<CredentialSecret, KeyFormatError> {
    let _ = (input, is_terminal);
    #[expect(clippy::todo, reason = "B0 stub")]
    {
        todo!("P03-T07")
    }
}
