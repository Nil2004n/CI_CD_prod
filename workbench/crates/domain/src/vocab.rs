//! Assessment vocabulary (P01-T07; frozen surface `planning/P01-T01-request.md` §5.4).
//!
//! String values equal the enums of `schemas/workbench-store-v1.3.schema.json`.

use serde::{Deserialize, Serialize};

/// Permission capability (`$defs/Capability`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Clone a new managed repository.
    GitAcquire,
    /// Update an existing managed checkout.
    GitUpdate,
    /// Retrieve an external reference.
    ExternalReference,
    /// Fetch a schema.
    SchemaFetch,
    /// Query a vulnerability source.
    VulnerabilityLookup,
    /// Fetch remote documentation (deferred).
    DocumentationFetch,
    /// Send approved context to the model provider.
    ModelTransmission,
    /// Send content to an embedding provider (deferred).
    EmbeddingTransmission,
    /// Render templates locally.
    LocalRender,
    /// Non-mutating live verification (deferred).
    NonMutatingVerification,
    /// Application-owned storage.
    AppStorage,
}

impl Capability {
    /// Every value, in schema order.
    pub const ALL: [Self; 11] = [
        Self::GitAcquire,
        Self::GitUpdate,
        Self::ExternalReference,
        Self::SchemaFetch,
        Self::VulnerabilityLookup,
        Self::DocumentationFetch,
        Self::ModelTransmission,
        Self::EmbeddingTransmission,
        Self::LocalRender,
        Self::NonMutatingVerification,
        Self::AppStorage,
    ];

    /// The schema string value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitAcquire => "git_acquire",
            Self::GitUpdate => "git_update",
            Self::ExternalReference => "external_reference",
            Self::SchemaFetch => "schema_fetch",
            Self::VulnerabilityLookup => "vulnerability_lookup",
            Self::DocumentationFetch => "documentation_fetch",
            Self::ModelTransmission => "model_transmission",
            Self::EmbeddingTransmission => "embedding_transmission",
            Self::LocalRender => "local_render",
            Self::NonMutatingVerification => "non_mutating_verification",
            Self::AppStorage => "app_storage",
        }
    }

    /// Whether pilot semantic validation rejects this capability as deferred (Data §18.6a).
    pub fn is_deferred(self) -> bool {
        matches!(
            self,
            Self::DocumentationFetch | Self::EmbeddingTransmission | Self::NonMutatingVerification
        )
    }
}

/// Assessment category (`$defs/Category`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// YAML syntax.
    Syntax,
    /// Schema conformance.
    Schema,
    /// Suspected secrets.
    Secrets,
    /// Security misconfiguration.
    Misconfiguration,
    /// Known vulnerabilities.
    Vulnerability,
    /// External verification (deferred capability).
    ExternalVerification,
    /// Style diagnostics.
    Style,
    /// Workflow validation.
    WorkflowValidation,
    /// Template rendering.
    Rendering,
}

impl Category {
    /// Every value, in schema order.
    pub const ALL: [Self; 9] = [
        Self::Syntax,
        Self::Schema,
        Self::Secrets,
        Self::Misconfiguration,
        Self::Vulnerability,
        Self::ExternalVerification,
        Self::Style,
        Self::WorkflowValidation,
        Self::Rendering,
    ];

    /// The schema string value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::Schema => "schema",
            Self::Secrets => "secrets",
            Self::Misconfiguration => "misconfiguration",
            Self::Vulnerability => "vulnerability",
            Self::ExternalVerification => "external_verification",
            Self::Style => "style",
            Self::WorkflowValidation => "workflow_validation",
            Self::Rendering => "rendering",
        }
    }
}

/// Finding severity (`$defs/Severity`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Critical.
    Critical,
    /// High.
    High,
    /// Medium.
    Medium,
    /// Low.
    Low,
    /// Informational.
    Info,
    /// Not determined.
    Unknown,
}

impl Severity {
    /// Every value, in schema order.
    pub const ALL: [Self; 6] = [
        Self::Critical,
        Self::High,
        Self::Medium,
        Self::Low,
        Self::Info,
        Self::Unknown,
    ];

    /// The schema string value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Info => "info",
            Self::Unknown => "unknown",
        }
    }
}

/// Assessment summary (`$defs/Assessment`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Assessment {
    /// Findings were observed.
    Findings,
    /// No findings in the available evidence (qualify by coverage).
    NoFindings,
    /// No meaningful assessment evidence.
    Unknown,
}

impl Assessment {
    /// Every value, in schema order.
    pub const ALL: [Self; 3] = [Self::Findings, Self::NoFindings, Self::Unknown];

    /// The schema string value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Findings => "findings",
            Self::NoFindings => "no_findings",
            Self::Unknown => "unknown",
        }
    }
}

/// Coverage summary (`$defs/Coverage`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    /// All selected checks produced usable evidence.
    Complete,
    /// Some coverage was lost.
    Partial,
    /// No usable coverage.
    None,
}

impl Coverage {
    /// Every value, in schema order.
    pub const ALL: [Self; 3] = [Self::Complete, Self::Partial, Self::None];

    /// The schema string value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::None => "none",
        }
    }
}
