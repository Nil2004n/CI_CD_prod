//! Domain types for the PWT YAML Security Workbench: typed identifiers, UTC timestamps, text-safety
//! newtypes, assessment vocabulary, state machines, the application command/event contract and the typed
//! store records (`store`, P02).
//!
//! No TUI, I/O or process dependency (Tech §4). Frozen public surface: `planning/P01-T01-request.md` §5.
#![forbid(unsafe_code)]

pub mod contract;
pub mod ids;
pub mod states;
pub mod store;
pub mod text;
pub mod time;
pub mod versions;
pub mod vocab;
