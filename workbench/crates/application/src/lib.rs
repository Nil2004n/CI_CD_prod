//! Use cases, workflow/run coordinator and permission policy (Tech §4). P02: event emission with the
//! CA-04 envelope (`emit`) and the `dev` store entry points used by the CLI (`dev_store`).
#![forbid(unsafe_code)]

pub mod dev_store;
pub mod emit;
