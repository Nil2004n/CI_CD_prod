//! UTC timestamps (P01-T05; frozen surface `planning/P01-T01-request.md` §5.2).
//!
//! Accepted form: `YYYY-MM-DDTHH:MM:SS[.f{1,9}]Z`, years 0001–9999, no leap second, `Z` only.
//! Canonical output omits a zero fraction and trims trailing fractional zeros.

use std::fmt;

/// Why a timestamp was rejected. Carries no input text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeError {
    /// Not the accepted RFC 3339 UTC form.
    Format,
    /// A field or the instant is outside the supported range.
    Range,
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Fixed texts only: the rejected input is never echoed.
        match self {
            Self::Format => f.write_str("timestamp is not YYYY-MM-DDTHH:MM:SS[.fraction]Z"),
            Self::Range => f.write_str("timestamp field or instant is out of range"),
        }
    }
}

impl std::error::Error for TimeError {}

/// Smallest supported instant: 0001-01-01T00:00:00Z.
const MIN_SECS: i64 = -62_135_596_800;
/// Largest supported whole second: 9999-12-31T23:59:59Z.
const MAX_SECS: i64 = 253_402_300_799;
/// Nanoseconds per second.
const NANOS_PER_SEC: u32 = 1_000_000_000;
/// Seconds per day.
const SECS_PER_DAY: i64 = 86_400;

/// Days since 1970-01-01 for a proleptic Gregorian date (days-from-civil algorithm).
/// `month` is 1..=12 and `day` is 1..=31; the caller has validated both.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`]: `(year, month, day)` for days since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        _ => 28,
    }
}

/// Exactly `n` ASCII digits starting at `start`, as a number; `None` on anything else.
fn digits(bytes: &[u8], start: usize, n: usize) -> Option<i64> {
    let slice = bytes.get(start..start.checked_add(n)?)?;
    let mut value = 0i64;
    for &c in slice {
        if !c.is_ascii_digit() {
            return None;
        }
        value = value * 10 + i64::from(c - b'0');
    }
    Some(value)
}

/// `true` when `bytes[pos] == expected`.
fn byte_is(bytes: &[u8], pos: usize, expected: u8) -> bool {
    bytes.get(pos) == Some(&expected)
}

/// Fraction digits (1..=9 ASCII digits) scaled to nanoseconds.
fn parse_fraction(frac: &[u8]) -> Option<u32> {
    if frac.is_empty() || frac.len() > 9 {
        return None;
    }
    let mut nanos = 0u32;
    for &c in frac {
        if !c.is_ascii_digit() {
            return None;
        }
        nanos = nanos * 10 + u32::from(c - b'0');
    }
    for _ in frac.len()..9 {
        nanos *= 10;
    }
    Some(nanos)
}

/// An instant in UTC, stored as seconds and nanoseconds since the Unix epoch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcTimestamp {
    secs: i64,
    nanos: u32,
}

impl UtcTimestamp {
    /// Parses the accepted RFC 3339 UTC form.
    ///
    /// Error split: `TimeError::Format` for any deviation from the textual shape (length, digit or
    /// separator positions, lowercase `t`/`z`, offsets, whitespace, non-ASCII, an empty or
    /// longer-than-9-digit fraction). Only a string that has the exact shape can yield
    /// `TimeError::Range`, for numerically invalid fields: year `0000`, month outside 1..=12, a day
    /// that does not exist in that month and year, hour above 23, minute above 59 or second above
    /// 59 (so a leap second `:60` is `Range`).
    pub fn parse_rfc3339(s: &str) -> Result<Self, TimeError> {
        let b = s.as_bytes();
        let shape = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')];
        if !shape.iter().all(|&(pos, c)| byte_is(b, pos, c)) {
            return Err(TimeError::Format);
        }
        let field = |start, n| digits(b, start, n).ok_or(TimeError::Format);
        let year = field(0, 4)?;
        let month = field(5, 2)?;
        let day = field(8, 2)?;
        let hour = field(11, 2)?;
        let minute = field(14, 2)?;
        let second = field(17, 2)?;
        let nanos = match b.get(19..) {
            Some([b'Z']) => 0,
            Some([b'.', frac @ .., b'Z']) => parse_fraction(frac).ok_or(TimeError::Format)?,
            _ => return Err(TimeError::Format),
        };
        // The shape is now fully valid; what remains is numeric range checking.
        if year < 1
            || !(1..=12).contains(&month)
            || day < 1
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return Err(TimeError::Range);
        }
        let secs =
            days_from_civil(year, month, day) * SECS_PER_DAY + hour * 3600 + minute * 60 + second;
        Self::from_unix(secs, nanos)
    }

    /// Formats the canonical RFC 3339 UTC form.
    pub fn to_rfc3339(&self) -> String {
        let (year, month, day) = civil_from_days(self.secs.div_euclid(SECS_PER_DAY));
        let rem = self.secs.rem_euclid(SECS_PER_DAY);
        let mut out = format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}",
            rem / 3600,
            rem % 3600 / 60,
            rem % 60
        );
        if self.nanos != 0 {
            let frac = format!("{:09}", self.nanos);
            out.push('.');
            out.push_str(frac.trim_end_matches('0'));
        }
        out.push('Z');
        out
    }

    /// Builds a timestamp from Unix seconds and nanoseconds (`nanos < 1_000_000_000`,
    /// instant within years 0001–9999).
    ///
    /// Returns `TimeError::Range` for `nanos >= 1_000_000_000` and for any instant outside
    /// `0001-01-01T00:00:00Z..=9999-12-31T23:59:59.999999999Z`.
    pub fn from_unix(secs: i64, nanos: u32) -> Result<Self, TimeError> {
        if nanos >= NANOS_PER_SEC || !(MIN_SECS..=MAX_SECS).contains(&secs) {
            return Err(TimeError::Range);
        }
        Ok(Self { secs, nanos })
    }

    /// Seconds since the Unix epoch.
    pub fn unix_seconds(&self) -> i64 {
        self.secs
    }

    /// Nanoseconds within the second.
    pub fn subsec_nanos(&self) -> u32 {
        self.nanos
    }

    /// The current system time.
    ///
    /// Never panics. A clock set before 1970 yields negative Unix seconds (floor semantics, so
    /// `nanos` stays in `0..1e9`). A clock outside years 0001–9999 is clamped to the nearest
    /// bound of the supported range.
    pub fn now() -> Self {
        let (secs, nanos) = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            Ok(after) => (
                i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
                after.subsec_nanos(),
            ),
            Err(err) => {
                let before = err.duration();
                let whole = i64::try_from(before.as_secs()).unwrap_or(i64::MAX);
                match before.subsec_nanos() {
                    0 => (-whole, 0),
                    n => (-whole - 1, NANOS_PER_SEC - n),
                }
            }
        };
        let clamped = secs.clamp(MIN_SECS, MAX_SECS);
        // Out-of-range clock: pin the fraction to the same side as the clamped second.
        let nanos = match secs {
            s if s < MIN_SECS => 0,
            s if s > MAX_SECS => NANOS_PER_SEC - 1,
            _ => nanos,
        };
        Self {
            secs: clamped,
            nanos,
        }
    }
}

impl fmt::Display for UtcTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl fmt::Debug for UtcTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UtcTimestamp({})", self.to_rfc3339())
    }
}

impl serde::Serialize for UtcTimestamp {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

impl<'de> serde::Deserialize<'de> for UtcTimestamp {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse_rfc3339(&s).map_err(serde::de::Error::custom)
    }
}
