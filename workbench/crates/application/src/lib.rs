//! Use cases, workflow/run coordinator and permission policy (Tech §4). P02: event emission with the
//! CA-04 envelope (`emit`) and the `dev` store entry points used by the CLI (`dev_store`). P03
//! (`planning/P03-T01-request.md` §3): permission requests and the evaluator (`permission`), the approval
//! lifecycle (`approvals`), the child-environment sanitiser (`child_env`), the session credential store
//! (`credential`) and `dev permission-check` (`dev_permission`).
#![forbid(unsafe_code)]

pub mod approvals;
pub mod child_env;
pub mod credential;
pub mod dev_permission;
pub mod dev_store;
pub mod emit;
pub mod permission;
pub mod repository_registry;
pub mod repository_service;
