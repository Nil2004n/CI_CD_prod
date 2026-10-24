# PWT YAML Security Workbench — Database and Data Schema

Consolidated revision 0.3. Includes all decisions through 23 October 2026: OpenRouter BYOK; local JSON/Markdown/PDF; named tool and wrapper plan; one core developer plus a Figma design team; Opus-led verification, Sonnet implementation, Haiku documentation and owner-approved debug cycles. This specification is not evidence of implemented code or completed tests. Earlier v0.1/v0.2 wording is superseded by this consolidated set.

Version: 0.3.3 (amendments CA-01, CA-02; store contract 1.3, wrapper protocol 1.1) | Date: 2026-10-23 | Status: Proposed implementation contract

## 1. Storage decision

The initial “database” is an application-owned JSON document store. A SQL server, SQLite file or vector database is not assumed or required. This preserves the agreed `log.json` and backup-report design. If scale later requires a transactional/indexed backend, treat that as a migration decision; keep the public report format stable.

The JSON Schema below uses Draft 2020-12, a published JSON Schema dialect. [web:132] The schema defines shape, not all runtime security/relational constraints. Code examples are illustrative proposed records, not live company repositories.

## 2. Physical file mapping

| File/directory | Root schema definition | Contents |
|---|---|---|
| `config.json` | ConfigFile | Resource limits, paths, theme, provider references, policy settings |
| `repo.json` | RepositoryFile | Registered repositories and latest discovery inventory |
| `workflows.json` | WorkflowFile | Versioned workflow definitions and selections |
| `approvals.json` | ApprovalFile | Pending decisions and constrained grants/denials |
| `log.json` | LogFile | Run summaries, sanitised events, report manifests |
| `suppressions.json` | SuppressionFile | Reserved: justified finding suppressions (RES-06 deferred; not written by the pilot) |
| `backup/<timestamp>_<repo-slug>_<run-id>.json` | ReportFile | Immutable per-repository assessment snapshot |
| Matching `.md` report | Derived text | Deterministic evidence with optional labelled AI narrative |
| Matching `.pdf` report | Derived binary | Restricted local conversion; independent export result |
| `tools.json` | ToolManifestFile | Versioned adapter/tool availability, pinned contracts and licensing metadata |
| `rag/index.json` | RagIndexFile | Sanitised retrievable chunks and provenance |
| `rag/conversations.json` | ConversationFile | Sanitised chat history only when persistence is enabled |
| `checkouts/<repo-id>/` | Not a JSON store | Tool-managed Git checkout; may contain original sensitive source |
| `tmp/`, `cache/` | Adapter-owned bounded formats | Approved transient/generated inputs; retention controlled |

The app-data root is platform-resolved or explicitly configured. Store relative internal paths where possible. Never store passwords, API keys, raw detected secrets, raw authentication output, or unrestricted raw scanner output in these JSON stores. OpenRouter is the pilot runtime provider. Credentials are session-only or an opaque OS credential-store reference, never key values. No PWT runtime server/shared key is required.

Operational stores are application-writeable; source repositories are assessment-read-only except approved Git clone/update operations. This is the agreed distinction between no auto-remediation and necessary app persistence.

## 3. Logical entity relationship model

```mermaid
erDiagram
  REPOSITORY ||--o{ FILE_RECORD : inventories
  WORKFLOW ||--|{ REPO_TARGET : selects
  REPOSITORY ||--o{ REPO_TARGET : referenced_by
  WORKFLOW ||--o{ RUN_SUMMARY : launches
  RUN_SUMMARY ||--o{ REPORT : produces_per_repo
  REPORT ||--o{ CHECK : contains
  CHECK ||--o{ FINDING : yields
  REPORT ||--o{ ARTIFACT : references
  FINDING }o--o{ SOURCE_REF : supported_by
  APPROVAL }o--o{ RUN_SUMMARY : authorises_scoped_operations
  SOURCE_REF ||--o{ CHUNK : indexed_as
  CONVERSATION ||--o{ MESSAGE : contains
  MESSAGE }o--o{ SOURCE_REF : cites
  SUPPRESSION }o--o{ FINDING : reviews
```

Relationships are logical and frequently embedded, not SQL foreign keys. Ad-hoc runs have no workflow. Reports remain interpretable if a repository/workflow is later removed because they retain frozen plan/provenance. The schema permits historical IDs without requiring current registration to exist.

## 4. Identity and common conventions

- Primary identifiers: UUID strings. Repository names, branch names and slugs are not primary keys.
- Repository-relative paths: forward-slash storage form; runtime rejects absolute/escaping paths and translates to native paths at access time.
- Source locations: one-based lines/documents; null when unavailable. Generated location is not invented source location.
- Timestamps: UTC RFC 3339 for persisted events; schedule separately stores IANA timezone and local-time intent.
- Git OIDs: accept full 40- or 64-character lowercase hex IDs; do not assume every repo uses the same hash algorithm.
- Application content digests: SHA-256 hex. Avoid using raw secret or an unsalted secret hash as a persisted fingerprint input.
- Fingerprints: stable rule/repository/path/normalised-location or resource identity; exclude raw secret values. A secret finding can use rule + location; do not persist reversible secret snippets.
- `schema_version`: storage-contract version, independent of app/scanner versions.
- `revision`: workflow definition revision. Historical runs retain their own plan snapshot.
- `store_type`: discriminates root document shapes.

## 5. Entity dictionary

| Entity | Key | Notes |
|---|---|---|
| Repository | repo_id | Canonical remote, local checkout, default ref, latest inventory |
| FileRecord | file_id | Stable UUID for repo/path; current digest and framework are observations |
| Workflow | workflow_id + revision | Targets, checks, schedule, approval references |
| Approval | approval_id | Capability, operation fingerprint, destinations/data classes, validity |
| RunSummary | run_id | Lifecycle and immutable plan snapshot, aggregate coverage, report IDs |
| Check | check_id | Specific capability result with task/tool provenance |
| Finding | finding_id | Redacted evidence, engine, severity, source refs and review state |
| Artifact | artifact_id | Generated/external/schema/doc provenance; retained path optional |
| Report | report_id | Per-run/per-repo evidence; authoritative machine-readable payload |
| Event | event_id | Sanitised structured activity, not unrestricted free-form process output |
| Chunk | chunk_id | Redacted text plus exact provenance and optional embedding reference |
| Conversation | conversation_id | Explicit context and optional persisted sanitised messages |
| Suppression | suppression_id | Rule/fingerprint scope, rationale, actor and optional expiry |

## 6. Status and counting semantics

Run execution: queued, awaiting_approval, running, cancelling, completed, partial, failed, cancelled, interrupted.

Check outcome: pending, running, passed, findings, failed, unsupported, skipped, incomplete, cancelled.

Assessment: findings, no_findings, unknown. Coverage: complete, partial, none.

- `completed` does not mean clean. A completed assessment may contain critical findings.
- `assessment=no_findings` only describes available evidence; the UI must qualify it if coverage is partial/none.
- `assessment=unknown` applies where there is no meaningful assessment evidence.
- A check with findings may end incomplete; retain findings but do not label its execution passed.
- File counts count unique discovered/selected file identities within the reported scope. Check counts count check records, not files or rules unless explicitly modelled as separate checks.
- Run aggregates deduplicate target/file identities across per-repo reports. A reference artifact is not an extra selected source file.
- Terminal report checks may not be left pending/running. Convert abandoned work to incomplete/cancelled with explanation during finalisation/recovery.
- Suppressed findings remain in the report and coverage history. “Resolved” is a comparison result requiring compatible completed coverage, not inferred because a check failed or a file was omitted.

## 7. Canonical JSON Schema

