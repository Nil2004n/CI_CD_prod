//! Application command/event contract v1.0 (Architecture §6; frozen surface
//! `planning/P01-T01-request.md` §5.5). `AppCommand`, `AppEvent` and their payload field list are added
//! by the lead at baseline B1, before P01-T08.

/// Application command/event contract version (Architecture §6, A-11).
pub const APP_CONTRACT_VERSION: &str = "1.0";

/// Whether a command or event is implemented in the pilot or only reserved.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ContractStatus {
    /// Implemented in the pilot.
    Pilot,
    /// Name reserved for deferred work; dispatching it fails with code `deferred`.
    Reserved,
}

/// Delivery rule for an event (Architecture §6).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DeliveryClass {
    /// Never dropped or coalesced.
    MustDeliver,
    /// Intermediate values may be replaced by the latest.
    LatestValue,
    /// May be merged without losing content.
    Coalescible,
}
