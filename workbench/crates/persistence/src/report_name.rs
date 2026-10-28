//! Report stem YYYYMMDDTHHMMSSZ_<slug>_<run-id> (Data Schema §13, request §6.4).
//!
//! Frozen public surface (request §5.12): [`ReportStem`] (`as_str`, `Display`) and [`report_stem`]. The
//! crate-internal helpers ([`compact_utc`], [`split_report_file_name`]) serve the writer and startup
//! reconciliation, which must recognise the application's own report files and nothing else.
//!
//! **Name bound.** A stem is `16 + 1 + slug + 1 + 36` bytes; [`Slug`] is limited to 48 bytes by its own
//! validation, so a stem is at most 102 bytes and the longest file name (`.json`, `.pdf` and `.md` are the
//! reserved extensions) at most 107 bytes. That is below [`MAX_REPORT_NAME_BYTES`], which in turn leaves
//! room for the longest temp name derived from a report name (see [`crate::paths`]). The constant
//! assertion below keeps that arithmetic honest if one of the numbers is ever changed.

use std::fmt;

use domain::ids::RunId;
use domain::store::values::Slug;
use domain::time::UtcTimestamp;

use crate::paths::MAX_REPORT_NAME_BYTES;

/// Length of the compact UTC stamp `YYYYMMDDTHHMMSSZ`.
const STAMP_BYTES: usize = 16;
/// Length of a canonical hyphenated run ID.
const RUN_ID_BYTES: usize = 36;
/// Longest [`Slug`] (the bound is enforced by `Slug::parse`; the tests pin it at 48).
const SLUG_MAX_BYTES: usize = 48;
/// Longest reserved extension, including the dot (`.json`).
const LONGEST_EXTENSION_BYTES: usize = 5;
/// The reserved extensions of the three report files of one stem.
const EXTENSIONS: [&str; 3] = ["json", "md", "pdf"];

const _: () = assert!(
    STAMP_BYTES + 1 + SLUG_MAX_BYTES + 1 + RUN_ID_BYTES + LONGEST_EXTENSION_BYTES
        <= MAX_REPORT_NAME_BYTES,
    "the longest generated report file name must fit paths::MAX_REPORT_NAME_BYTES"
);

/// A generated report stem: `YYYYMMDDTHHMMSSZ_<slug>_<run-id>` (UTC seconds, validated slug, full
/// 36-character run ID). File names are the stem plus `.json` / `.md` / `.pdf`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ReportStem(String);

impl ReportStem {
    /// The stem text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReportStem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The compact UTC stamp `YYYYMMDDTHHMMSSZ` of `at`, truncated to whole seconds (the fraction is dropped,
/// never rounded). Years 0001-9999, so the year always has four digits and the stamp is always 16 bytes.
pub(crate) fn compact_utc(at: UtcTimestamp) -> String {
    // `to_rfc3339` is `YYYY-MM-DDTHH:MM:SS[.f]Z`; the first 19 characters are the whole-second part.
    let mut stamp: String = at
        .to_rfc3339()
        .chars()
        .take(19)
        .filter(|c| !matches!(c, '-' | ':'))
        .collect();
    stamp.push('Z');
    stamp
}

/// The stem for a report of run `run` on the repository with `slug`, generated at `at`.
///
/// The stamp has whole-second resolution, so two reports of the same repository generated in the same
/// second differ by their run ID. The same run ID, repository slug and second give the same stem: the
/// writer then refuses the second report with `StoreError::ReportNameCollision` and never overwrites.
///
/// ```
/// use domain::ids::RunId;
/// use domain::store::values::Slug;
/// use domain::time::UtcTimestamp;
/// use persistence::report_name::report_stem;
///
/// let at = UtcTimestamp::parse_rfc3339("2026-10-16T05:25:00.987Z").unwrap();
/// let slug = Slug::parse("demo-repo").unwrap();
/// let run = RunId::parse("00000000-0000-4000-8000-000000000014").unwrap();
/// assert_eq!(
///     report_stem(at, &slug, run).as_str(),
///     "20261016T052500Z_demo-repo_00000000-0000-4000-8000-000000000014"
/// );
/// ```
pub fn report_stem(at: UtcTimestamp, slug: &Slug, run: RunId) -> ReportStem {
    ReportStem(format!("{}_{}_{}", compact_utc(at), slug.as_str(), run))
}

/// Whether `stamp` is `YYYYMMDDTHHMMSSZ`: digits with `T` at index 8 and `Z` at index 15.
fn is_compact_stamp(stamp: &str) -> bool {
    stamp.len() == STAMP_BYTES
        && stamp.bytes().enumerate().all(|(i, b)| match i {
            8 => b == b'T',
            15 => b == b'Z',
            _ => b.is_ascii_digit(),
        })
}

/// Splits `name` into `(stem, extension)` if it is exactly a file name this module generates:
/// `<YYYYMMDDTHHMMSSZ>_<valid slug>_<canonical run id>.<json|md|pdf>`. Anything else, including a name
/// that merely looks similar (wrong stamp shape, upper-case or nil run ID, extra segments, other
/// extension), is `None`. A slug cannot contain `_`, so the split on `_` is unambiguous.
pub(crate) fn split_report_file_name(name: &str) -> Option<(&str, &'static str)> {
    let (stem, ext) = name.rsplit_once('.')?;
    let ext = EXTENSIONS.into_iter().find(|candidate| *candidate == ext)?;
    let mut parts = stem.split('_');
    let (stamp, slug, run) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some()
        || !is_compact_stamp(stamp)
        || Slug::parse(slug).is_err()
        || run.len() != RUN_ID_BYTES
        || RunId::parse(run).is_err()
    {
        return None;
    }
    Some((stem, ext))
}