This single schema validates all proposed root store types via `oneOf` and `$defs`. An implementation may split it into files while preserving references and versioning. All fields are required unless explicitly optional in the schema; nullable fields represent unknown/not-applicable values. Collections may be empty unless semantic validation below prohibits it for an enabled operation.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$comment": "Store contract 1.3 / wrapper protocol 1.1 = store 1.2 / protocol 1.0 plus pre-implementation amendment CA-01 (2026-10-23, identifiers retained) plus amendment CA-02 (2026-10-24, versioned by owner decision Q-03). See 05_Database_Data_Schema.md sections 22-23 and 19 (records under 1.2/1.0 are not valid 1.3/1.1 records).",
  "title": "PWT Workbench Store Contract v1.3",
  "description": "Proposed shared contract; semantic and referential constraints are enforced by application code.",
  "oneOf": [
    {
      "$ref": "#/$defs/ConfigFile"
    },
    {
      "$ref": "#/$defs/RepositoryFile"
    },
    {
      "$ref": "#/$defs/WorkflowFile"
    },
    {
      "$ref": "#/$defs/ApprovalFile"
    },
    {
      "$ref": "#/$defs/LogFile"
    },
    {
      "$ref": "#/$defs/SuppressionFile"
    },
    {
      "$ref": "#/$defs/RagIndexFile"
    },
    {
      "$ref": "#/$defs/ConversationFile"
    },
    {
      "$ref": "#/$defs/ReportFile"
    },
    {
      "$ref": "#/$defs/ToolManifestFile"
    }
  ],
  "$defs": {
    "Id": {
      "type": "string",
      "format": "uuid"
    },
    "Timestamp": {
      "type": "string",
      "format": "date-time"
    },
    "Digest": {
      "type": "string",
      "pattern": "^[a-f0-9]{64}$"
    },
    "GitOid": {
      "type": "string",
      "pattern": "^(?:[a-f0-9]{40}|[a-f0-9]{64})$"
    },
    "RelativePath": {
      "type": "string",
      "minLength": 1,
      "description": "Repository-relative forward-slash path. Runtime rejects absolute, drive, dot-dot, NUL and escaping paths."
    },
    "Category": {
      "type": "string",
      "enum": [
        "syntax",
        "schema",
        "secrets",
        "misconfiguration",
        "vulnerability",
        "external_verification",
        "style",
        "workflow_validation",
        "rendering"
      ]
    },
    "Execution": {
      "type": "string",
      "enum": [
        "queued",
        "awaiting_approval",
        "running",
        "cancelling",
        "completed",
        "partial",
        "failed",
        "cancelled",
        "interrupted"
      ]
    },
    "Outcome": {
      "type": "string",
      "enum": [
        "pending",
        "running",
        "passed",
        "findings",
        "failed",
        "unsupported",
        "skipped",
        "incomplete",
        "cancelled"
      ]
    },
    "Assessment": {
      "type": "string",
      "enum": [
        "findings",
        "no_findings",
        "unknown"
      ]
    },
    "Coverage": {
      "type": "string",
      "enum": [
        "complete",
        "partial",
        "none"
      ]
    },
    "Severity": {
      "type": "string",
      "enum": [
        "critical",
        "high",
        "medium",
        "low",
        "info",
        "unknown"
      ]
    },
    "Selector": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "mode": {
          "type": "string",
          "enum": [
            "all_yaml",
            "explicit",
            "patterns"
          ]
        },
        "paths": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/RelativePath"
          }
        },
        "include_patterns": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "exclude_patterns": {
          "type": "array",
          "items": {
            "type": "string"
          }
        }
      },
      "required": [
        "mode",
        "paths",
        "include_patterns",
        "exclude_patterns"
      ]
    },
    "RepoTarget": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "repo_id": {
          "$ref": "#/$defs/Id"
        },
        "ref_kind": {
          "type": "string",
          "enum": [
            "branch",
            "tag",
            "commit"
          ]
        },
        "ref_value": {
          "type": "string"
        },
        "selector": {
          "$ref": "#/$defs/Selector"
        }
      },
      "required": [
        "repo_id",
        "ref_kind",
        "ref_value",
        "selector"
      ]
    },
    "SchemaBinding": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "path_pattern": {
          "type": "string"
        },
        "schema_resource": {
          "type": "string"
        },
        "dialect": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "expected_digest": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "path_pattern",
        "schema_resource",
        "dialect",
        "expected_digest"
      ]
    },
    "RendererSetting": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "framework": {
          "type": "string"
        },
        "renderer_id": {
          "type": "string"
        },
        "input_paths": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/RelativePath"
          }
        },
        "parameters_redacted": {
          "type": "object",
          "additionalProperties": {
            "type": "string"
          }
        }
      },
      "required": [
        "framework",
        "renderer_id",
        "input_paths",
        "parameters_redacted"
      ]
    },
    "ScanSettings": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "categories": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Category"
          }
        },
        "secret_scope": {
          "type": "string",
          "enum": [
            "checkout",
            "history"
          ]
        },
        "template_mode": {
          "type": "string",
          "enum": [
            "source",
            "rendered",
            "both"
          ]
        },
        "schema_mode": {
          "type": "string",
          "enum": [
            "automatic",
            "user_supplied",
            "both"
          ]
        },
        "schema_bindings": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/SchemaBinding"
          }
        },
        "renderers": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/RendererSetting"
          }
        },
        "allow_reference_context": {
          "type": "boolean"
        },
        "adapter_ids": {
          "type": "array",
          "items": {
            "type": "string"
          }
        }
      },
      "required": [
        "categories",
        "secret_scope",
        "template_mode",
        "schema_mode",
        "schema_bindings",
        "renderers",
        "allow_reference_context",
        "adapter_ids"
      ]
    },
    "FileRecord": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "file_id": {
          "$ref": "#/$defs/Id"
        },
        "path": {
          "$ref": "#/$defs/RelativePath"
        },
        "byte_size": {
          "type": "integer",
          "minimum": 0
        },
        "content_digest": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        },
        "commit_oid": {
          "$ref": "#/$defs/GitOid"
        },
        "detected_framework": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "discovery_state": {
          "type": "string",
          "enum": [
            "eligible",
            "excluded",
            "unreadable"
          ]
        },
        "reason": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "file_id",
        "path",
        "byte_size",
        "content_digest",
        "commit_oid",
        "detected_framework",
        "discovery_state",
        "reason"
      ]
    },
    "Repository": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "repo_id": {
          "$ref": "#/$defs/Id"
        },
        "display_name": {
          "type": "string"
        },
        "slug": {
          "type": "string"
        },
        "remote_url": {
          "type": "string"
        },
        "canonical_remote": {
          "type": "string"
        },
        "local_path": {
          "type": "string"
        },
        "default_ref_kind": {
          "type": "string",
          "enum": [
            "branch",
            "tag",
            "commit"
          ]
        },
        "default_ref_value": {
          "type": "string"
        },
        "registered_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "last_update_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "last_commit_oid": {
          "anyOf": [
            {
              "$ref": "#/$defs/GitOid"
            },
            {
              "type": "null"
            }
          ]
        },
        "checkout_state": {
          "type": "string",
          "enum": [
            "not_cloned",
            "ready",
            "dirty",
            "updating",
            "error"
          ]
        },
        "last_error_redacted": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "inventory_revision": {
          "type": "integer",
          "minimum": 0
        },
        "inventory_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "files": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/FileRecord"
          }
        }
      },
      "required": [
        "repo_id",
        "display_name",
        "slug",
        "remote_url",
        "canonical_remote",
        "local_path",
        "default_ref_kind",
        "default_ref_value",
        "registered_at",
        "last_update_at",
        "last_commit_oid",
        "checkout_state",
        "last_error_redacted",
        "inventory_revision",
        "inventory_at",
        "files"
      ]
    },
    "Schedule": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "kind": {
          "type": "string",
          "enum": [
            "manual",
            "interval",
            "daily"
          ]
        },
        "interval_seconds": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 1
            },
            {
              "type": "null"
            }
          ]
        },
        "daily_local_time": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "timezone": {
          "type": "string"
        },
        "next_due_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "overlap_policy": {
          "type": "string",
          "enum": [
            "skip_if_running"
          ]
        },
        "missed_policy": {
          "type": "string",
          "enum": [
            "record_no_replay"
          ]
        }
      },
      "required": [
        "kind",
        "interval_seconds",
        "daily_local_time",
        "timezone",
        "next_due_at",
        "overlap_policy",
        "missed_policy"
      ]
    },
    "Workflow": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "workflow_id": {
          "$ref": "#/$defs/Id"
        },
        "revision": {
          "type": "integer",
          "minimum": 1
        },
        "name": {
          "type": "string"
        },
        "description": {
          "type": "string"
        },
        "enabled": {
          "type": "boolean"
        },
        "created_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "targets": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/RepoTarget"
          }
        },
        "scan_settings": {
          "$ref": "#/$defs/ScanSettings"
        },
        "schedule": {
          "$ref": "#/$defs/Schedule"
        },
        "approval_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "output_formats": {
          "type": "array",
          "items": {
            "type": "string",
            "enum": [
              "json",
              "md",
              "pdf"
            ]
          },
          "uniqueItems": true,
          "minItems": 1
        }
      },
      "required": [
        "workflow_id",
        "revision",
        "name",
        "description",
        "enabled",
        "created_at",
        "updated_at",
        "targets",
        "scan_settings",
        "schedule",
        "approval_ids",
        "output_formats"
      ]
    },
    "Capability": {
      "type": "string",
      "enum": [
        "git_acquire",
        "git_update",
        "external_reference",
        "schema_fetch",
        "vulnerability_lookup",
        "documentation_fetch",
        "model_transmission",
        "embedding_transmission",
        "local_render",
        "non_mutating_verification",
        "app_storage"
      ]
    },
    "Approval": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "approval_id": {
          "$ref": "#/$defs/Id"
        },
        "status": {
          "type": "string",
          "enum": [
            "pending",
            "approved",
            "denied",
            "cancelled",
            "revoked",
            "expired"
          ]
        },
        "capability": {
          "$ref": "#/$defs/Capability"
        },
        "workflow_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "workflow_revision": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 1
            },
            {
              "type": "null"
            }
          ]
        },
        "run_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "repo_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "purpose": {
          "type": "string"
        },
        "operation_fingerprint": {
          "$ref": "#/$defs/Digest"
        },
        "destinations": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "resource_constraints": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "data_categories": {
          "type": "array",
          "items": {
            "type": "string",
            "enum": [
              "repository_metadata",
              "redacted_source",
              "redacted_findings",
              "schema_documents",
              "public_artifact_identifiers",
              "redacted_chat_context",
              "app_metadata"
            ]
          }
        },
        "lifetime": {
          "type": "string",
          "enum": [
            "once",
            "run",
            "workflow_revision"
          ]
        },
        "requested_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "decided_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "expires_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "actor": {
          "type": "string"
        },
        "consumed": {
          "type": "boolean"
        },
        "inference_scope": {
          "anyOf": [
            {
              "$ref": "#/$defs/InferenceScope"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "approval_id",
        "status",
        "capability",
        "workflow_id",
        "workflow_revision",
        "run_id",
        "repo_ids",
        "purpose",
        "operation_fingerprint",
        "destinations",
        "resource_constraints",
        "data_categories",
        "lifetime",
        "requested_at",
        "decided_at",
        "expires_at",
        "actor",
        "consumed",
        "inference_scope"
      ]
    },
    "SourceRef": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "source_id": {
          "$ref": "#/$defs/Id"
        },
        "kind": {
          "type": "string",
          "enum": [
            "repository_file",
            "rendered_artifact",
            "scan_report",
            "activity_log",
            "workflow",
            "rule_document",
            "schema_document",
            "external_reference"
          ]
        },
        "repo_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "commit_oid": {
          "anyOf": [
            {
              "$ref": "#/$defs/GitOid"
            },
            {
              "type": "null"
            }
          ]
        },
        "run_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "file_path": {
          "anyOf": [
            {
              "$ref": "#/$defs/RelativePath"
            },
            {
              "type": "null"
            }
          ]
        },
        "artifact_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "document_index": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 1
            },
            {
              "type": "null"
            }
          ]
        },
        "line_start": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 1
            },
            {
              "type": "null"
            }
          ]
        },
        "line_end": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 1
            },
            {
              "type": "null"
            }
          ]
        },
        "resource_identifier": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "content_digest": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        },
        "retrieved_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "approval_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "redaction_version": {
          "type": "string"
        },
        "stale": {
          "type": "boolean"
        }
      },
      "required": [
        "source_id",
        "kind",
        "repo_id",
        "commit_oid",
        "run_id",
        "file_path",
        "artifact_id",
        "document_index",
        "line_start",
        "line_end",
        "resource_identifier",
        "content_digest",
        "retrieved_at",
        "approval_id",
        "redaction_version",
        "stale"
      ]
    },
    "Engine": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "adapter_id": {
          "type": "string"
        },
        "adapter_version": {
          "type": "string"
        },
        "tool_name": {
          "type": "string"
        },
        "tool_version": {
          "type": "string"
        },
        "ruleset_version": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "data_updated_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "adapter_id",
        "adapter_version",
        "tool_name",
        "tool_version",
        "ruleset_version",
        "data_updated_at"
      ]
    },
    "Check": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "check_id": {
          "$ref": "#/$defs/Id"
        },
        "task_id": {
          "$ref": "#/$defs/Id"
        },
        "file_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "category": {
          "$ref": "#/$defs/Category"
        },
        "framework": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "engine": {
          "$ref": "#/$defs/Engine"
        },
        "outcome": {
          "$ref": "#/$defs/Outcome"
        },
        "reason_redacted": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "started_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "finished_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "finding_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "source_refs": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/SourceRef"
          }
        },
        "adapter_id": {
          "type": "string"
        },
        "config_digest": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        },
        "manifest_digest": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        },
        "exit_code": {
          "anyOf": [
            {
              "type": "integer"
            },
            {
              "type": "null"
            }
          ]
        },
        "term_signal": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 1
            },
            {
              "type": "null"
            }
          ]
        },
        "native_status": {
          "type": "string",
          "enum": [
            "not_started",
            "exited",
            "timed_out",
            "cancelled",
            "launch_failed",
            "output_limit_exceeded"
          ]
        },
        "approval_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        }
      },
      "required": [
        "check_id",
        "task_id",
        "file_id",
        "category",
        "framework",
        "engine",
        "outcome",
        "reason_redacted",
        "started_at",
        "finished_at",
        "finding_ids",
        "source_refs",
        "adapter_id",
        "config_digest",
        "manifest_digest",
        "exit_code",
        "term_signal",
        "native_status",
        "approval_ids"
      ]
    },
    "Finding": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "finding_id": {
          "$ref": "#/$defs/Id"
        },
        "fingerprint": {
          "$ref": "#/$defs/Digest"
        },
        "check_id": {
          "$ref": "#/$defs/Id"
        },
        "task_id": {
          "$ref": "#/$defs/Id"
        },
        "repo_id": {
          "$ref": "#/$defs/Id"
        },
        "run_id": {
          "$ref": "#/$defs/Id"
        },
        "commit_oid": {
          "$ref": "#/$defs/GitOid"
        },
        "category": {
          "$ref": "#/$defs/Category"
        },
        "severity": {
          "$ref": "#/$defs/Severity"
        },
        "confidence": {
          "type": "string",
          "enum": [
            "high",
            "medium",
            "low",
            "unknown"
          ]
        },
        "rule_id": {
          "type": "string"
        },
        "title": {
          "type": "string"
        },
        "description_redacted": {
          "type": "string"
        },
        "evidence_redacted": {
          "type": "string"
        },
        "remediation_redacted": {
          "type": "string"
        },
        "engine": {
          "$ref": "#/$defs/Engine"
        },
        "sources": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/SourceRef"
          }
        },
        "review_status": {
          "type": "string",
          "enum": [
            "new",
            "existing",
            "suppressed"
          ]
        },
        "suppression_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "observed_at": {
          "$ref": "#/$defs/Timestamp"
        }
      },
      "required": [
        "finding_id",
        "fingerprint",
        "check_id",
        "task_id",
        "repo_id",
        "run_id",
        "commit_oid",
        "category",
        "severity",
        "confidence",
        "rule_id",
        "title",
        "description_redacted",
        "evidence_redacted",
        "remediation_redacted",
        "engine",
        "sources",
        "review_status",
        "suppression_id",
        "observed_at"
      ]
    },
    "Artifact": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "artifact_id": {
          "$ref": "#/$defs/Id"
        },
        "kind": {
          "type": "string",
          "enum": [
            "rendered",
            "external",
            "schema",
            "rule_document"
          ]
        },
        "display_name": {
          "type": "string"
        },
        "content_digest": {
          "$ref": "#/$defs/Digest"
        },
        "retained_path": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "source_refs": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/SourceRef"
          }
        },
        "approval_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "contains_only_redacted_retained_content": {
          "type": "boolean"
        }
      },
      "required": [
        "artifact_id",
        "kind",
        "display_name",
        "content_digest",
        "retained_path",
        "source_refs",
        "approval_id",
        "contains_only_redacted_retained_content"
      ]
    },
    "CoverageSummary": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "discovered_files": {
          "type": "integer",
          "minimum": 0
        },
        "selected_files": {
          "type": "integer",
          "minimum": 0
        },
        "assessed_files": {
          "type": "integer",
          "minimum": 0
        },
        "excluded_files": {
          "type": "integer",
          "minimum": 0
        },
        "missing_files": {
          "type": "integer",
          "minimum": 0
        },
        "checks_passed": {
          "type": "integer",
          "minimum": 0
        },
        "checks_with_findings": {
          "type": "integer",
          "minimum": 0
        },
        "checks_failed": {
          "type": "integer",
          "minimum": 0
        },
        "checks_unsupported": {
          "type": "integer",
          "minimum": 0
        },
        "checks_skipped": {
          "type": "integer",
          "minimum": 0
        },
        "checks_incomplete": {
          "type": "integer",
          "minimum": 0
        },
        "checks_cancelled": {
          "type": "integer",
          "minimum": 0
        },
        "coverage": {
          "$ref": "#/$defs/Coverage"
        },
        "assessment": {
          "$ref": "#/$defs/Assessment"
        }
      },
      "required": [
        "discovered_files",
        "selected_files",
        "assessed_files",
        "excluded_files",
        "missing_files",
        "checks_passed",
        "checks_with_findings",
        "checks_failed",
        "checks_unsupported",
        "checks_skipped",
        "checks_incomplete",
        "checks_cancelled",
        "coverage",
        "assessment"
      ]
    },
    "PlanSnapshot": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "workflow_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "workflow_revision": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 1
            },
            {
              "type": "null"
            }
          ]
        },
        "targets": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/RepoTarget"
          }
        },
        "scan_settings": {
          "$ref": "#/$defs/ScanSettings"
        },
        "approval_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "created_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "plan_digest": {
          "$ref": "#/$defs/Digest"
        },
        "target_snapshots": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/TargetSnapshot"
          }
        },
        "engine_snapshots": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/EngineSnapshot"
          }
        }
      },
      "required": [
        "workflow_id",
        "workflow_revision",
        "targets",
        "scan_settings",
        "approval_ids",
        "created_at",
        "plan_digest",
        "target_snapshots",
        "engine_snapshots"
      ]
    },
    "RunSummary": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "run_id": {
          "$ref": "#/$defs/Id"
        },
        "trigger": {
          "type": "string",
          "enum": [
            "manual",
            "interval",
            "daily"
          ]
        },
        "execution": {
          "$ref": "#/$defs/Execution"
        },
        "created_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "started_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "finished_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "plan": {
          "$ref": "#/$defs/PlanSnapshot"
        },
        "coverage": {
          "$ref": "#/$defs/CoverageSummary"
        },
        "report_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "last_checkpoint_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "run_id",
        "trigger",
        "execution",
        "created_at",
        "started_at",
        "finished_at",
        "plan",
        "coverage",
        "report_ids",
        "last_checkpoint_at"
      ]
    },
    "Event": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "event_id": {
          "$ref": "#/$defs/Id"
        },
        "at": {
          "$ref": "#/$defs/Timestamp"
        },
        "level": {
          "type": "string",
          "enum": [
            "debug",
            "info",
            "warning",
            "error"
          ]
        },
        "event_type": {
          "type": "string"
        },
        "run_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "repo_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "workflow_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "task_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "approval_id": {
          "anyOf": [
            {
              "$ref": "#/$defs/Id"
            },
            {
              "type": "null"
            }
          ]
        },
        "message_redacted": {
          "type": "string"
        },
        "code": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "event_id",
        "at",
        "level",
        "event_type",
        "run_id",
        "repo_id",
        "workflow_id",
        "task_id",
        "approval_id",
        "message_redacted",
        "code"
      ]
    },
    "ReportManifest": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "report_id": {
          "$ref": "#/$defs/Id"
        },
        "run_id": {
          "$ref": "#/$defs/Id"
        },
        "repo_id": {
          "$ref": "#/$defs/Id"
        },
        "json_path": {
          "type": "string"
        },
        "md_path": {
          "type": "string"
        },
        "json_written": {
          "type": "boolean"
        },
        "md_written": {
          "type": "boolean"
        },
        "json_digest": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        },
        "generated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "pdf_path": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "pdf_written": {
          "type": "boolean"
        },
        "pdf_digest": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        },
        "pdf_error_redacted": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "pdf_renderer_version": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "report_id",
        "run_id",
        "repo_id",
        "json_path",
        "md_path",
        "json_written",
        "md_written",
        "json_digest",
        "generated_at",
        "pdf_path",
        "pdf_written",
        "pdf_digest",
        "pdf_error_redacted",
        "pdf_renderer_version"
      ]
    },
    "Suppression": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "suppression_id": {
          "$ref": "#/$defs/Id"
        },
        "repo_id": {
          "$ref": "#/$defs/Id"
        },
        "finding_fingerprint": {
          "$ref": "#/$defs/Digest"
        },
        "rule_id": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        },
        "created_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "expires_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "actor": {
          "type": "string"
        },
        "enabled": {
          "type": "boolean"
        }
      },
      "required": [
        "suppression_id",
        "repo_id",
        "finding_fingerprint",
        "rule_id",
        "reason",
        "created_at",
        "expires_at",
        "actor",
        "enabled"
      ]
    },
    "Chunk": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "chunk_id": {
          "$ref": "#/$defs/Id"
        },
        "source": {
          "$ref": "#/$defs/SourceRef"
        },
        "text_redacted": {
          "type": "string"
        },
        "content_digest": {
          "$ref": "#/$defs/Digest"
        },
        "redaction_version": {
          "type": "string"
        },
        "embedding_key": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "indexed_at": {
          "$ref": "#/$defs/Timestamp"
        }
      },
      "required": [
        "chunk_id",
        "source",
        "text_redacted",
        "content_digest",
        "redaction_version",
        "embedding_key",
        "indexed_at"
      ]
    },
    "ChatContext": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "repo_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "run_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "finding_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "file_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "include_history": {
          "type": "boolean"
        }
      },
      "required": [
        "repo_ids",
        "run_ids",
        "finding_ids",
        "file_ids",
        "include_history"
      ]
    },
    "Message": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "message_id": {
          "$ref": "#/$defs/Id"
        },
        "role": {
          "type": "string",
          "enum": [
            "user",
            "assistant",
            "system_notice"
          ]
        },
        "text_redacted": {
          "type": "string"
        },
        "source_refs": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/SourceRef"
          }
        },
        "provider_id": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "model_id": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "approval_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "created_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "status": {
          "type": "string",
          "enum": [
            "complete",
            "cancelled",
            "failed"
          ]
        },
        "tokens_in": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 0
            },
            {
              "type": "null"
            }
          ]
        },
        "tokens_out": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 0
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "message_id",
        "role",
        "text_redacted",
        "source_refs",
        "provider_id",
        "model_id",
        "approval_ids",
        "created_at",
        "status",
        "tokens_in",
        "tokens_out"
      ]
    },
    "Conversation": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "conversation_id": {
          "$ref": "#/$defs/Id"
        },
        "title": {
          "type": "string"
        },
        "context": {
          "$ref": "#/$defs/ChatContext"
        },
        "created_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "messages": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Message"
          }
        }
      },
      "required": [
        "conversation_id",
        "title",
        "context",
        "created_at",
        "updated_at",
        "messages"
      ]
    },
    "Provider": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "provider_id": {
          "const": "openrouter"
        },
        "kind": {
          "const": "api"
        },
        "endpoint": {
          "const": "https://openrouter.ai/api/v1"
        },
        "model_id": {
          "type": "string",
          "minLength": 1
        },
        "credential_mode": {
          "type": "string",
          "enum": [
            "session_only",
            "os_credential_store"
          ]
        },
        "credential_reference": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "routing_policy": {
          "$ref": "#/$defs/ProviderPolicy"
        }
      },
      "required": [
        "provider_id",
        "kind",
        "endpoint",
        "model_id",
        "credential_mode",
        "credential_reference",
        "routing_policy"
      ]
    },
    "Config": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "app_data_root": {
          "type": "string"
        },
        "backup_directory": {
          "type": "string"
        },
        "active_theme": {
          "type": "string"
        },
        "max_active_repositories": {
          "type": "integer",
          "minimum": 1
        },
        "max_scanner_processes": {
          "type": "integer",
          "minimum": 1
        },
        "history_retention_days": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 1
            },
            {
              "type": "null"
            }
          ]
        },
        "chat_persistence_enabled": {
          "type": "boolean"
        },
        "provider": {
          "anyOf": [
            {
              "$ref": "#/$defs/Provider"
            },
            {
              "type": "null"
            }
          ]
        },
        "telemetry_enabled": {
          "const": false
        },
        "redaction_version": {
          "type": "string"
        }
      },
      "required": [
        "app_data_root",
        "backup_directory",
        "active_theme",
        "max_active_repositories",
        "max_scanner_processes",
        "history_retention_days",
        "chat_persistence_enabled",
        "provider",
        "telemetry_enabled",
        "redaction_version"
      ]
    },
    "ConfigFile": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema_version": {
          "const": "1.3"
        },
        "store_type": {
          "const": "config"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "config": {
          "$ref": "#/$defs/Config"
        }
      },
      "required": [
        "schema_version",
        "store_type",
        "updated_at",
        "config"
      ]
    },
    "RepositoryFile": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema_version": {
          "const": "1.3"
        },
        "store_type": {
          "const": "repositories"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "repositories": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Repository"
          }
        }
      },
      "required": [
        "schema_version",
        "store_type",
        "updated_at",
        "repositories"
      ]
    },
    "WorkflowFile": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema_version": {
          "const": "1.3"
        },
        "store_type": {
          "const": "workflows"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "workflows": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Workflow"
          }
        }
      },
      "required": [
        "schema_version",
        "store_type",
        "updated_at",
        "workflows"
      ]
    },
    "ApprovalFile": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema_version": {
          "const": "1.3"
        },
        "store_type": {
          "const": "approvals"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "approvals": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Approval"
          }
        }
      },
      "required": [
        "schema_version",
        "store_type",
        "updated_at",
        "approvals"
      ]
    },
    "LogFile": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema_version": {
          "const": "1.3"
        },
        "store_type": {
          "const": "log"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "runs": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/RunSummary"
          }
        },
        "events": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Event"
          }
        },
        "reports": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/ReportManifest"
          }
        }
      },
      "required": [
        "schema_version",
        "store_type",
        "updated_at",
        "runs",
        "events",
        "reports"
      ]
    },
    "SuppressionFile": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema_version": {
          "const": "1.3"
        },
        "store_type": {
          "const": "suppressions"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "suppressions": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Suppression"
          }
        }
      },
      "required": [
        "schema_version",
        "store_type",
        "updated_at",
        "suppressions"
      ]
    },
    "RagIndexFile": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema_version": {
          "const": "1.3"
        },
        "store_type": {
          "const": "rag_index"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "chunks": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Chunk"
          }
        }
      },
      "required": [
        "schema_version",
        "store_type",
        "updated_at",
        "chunks"
      ]
    },
    "ConversationFile": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema_version": {
          "const": "1.3"
        },
        "store_type": {
          "const": "conversations"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "conversations": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Conversation"
          }
        }
      },
      "required": [
        "schema_version",
        "store_type",
        "updated_at",
        "conversations"
      ]
    },
    "ReportFile": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema_version": {
          "const": "1.3"
        },
        "store_type": {
          "const": "report"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "report_id": {
          "$ref": "#/$defs/Id"
        },
        "run_id": {
          "$ref": "#/$defs/Id"
        },
        "repo_id": {
          "$ref": "#/$defs/Id"
        },
        "repo_name": {
          "type": "string"
        },
        "remote_identifier_redacted": {
          "type": "string"
        },
        "selected_ref": {
          "type": "string"
        },
        "commit_oid": {
          "anyOf": [
            {
              "$ref": "#/$defs/GitOid"
            },
            {
              "type": "null"
            }
          ]
        },
        "execution": {
          "$ref": "#/$defs/Execution"
        },
        "plan": {
          "$ref": "#/$defs/PlanSnapshot"
        },
        "generated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "coverage": {
          "$ref": "#/$defs/CoverageSummary"
        },
        "checks": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Check"
          }
        },
        "findings": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Finding"
          }
        },
        "artifacts": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Artifact"
          }
        },
        "limitations": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "ai_enrichment": {
          "$ref": "#/$defs/AIEnrichment"
        }
      },
      "required": [
        "schema_version",
        "store_type",
        "updated_at",
        "report_id",
        "run_id",
        "repo_id",
        "repo_name",
        "remote_identifier_redacted",
        "selected_ref",
        "commit_oid",
        "execution",
        "plan",
        "generated_at",
        "coverage",
        "checks",
        "findings",
        "artifacts",
        "limitations",
        "ai_enrichment"
      ]
    },
    "ProviderPolicy": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "data_collection": {
          "const": "deny"
        },
        "zdr": {
          "const": true
        },
        "allow_fallbacks": {
          "const": false
        },
        "only": {
          "type": "array",
          "items": {
            "type": "string"
          }
        }
      },
      "required": [
        "data_collection",
        "zdr",
        "allow_fallbacks",
        "only"
      ]
    },
    "InferenceScope": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "provider_id": {
          "const": "openrouter"
        },
        "model_id": {
          "type": "string"
        },
        "routing_policy": {
          "$ref": "#/$defs/ProviderPolicy"
        },
        "source_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "redaction_version": {
          "type": "string"
        }
      },
      "required": [
        "provider_id",
        "model_id",
        "routing_policy",
        "source_ids",
        "redaction_version"
      ]
    },
    "AIEnrichment": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "status": {
          "type": "string",
          "enum": [
            "not_requested",
            "completed",
            "failed",
            "unavailable",
            "denied"
          ]
        },
        "provider_id": {
          "anyOf": [
            {
              "const": "openrouter"
            },
            {
              "type": "null"
            }
          ]
        },
        "requested_model": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "actual_model": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "actual_provider": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "approval_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "source_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "routing_policy": {
          "anyOf": [
            {
              "$ref": "#/$defs/ProviderPolicy"
            },
            {
              "type": "null"
            }
          ]
        },
        "redaction_version": {
          "type": "string"
        },
        "generated_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        },
        "narrative_redacted": {
          "type": "string"
        },
        "reason_redacted": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "status",
        "provider_id",
        "requested_model",
        "actual_model",
        "actual_provider",
        "approval_ids",
        "source_ids",
        "routing_policy",
        "redaction_version",
        "generated_at",
        "narrative_redacted",
        "reason_redacted"
      ]
    },
    "TargetSnapshot": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "repo_id": {
          "$ref": "#/$defs/Id"
        },
        "selected_ref": {
          "type": "string"
        },
        "commit_oid": {
          "anyOf": [
            {
              "$ref": "#/$defs/GitOid"
            },
            {
              "type": "null"
            }
          ]
        },
        "inventory_revision": {
          "type": "integer",
          "minimum": 0
        },
        "selected_files": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/FileRecord"
          }
        },
        "missing_paths": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/RelativePath"
          }
        },
        "snapshot_at": {
          "$ref": "#/$defs/Timestamp"
        }
      },
      "required": [
        "repo_id",
        "selected_ref",
        "commit_oid",
        "inventory_revision",
        "selected_files",
        "missing_paths",
        "snapshot_at"
      ]
    },
    "EngineSnapshot": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "adapter_id": {
          "type": "string"
        },
        "adapter_version": {
          "type": "string"
        },
        "tool_name": {
          "type": "string"
        },
        "tool_version": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "ruleset_version": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "config_digest": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        },
        "manifest_digest": {
          "$ref": "#/$defs/Digest"
        },
        "availability": {
          "type": "string",
          "enum": [
            "ready",
            "missing",
            "incompatible",
            "deferred",
            "unconfigured"
          ]
        },
        "data_updated_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "adapter_id",
        "adapter_version",
        "tool_name",
        "tool_version",
        "ruleset_version",
        "config_digest",
        "manifest_digest",
        "availability",
        "data_updated_at"
      ]
    },
    "LicenseRecord": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "spdx": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "verified_version": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "review_status": {
          "type": "string",
          "enum": [
            "pending",
            "approved",
            "blocked"
          ]
        },
        "license_file": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "notice_files": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "source_obligation_note": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "reviewed_at": {
          "anyOf": [
            {
              "$ref": "#/$defs/Timestamp"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "spdx",
        "verified_version",
        "review_status",
        "license_file",
        "notice_files",
        "source_obligation_note",
        "reviewed_at"
      ]
    },
    "ExitMeaning": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "code": {
          "type": "integer"
        },
        "meaning": {
          "type": "string",
          "enum": [
            "inspection_required",
            "execution_error",
            "findings_possible",
            "no_findings_when_output_confirms"
          ]
        }
      },
      "required": [
        "code",
        "meaning"
      ]
    },
    "ToolManifest": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "adapter_id": {
          "type": "string"
        },
        "adapter_version": {
          "type": "string"
        },
        "tool_name": {
          "type": "string"
        },
        "tool_version": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "role": {
          "type": "string",
          "enum": [
            "runtime",
            "development",
            "optional_helper"
          ]
        },
        "release_tier": {
          "type": "string",
          "enum": [
            "foundation",
            "pilot_baseline",
            "pilot_framework",
            "recommended_extension",
            "conditional",
            "deferred"
          ]
        },
        "availability": {
          "type": "string",
          "enum": [
            "ready",
            "missing",
            "incompatible",
            "deferred",
            "unconfigured"
          ]
        },
        "capabilities": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Category"
          }
        },
        "frameworks": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "platforms": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "wrapper_name": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "invocation_mode": {
          "type": "string",
          "enum": [
            "native",
            "shell",
            "in_process"
          ]
        },
        "command_policy_id": {
          "type": "string"
        },
        "parser_id": {
          "type": "string"
        },
        "native_format": {
          "type": "string"
        },
        "config_digest": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        },
        "selection_strategy": {
          "type": "string",
          "enum": [
            "explicit_files",
            "staged_selection",
            "approved_wider_context",
            "in_process"
          ]
        },
        "network_policy": {
          "type": "string",
          "enum": [
            "none",
            "cache_only",
            "approval_required"
          ]
        },
        "network_resources": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "exit_mapping": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/ExitMeaning"
          }
        },
        "timeout_seconds": {
          "type": "integer",
          "minimum": 1
        },
        "max_output_bytes": {
          "type": "integer",
          "minimum": 1
        },
        "distribution": {
          "type": "string",
          "enum": [
            "bundled",
            "system_prerequisite",
            "not_shipped"
          ]
        },
        "binary_origin": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "binary_sha256": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        },
        "license": {
          "$ref": "#/$defs/LicenseRecord"
        },
        "limitations": {
          "type": "array",
          "items": {
            "type": "string"
          }
        }
      },
      "required": [
        "adapter_id",
        "adapter_version",
        "tool_name",
        "tool_version",
        "role",
        "release_tier",
        "availability",
        "capabilities",
        "frameworks",
        "platforms",
        "wrapper_name",
        "invocation_mode",
        "command_policy_id",
        "parser_id",
        "native_format",
        "config_digest",
        "selection_strategy",
        "network_policy",
        "network_resources",
        "exit_mapping",
        "timeout_seconds",
        "max_output_bytes",
        "distribution",
        "binary_origin",
        "binary_sha256",
        "license",
        "limitations"
      ]
    },
    "ToolManifestFile": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema_version": {
          "const": "1.3"
        },
        "store_type": {
          "const": "tool_manifest"
        },
        "updated_at": {
          "$ref": "#/$defs/Timestamp"
        },
        "tools": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/ToolManifest"
          }
        }
      },
      "required": [
        "schema_version",
        "store_type",
        "updated_at",
        "tools"
      ]
    },
    "WrapperRequest": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "protocol_version": {
          "const": "1.1"
        },
        "request_id": {
          "$ref": "#/$defs/Id"
        },
        "task_id": {
          "$ref": "#/$defs/Id"
        },
        "run_id": {
          "$ref": "#/$defs/Id"
        },
        "repo_id": {
          "$ref": "#/$defs/Id"
        },
        "adapter_id": {
          "type": "string"
        },
        "category": {
          "$ref": "#/$defs/Category"
        },
        "required_capabilities": {
          "type": "array",
          "uniqueItems": true,
          "items": {
            "$ref": "#/$defs/Capability"
          }
        },
        "manifest_digest": {
          "$ref": "#/$defs/Digest"
        },
        "checkout_root": {
          "type": "string"
        },
        "selected_files": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/FileRecord"
          }
        },
        "approved_context_paths": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "approval_ids": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Id"
          }
        },
        "network_resources": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "config_digest": {
          "anyOf": [
            {
              "$ref": "#/$defs/Digest"
            },
            {
              "type": "null"
            }
          ]
        },
        "timeout_seconds": {
          "type": "integer",
          "minimum": 1
        },
        "max_output_bytes": {
          "type": "integer",
          "minimum": 1
        },
        "termination_policy": {
          "$ref": "#/$defs/TerminationPolicy"
        }
      },
      "required": [
        "protocol_version",
        "request_id",
        "task_id",
        "run_id",
        "repo_id",
        "adapter_id",
        "category",
        "required_capabilities",
        "manifest_digest",
        "checkout_root",
        "selected_files",
        "approved_context_paths",
        "approval_ids",
        "network_resources",
        "config_digest",
        "timeout_seconds",
        "max_output_bytes",
        "termination_policy"
      ]
    },
    "WrapperControl": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "protocol_version": {
          "const": "1.1"
        },
        "request_id": {
          "$ref": "#/$defs/Id"
        },
        "task_id": {
          "$ref": "#/$defs/Id"
        },
        "adapter_id": {
          "type": "string"
        },
        "tool_version": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "transport_status": {
          "type": "string",
          "enum": [
            "completed",
            "launch_failed",
            "timed_out",
            "cancelled",
            "invalid_output",
            "output_limit_exceeded"
          ]
        },
        "tool_exit_code": {
          "anyOf": [
            {
              "type": "integer"
            },
            {
              "type": "null"
            }
          ]
        },
        "tool_term_signal": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 1
            },
            {
              "type": "null"
            }
          ]
        },
        "termination": {
          "$ref": "#/$defs/TerminationRecord"
        },
        "output_streams": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "stdout": {
              "$ref": "#/$defs/StreamRecord"
            },
            "stderr": {
              "$ref": "#/$defs/StreamRecord"
            }
          },
          "required": [
            "stdout",
            "stderr"
          ]
        },
        "native_format": {
          "type": "string"
        },
        "output_channel_id": {
          "anyOf": [
            {
              "type": "string"
            },
            {
              "type": "null"
            }
          ]
        },
        "diagnostics_redacted": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "elapsed_ms": {
          "type": "integer",
          "minimum": 0
        }
      },
      "required": [
        "protocol_version",
        "request_id",
        "task_id",
        "adapter_id",
        "tool_version",
        "transport_status",
        "tool_exit_code",
        "tool_term_signal",
        "termination",
        "output_streams",
        "native_format",
        "output_channel_id",
        "diagnostics_redacted",
        "elapsed_ms"
      ]
    },
    "TerminationPolicy": {
      "$comment": "Declarative only. Timeout and cancellation are enforced out-of-band by the core process supervisor; this record does not itself stop a running process.",
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "control": {
          "const": "supervisor_out_of_band"
        },
        "grace_ms": {
          "type": "integer",
          "minimum": 0
        },
        "kill_scope": {
          "const": "process_tree"
        }
      },
      "required": [
        "control",
        "grace_ms",
        "kill_scope"
      ]
    },
    "TerminationRecord": {
      "$comment": "reason is the primary supervisor trigger and is never overwritten by later descendant cleanup; descendant_cleanup records that action separately. 'descendant_cleanup' as reason means the child exited on its own and cleanup was the only supervisor action. process_tree_exited is null when undeterminable.",
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "reason": {
          "type": "string",
          "enum": [
            "none",
            "timeout",
            "cancel_requested",
            "output_limit",
            "supervisor_shutdown",
            "descendant_cleanup"
          ]
        },
        "method": {
          "type": "string",
          "enum": [
            "not_required",
            "graceful",
            "forced",
            "failed"
          ]
        },
        "process_tree_exited": {
          "anyOf": [
            {
              "type": "boolean"
            },
            {
              "type": "null"
            }
          ]
        },
        "descendant_cleanup": {
          "$ref": "#/$defs/DescendantCleanup"
        }
      },
      "required": [
        "reason",
        "method",
        "process_tree_exited",
        "descendant_cleanup"
      ]
    },
    "DescendantCleanup": {
      "$comment": "Supervisor action against members of the tool's process group that remained after the direct child exited or was terminated. Does not cover descendants that left the group/session.",
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "performed": {
          "type": "boolean"
        },
        "method": {
          "type": "string",
          "enum": [
            "not_required",
            "graceful",
            "forced",
            "failed"
          ]
        },
        "leftover_live_count": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 0
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "performed",
        "method",
        "leftover_live_count"
      ]
    },
    "StreamRecord": {
      "$comment": "Per-stream reader outcome. bytes_consumed = bytes read by the supervisor; bytes_retained = bytes kept in memory for parsing/evidence. Child-generated bytes beyond what was consumed are unknown. not_applicable = no reader existed (e.g. spawn failure); counts are then null.",
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "state": {
          "type": "string",
          "enum": [
            "not_applicable",
            "complete",
            "truncated",
            "reader_timeout"
          ]
        },
        "bytes_consumed": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 0
            },
            {
              "type": "null"
            }
          ]
        },
        "bytes_retained": {
          "anyOf": [
            {
              "type": "integer",
              "minimum": 0
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "state",
        "bytes_consumed",
        "bytes_retained"
      ]
    }
  }
}
```

## 8. Example `repo.json`

Synthetic registration before acquisition. No credentials are embedded.

```json
{
  "schema_version": "1.3",
  "store_type": "repositories",
  "updated_at": "2026-10-16T05:25:00Z",
  "repositories": [
    {
      "repo_id": "526a9739-68cb-5ef8-8c3a-0fda8a207475",
      "display_name": "example-service",
      "slug": "example-service",
      "remote_url": "https://github.com/example/example-service.git",
      "canonical_remote": "github.com/example/example-service",
      "local_path": "checkouts/526a9739-68cb-5ef8-8c3a-0fda8a207475",
      "default_ref_kind": "branch",
      "default_ref_value": "main",
      "registered_at": "2026-10-16T05:25:00Z",
      "last_update_at": null,
      "last_commit_oid": null,
      "checkout_state": "not_cloned",
      "last_error_redacted": null,
      "inventory_revision": 0,
      "inventory_at": null,
      "files": []
    }
  ]
}
```

## 9. Example `workflows.json`

The example is disabled until targets, installed capability support and approvals are validated. Schedule execution is application-open only.

```json
{
  "schema_version": "1.3",
  "store_type": "workflows",
  "updated_at": "2026-10-16T05:25:00Z",
  "workflows": [
    {
      "workflow_id": "9d4aa1e3-e812-57cd-96ab-bf9c1ab1d62d",
      "revision": 1,
      "name": "Daily configuration review",
      "description": "Example only; framework support depends on installed approved adapters.",
      "enabled": false,
      "created_at": "2026-10-16T05:25:00Z",
      "updated_at": "2026-10-16T05:25:00Z",
      "targets": [
        {
          "repo_id": "526a9739-68cb-5ef8-8c3a-0fda8a207475",
          "ref_kind": "branch",
          "ref_value": "main",
          "selector": {
            "mode": "all_yaml",
            "paths": [],
            "include_patterns": [],
            "exclude_patterns": []
          }
        }
      ],
      "scan_settings": {
        "categories": [
          "syntax",
          "schema",
          "secrets",
          "misconfiguration",
          "vulnerability"
        ],
        "secret_scope": "checkout",
        "template_mode": "both",
        "schema_mode": "both",
        "schema_bindings": [],
        "renderers": [],
        "allow_reference_context": true,
        "adapter_ids": []
      },
      "schedule": {
        "kind": "daily",
        "interval_seconds": null,
        "daily_local_time": "10:00",
        "timezone": "Asia/Kolkata",
        "next_due_at": null,
        "overlap_policy": "skip_if_running",
        "missed_policy": "record_no_replay"
      },
      "approval_ids": [],
      "output_formats": [
        "json",
        "md",
        "pdf"
      ]
    }
  ]
}
```

## 10. Example initial `log.json`

This is ordinary JSON. Do not append newline-delimited events directly to this object; mutate through the single writer and safe replacement path.

```json
{
  "schema_version": "1.3",
  "store_type": "log",
  "updated_at": "2026-10-16T05:25:00Z",
  "runs": [],
  "events": [],
  "reports": []
}
```

## 11. Runtime semantic validation

JSON Schema alone is insufficient. Enforce these invariants in application code:

### IDs and references

1. IDs are unique within their entity domain; embedded check/finding IDs are consistent with run/report IDs.
2. Workflow targets reference registered repos at launch. Historical report snapshots can retain IDs after deregistration.
3. File IDs belong to the indicated repository; report findings/checks cannot silently cross repo scope.
4. `check.finding_ids` resolves to report findings; finding check/task IDs match the originating check.
5. Source refs point to eligible paths/artifacts/reports; cite only retained or resolvable evidence. Stale source IDs remain labelled stale.
6. Local internal paths are confined to allowed app/checkout roots. Remote identifiers must not contain credentials or expose secret query parameters.

### Selectors and schedules

7. `all_yaml` uses empty explicit paths; `explicit` uses approved exact paths; `patterns` uses validated non-shell patterns. Empty selections are allowed in a draft, not a runnable plan.
8. Enabled runnable workflows need at least one valid target/category, installed/acknowledged capabilities and JSON/Markdown/PDF output according to the agreed reporting policy.
9. Interval schedules require positive `interval_seconds`; daily schedules require valid `HH:MM` and IANA timezone. Inapplicable schedule values are null.
10. Newly discovered files join only dynamic selector modes; missing explicit files remain recorded as gaps.
11. Schema resources and renderer IDs are validated against allowlists; renderer parameters cannot represent arbitrary shell commands or contain raw secrets in persisted state.

### Approvals and disclosure

12. Workflow-lifetime grants require workflow ID/revision; run grants require run ID. Once grants become consumed atomically before/reconciled after execution to prevent double use.
13. Denied/revoked/expired/consumed grants never authorise new actions. Scope match includes capability, destinations, resource constraints, data categories and operation fingerprint.
14. Changes to relevant workflow settings invalidate grants even when a stale approval ID remains in a list.
15. Do not treat `operation_fingerprint` as encryption, authorisation by itself, or proof of user intent. It binds a recorded approval to canonical operation data.
16. Raw secret values are prohibited data categories even with broad external permission. Sensitive business information still requires disclosure approval after redaction.
17. Endpoints/redirects and schema/reference fetches require bounded validated destinations, including tool-controlled requests.

### Provenance, outcomes and integrity

18. Timestamps are ordered sensibly; finish is not before start. Null start/finish is valid before execution.
19. Content digest matches the sanitised persisted chunk; source digest may identify original source for freshness without retaining original content in the index.
20. Source line ranges are ordered; missing exact mapping uses null. Git commit may be null only when acquisition failed before a usable snapshot.
21. Coverage counters agree with check records and selected-file snapshots. Do not sum incompatible counts blindly across reports.
22. Findings must have at least one source/evidence reference or an explicit source-unavailable limitation. Reports retain engine/rules/data freshness.
23. Provider credential reference is only an approved locator; never an actual key. Do not persist raw prompts/requests by default.
24. Chat persistence is opt-in, sanitised and subject to context isolation/retention. Embedding references must not reference another repo's unauthorised content.
25. Storage errors are not swallowed; halt new tasks if results cannot be durably checkpointed.

## 12. Persistence and recovery protocol

1. Obtain exclusive app-instance/store lock; readers use immutable snapshots.
2. Prepare typed next-state value, validate shape and semantic invariants, redact and bound data.
3. Write same-directory temporary file, flush according to platform policy, then perform tested replacement. Keep recovery copy/checkpoint as required.
4. JSON report is authoritative; derive Markdown from exactly that report object.
5. Record report manifest only with accurate `json_written`/`md_written`/`pdf_written` status. JSON success plus Markdown/PDF failure is recoverable without rescanning; each export status is independent.
6. Cross-file writes are not one transaction. On startup reconcile log run/report IDs, existing reports and any incomplete temporary files.
7. Mark abandoned queued/running/awaiting-approval runs interrupted; preserve completed evidence. Pending requests do not become approvals on recovery.
8. Compact `log.json` only under explicit retention policy; preserve referential/report metadata needed by retained history. Retention defaults and backup size limit remain open.
9. Store migrations validate old schema, back up, migrate to new schema, validate and commit. Reject unknown newer versions safely.

No implementation should claim crash-proof multi-file ACID semantics from atomic file replacement alone.

## 13. Report naming and immutability

Proposed pattern: `YYYYMMDDTHHMMSSZ_repo-slug_run-id.json` and matching `.md` and `.pdf`.

- Slug is normalised and length-bounded, never a raw path.
- Include full or collision-checked run ID to avoid same-second collision.
- Per-repo reports are immutable after finalisation; regeneration of a missing Markdown/PDF companion uses original JSON and is recorded in history.
- Active runs use checkpoints/partial reports, not a falsely final clean report.
- Optional combined run reports are not required; log run summary joins per-repo report IDs.

## 14. RAG index policy

Initial retrieval may use a local lexical index over stored sanitised chunks. `embedding_key` remains null unless an approved embedding backend is configured. A vector database is not mandated by “RAG.” Any later embedding store must use the same source/context/redaction/version constraints.

Derived chunks can be rebuilt. Source files remain in checkouts and are not copied verbatim into logs. Deleting a repo offers explicit choices for checkout deletion, derived index purge and report retention; deregistration is not a destructive-delete side effect.

## 15. Verification and migration checklist

- Validate each store root against this schema using a Draft 2020-12-compatible validator.
- Enable date/UUID format checking in the chosen validator; do not assume all validators enforce `format` by default.
- Test uniqueness, reference resolution and all runtime rules separately.
- Seed write interruption/full-disk/malformed-JSON cases and verify non-destructive recovery.
- Test same-name repos, same-day reports, Unicode paths, cross-platform separators and stale contexts.
- Audit every text field for secret/control-character leakage, including user messages and scanner diagnostics.
- Version schema, redaction policy and migrations explicitly.

## 16. Open schema decisions

Retention durations, maximum store/index sizes, provider credential backend, final schedule variants, optional suppression release scope and embedding backend remain open. These do not change the agreed requirement that `log.json` holds history and `backup` contains dated per-repository JSON, Markdown and PDF reports.


## 17. Contract versions and new entities

This document is v0.3.3; machine store schema is v1.3; wrapper control protocol is v1.1; the app command/event contract (Architecture §6) is v1.0; the declarative UI schema (UI Schema §5, `ui_schema_version`) is v1.0. Keep these independent; the UI schema version is not the storage-contract version. Pre-implementation amendment CA-01 is recorded in §22. Original stores used v1.0 and the previous specification defined v1.1. Migration is explicit and historical evidence is never silently rewritten.

| Entity | Role / important fields |
|---|---|
| ToolManifest / tools.json | Adapter/tool version, release tier, platform, capabilities, invocation/parser/command policy IDs, scope strategy, network policy, exit mappings, limits, source/checksum and license review |
| EngineSnapshot | Frozen adapter/version/config/manifest/data freshness and availability at planning time |
| TargetSnapshot | Frozen selected files/digests/commit plus missing paths; no reconstruction from current inventory |
| WrapperRequest | Core-validated IDs/scope/config/approved resources/limits; no arbitrary shell command or secret field |
| WrapperControl | Transport/native-exit status, bounded redacted diagnostics and controlled native-result channel identifier; not a clean/dirty verdict |
| Check extension | Adapter/config/manifest provenance, actual native outcome/exit and approval IDs |
| Provider / ProviderPolicy | OpenRouter BYOK mode/reference, explicit model and privacy constraints |
| InferenceScope / Approval | Model/routing/source/redaction scope tied to actual permission |
| AIEnrichment | Optional narrative status, requested/actual provider/model metadata, citations, approval IDs and redacted content |
| ReportManifest extension | PDF path, written flag, digest, error and renderer version independently of JSON/Markdown |

Tool manifests are application-owned validated configuration, not executable repository-supplied configuration. Command/parser policy IDs resolve only to built-in reviewed implementations. Editing a string in tools.json must not create an arbitrary command. Declared metadata is not proof an executable is trusted or licensed; verify against the release manifest/binary identity at preflight.

Root schema `oneOf` covers store files. WrapperRequest and WrapperControl are `$defs` used by separate protocol schemas; they are not end-user persistent database roots. Native findings are parsed on a controlled channel by the core; never inject raw payloads into the transport record or ordinary logs.

## 18. Additional semantic invariants

1. `ready` tool requires installed compatible version, verified command/parser mapping, acceptable license/distribution review and non-placeholder executable identity. Missing/deferred tools never receive fake versions or success.
2. `ScanSettings.adapter_ids=[]` means classify/select from approved capability policy; explicit IDs narrow that policy. Freeze resolved engines/targets before execution. Adapter changes invalidate relevant plan fingerprints/approvals.
3. Manifest network `none/cache_only` must be enforced by compatible tool configuration/isolation. A label is not a network firewall; reject unsafe capability if enforcement unavailable.
4. Exit code is evidence, not verdict. `transport_status=completed` means invocation ended, not checks passed. Native output/exit parser must agree; findings may coexist with later failure/incomplete coverage.
5. Control-channel IDs resolve to core-owned bounded streams/artifacts, not arbitrary filesystem paths. Retain only sanitised durable material. Sensitive temporary output is separately controlled, not falsely described as securely erased.
6. WrapperRequest has no key/command-string/catch-all field. `category` must be one the adapter's manifest supports; every entry of `required_capabilities` must be covered by a valid grant in `approval_ids` (empty capabilities means no network). `termination_policy` is declarative; the supervisor enforces timeout/cancel/process-tree kill out-of-band and reports the observed `WrapperControl.termination`.
6a. Pilot semantic validation rejects `ScanSettings.secret_scope="history"` (SCAN-09 deferred) and capabilities `non_mutating_verification`, `documentation_fetch` and `embedding_transmission` (deferred), with an explicit deferred reason rather than a silent drop. Do not inherit model secrets into child environment or persist credentials elsewhere to bypass the schema.
7. File/context boundaries, one-based locations, source/generated artifact identity and working-tree digests remain enforced. Non-YAML context is disclosed, not silently a new selected file.
8. Secret evidence/fingerprints never include raw secret values. Redaction version and source provenance are retained; redaction is not a universal confidentiality guarantee.
9. Session-only mode uses null credential reference; OS mode requires supported secure storage. If unavailable, offer session-only or disabled assistant, never plaintext fallback.
10. Model-transmission grants require matching inference scope; all other grants use null inference scope. Relevant model/provider/privacy/context changes cannot reuse mismatched approvals.
11. Initial privacy policy denies collection, requires ZDR and disables fallback; all three are schema constants (`allow_fallbacks` `const false` since CA-01) and are also enforced when the request is built. Empty provider allowlist means omit `only` in the provider API; no eligible endpoint fails closed. No silent policy/model change.
12. AI non-completion has empty narrative and explicit status/reason. Completed narrative cites eligible source IDs. Actual provider/model may remain null if not returned; never infer them.
13. AI narrative is not an engine-confirmed finding. Evidence, summary and coverage cannot be silently rewritten by a model. New narration creates a versioned/new report.
14. `pdf_written=true` requires verified nonempty output, valid path/digest and no unresolved conversion error; no remote assets/active code/unrestricted local file reads during conversion. JSON/MD remain available on PDF error.
15. Tool/license list is provisional until exact-version review. GPL/MIT/Apache summaries do not audit transitive libraries or automatically authorise bundling. Docker Desktop and vendor-hosted services are separate entitlements.
16. Style and workflow-validation categories distinguish lint/correctness from security. Rendering outcome is not semantic/security success. CVEs in images link to resolved artifact identity and data freshness.
17. Tool/engine overlap preserves all evidence origins. Deduplication does not erase failed coverage or make dissimilar checks equivalent.
18. Completed report checks cannot remain pending/running. Interrupted tasks become incomplete/cancelled with explicit run interruption metadata. A passing process plus unavailable schema is not a clean assessment.

## 19. Migration guidance

- Validate the actual old schema and back up before changing mutable app stores.
- Do not relabel old JSON with a newer `schema_version` (1.2 or 1.3) while omitting required fields.
- 1.2 → 1.3 / 1.0 → 1.1 (CA-02): new required fields (`TerminationRecord.descendant_cleanup`, `WrapperControl.tool_term_signal` and `output_streams`, `Check.term_signal`) have no truthful default for already-recorded runs (e.g. there is no "unknown" stream state). Records under 1.2/1.0 are therefore **not** valid 1.3/1.1 records and are not migrated automatically: keep them read-only under the 1.2/1.0 reader, or define an explicit, owner-approved migration that marks unavailable data rather than inventing it. No 1.2/1.0 records existed when CA-02 was versioned (no application code).
- Derive new manifests only from inspected/pinned tool data; never populate fake current versions.
- PDF metadata defaults to not written/null; no prior renderer/version is invented.
- AI metadata for old evidence is unavailable/not requested unless supported by records; do not guess historical model/provider use.
- Missing target/engine snapshots are unavailable evidence, not reconstructed from today's repo. Preserve old accepted/signed reports read-only with their schema reader where necessary.
- Existing non-OpenRouter provider settings need owner review rather than automatic disclosure to a different endpoint.
- After migration validate shape, references, timestamps, selection/approval bindings and report consistency. Interrupted/incomplete stores reconcile without turning pending decisions into grants.

## 20. Development records and orchestration are separate

`plan.md`, `status.md`, `handover.md` and CLAUDE.md are development-control files, not tool-run database records. Opus generates the plan after inspecting the real checkout. Sonnet writes bounded implementation/fixes; Opus invokes actual tests/regression and prepares a structured phase report; Haiku updates only allowed progress/handover documents from that report; Opus validates their diff. The human approves debug cycles and next feature phases. No invented test counts/commits/approvals.

Do not store real model keys, client excerpts or raw command logs in handovers. Record evidence references, code revision, actual command results, remaining manual checks, blockers and next authorised action. Agent status is not runtime permission. No unverified code progress is established by this schema or its examples.


## 21. Planned adapter ID registry

This table defines the planned identifiers and wrappers, not installed versions. tools.json entries are created only from approved/inspected implementation data. Development-only tools remain outside end-user scanner selection.

| Adapter ID | Tool | Wrapper / native equivalent | Capability | Proposed release status |
|---|---|---|---|---|
| git | Git | `pull.sh` / core Git invocation | Clone new managed repos; safe update existing repos | Foundation |
| yamllint | yamllint | `scan_yaml.sh` | Generic YAML diagnostics; style separated from syntax | Pilot baseline |
| gitleaks | Gitleaks | `scan_secrets.sh` | Current-checkout suspected-secret detection | Pilot baseline |
| checkov | Checkov | `scan_misconfig.sh` | Security policies for supported configuration frameworks | Pilot baseline |
| actionlint | actionlint | `scan_actions.sh` | GitHub Actions structure/expressions; controlled embedded-script analysis | Pilot framework adapter |
| kubeconform | kubeconform | `scan_kubernetes.sh` | Versioned Kubernetes schema checks, approved CRD schemas | Pilot framework adapter |
| zizmor | zizmor | `scan_actions_security.sh` | GitHub Actions security-specific analysis, no fixes | Conditional extension X1 (owner D-04) |
| docker-compose | Docker Compose CLI | `validate_compose.sh` | Explicit Compose model validation, not security certification | Conditional on Compose scope |
| helm | Helm | `scan_helm.sh` | Chart lint and controlled local template rendering | Conditional on Helm scope |
| trivy | Trivy | `scan_referenced_images.sh` | Known vulnerabilities in approved referenced images | Conditional on image-vulnerability scope |
| rust-schema | Chosen Rust schema validator | In-process adapter, not mandatory shell | Automatic bindings/local supplied schemas for supported dialects | Pilot baseline |

Invocation (owner D-03, CA-01): the Linux pilot invokes every tool through an application-owned native Rust process adapter (Git directly). The named `.sh` wrappers are the optional parity extension X6, not a mandatory runtime layer; the tool-to-capability mapping above is retained. Exact versions, flags and supported-framework coverage are frozen only after the approved per-tool spikes; a baseline capability is never silently downgraded. The canonical pilot/conditional/deferred label register is PRD §16.


`preflight.sh` checks dependencies; it is not a vulnerability scanner. ShellCheck/Bats verify application-owned wrappers during development. Cargo fmt/Clippy/test/build/audit verify the Rust product. Do not persist fabricated ready states, commands or tool versions from this roadmap.


## 22. Pre-implementation contract amendment CA-01

Date: 2026-10-23. Authority: owner decision D-08 in the documentation-only planning stage. Applied before any application code, test or persisted record existed (repository revision `805ced4`, no implementation).

| Item | Change | Resolves |
|---|---|---|
| `ProviderPolicy.allow_fallbacks` | `{"type":"boolean"}` → `{"const": false}` | C2 |
| `WrapperRequest` | Added required `category` (`Category`), `required_capabilities` (unique `Capability[]`) and `termination_policy` (`TerminationPolicy`) | C3 |
| `TerminationPolicy` (new) | `control="supervisor_out_of_band"`, `grace_ms` ≥ 0, `kill_scope="process_tree"`; declarative only | C3 |
| `WrapperControl` | Added required `termination` (`TerminationRecord`); `transport_status` gains `output_limit_exceeded` | C3 |
| `TerminationRecord` (new) | `reason`, `method`, `process_tree_exited` (nullable) | C3 |
| `Check.native_status` | Gains `output_limit_exceeded` | C3 consistency |
| Command/event vocabulary | One canonical list in Architecture §6 with pilot/reserved status | C4, C5 |
| UI schema version | `schema_version: "1.2"` → `ui_schema_version: "1.0"` | C6 |
| Storage layout | Architecture §11 lists `tools.json` and reserved `suppressions.json` | C7 |

Synchronisation: `schemas/workbench-store-v1.2.schema.json` is canonical. The §7 embedded copy and the `$defs` of `schemas/wrapper-request-v1.0.schema.json` and `schemas/wrapper-control-v1.0.schema.json` are byte-equivalent copies regenerated from it; each schema file carries a `$comment` naming CA-01. The §8–§10 examples were re-validated against the amended schema (planning evidence in `evidence/P00-planning/`).

Version handling (historical, as decided for CA-01): identifiers `schema_version="1.2"` and `protocol_version="1.0"` are retained because no implementation, persisted store, report or wrapper existed under the pre-amendment text, so no reader or record can observe two meanings. This exception closes at the first implementation commit that reads or writes these contracts (planned P01/P02): after that any shape change requires a new version, migration notes (§19) and owner approval. Copies of the pre-amendment v1.2/v1.0 files distributed outside this repository (for example the original specification ZIP) are superseded and must not be used to produce records. If the owner prefers explicit new identifiers (for example store 1.3 / protocol 1.1), that is a mechanical follow-up before P01.

Superseded identifiers (note added 2026-10-24, CA-01 text above unchanged): versioned amendment CA-02 (§23) moved the current contract to store **1.3** / wrapper protocol **1.1**; the canonical file is now `schemas/workbench-store-v1.3.schema.json`.

## 23. Contract amendment CA-02 (descendant cleanup, child signal, stream state)

Date: 2026-10-24. Authority: owner approval of findings F-2/F-3 from the P00-T05 feasibility spike (`evidence/P00/P00-T05/record.md`). CA-01 (§22) is unchanged; CA-02 is an additional amendment.

| Item | Change | Finding |
|---|---|---|
| `TerminationRecord.reason` | adds `descendant_cleanup` | F-2 |
| `TerminationRecord.descendant_cleanup` (new, required) | `DescendantCleanup`: `performed`, `method` (`not_required`/`graceful`/`forced`/`failed`), `leftover_live_count` (nullable) | F-2 |
| `WrapperControl.tool_term_signal` (new, required, nullable) | signal that ended the direct child, separate from `tool_exit_code` | F-2 (preserve child status) |
| `WrapperControl.output_streams` (new, required) | `stdout`/`stderr` `StreamRecord`: `state` (`not_applicable`/`complete`/`truncated`/`reader_timeout`), `bytes_consumed`, `bytes_retained` (nullable) | F-3, F-1 recording |
| `Check.term_signal` (new, required, nullable) | child signal status in report checks | F-2 |

Semantic rules (added to §18 by reference):
1. `reason` is the **primary** supervisor trigger (timeout, cancel_requested, output_limit, supervisor_shutdown). Later descendant cleanup never overwrites it; it is recorded in `descendant_cleanup`.
2. `reason = descendant_cleanup` only when the direct child exited on its own and cleaning up remaining group members was the only supervisor action.
3. `tool_exit_code` / `tool_term_signal` (and `Check.exit_code` / `term_signal`) describe the direct child as observed by `waitpid`, never the result of cleanup of other processes. At most one of exit code and signal is non-null.
4. Spawn failure (`transport_status = launch_failed`): both streams `state = not_applicable`, counts null; `descendant_cleanup.performed = false`.
5. `state = truncated` whenever consumption stopped at the budget; `transport_status = output_limit_exceeded` and `reason = output_limit` when that triggered termination. Bytes generated by the child beyond `bytes_consumed` are unknown and are not claimed.
6. `descendant_cleanup` covers only process-group members. Descendants that changed group/session are outside this record's guarantee.

Synchronisation: canonical `schemas/workbench-store-v1.3.schema.json`; the §7 embedded copy and both wrapper schemas (`schemas/wrapper-request-v1.1.schema.json`, `schemas/wrapper-control-v1.1.schema.json`) regenerated from it. Field changes applied by `evidence/P00/CA-02/apply_ca02.py` (check 21/0/0 at the time, against the then-named v1.2/v1.0 files); version identifiers and file names applied by `evidence/P00/CA-02-versioning/apply_versions.py` with check `evidence/P00/CA-02-versioning/check.md`. Earlier checks (`evidence/P00-planning/`, `evidence/P00/CA-02/`) are historical and are not re-run or edited.

Version handling — **resolved 2026-10-24 (owner decision Q-03): CA-02 is a versioned amendment.** Store contract `schema_version` 1.2 → **1.3** (all store root types); wrapper protocol `protocol_version` 1.0 → **1.1** (`WrapperRequest`, `WrapperControl`). Schema files renamed accordingly. CA-01 (§22) remains as recorded. Migration consequences: §19.

Example `WrapperControl` after an output-limit termination with descendant cleanup (synthetic; validated against the amended schema):

```json
{
  "protocol_version": "1.1",
  "request_id": "0b6f6c2e-3f1a-4c8e-9a51-6f0d2b7c1a10",
  "task_id": "5d2c9e41-7a0b-4b6e-8f3d-2e9a1c4b7d22",
  "adapter_id": "gitleaks",
  "tool_version": "8.30.1",
  "transport_status": "output_limit_exceeded",
  "tool_exit_code": null,
  "tool_term_signal": 15,
  "termination": {
    "reason": "output_limit",
    "method": "graceful",
    "process_tree_exited": true,
    "descendant_cleanup": {
      "performed": true,
      "method": "graceful",
      "leftover_live_count": 1
    }
  },
  "output_streams": {
    "stdout": {
      "state": "truncated",
      "bytes_consumed": 1048576,
      "bytes_retained": 1048576
    },
    "stderr": {
      "state": "complete",
      "bytes_consumed": 212,
      "bytes_retained": 212
    }
  },
  "native_format": "json",
  "output_channel_id": null,
  "diagnostics_redacted": [],
  "elapsed_ms": 418
}
```
