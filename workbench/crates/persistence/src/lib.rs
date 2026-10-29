//! Schema checks, single writer, exports and recovery (Tech §4, Data Schema §12–§13; P02 frozen surface
//! `planning/P02-T01-request.md` §5.4–§5.6).
//!
//! All file-system access goes through [`fs::FsOps`]; only `fs.rs` touches `std::fs` (static check).
//! Store documents are decoded only in [`read`], after the version gate and schema validation.
#![forbid(unsafe_code)]

pub mod atomic;
pub mod error;
pub mod fault;
pub mod fs;
pub mod lock;
pub mod paths;
pub mod read;
pub mod recovery;
pub mod report_name;
pub mod schema;
pub mod semantic;
pub mod validate;
pub mod writer;
