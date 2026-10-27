"""Generate the FR-31 classification appendix (planning/P02-T01-fr31-classification.md) from the canonical 1.4 schema.

Lead-owned development tool. Usage: python3 -I -B dev/p02_fr31_appendix.py <schema> <out.md>
Exits non-zero if any plain-string or string-map property of a store-reachable `$def` is unclassified, or if a
classified property does not exist in the schema.
Revision 2 (P02-D01, 2026-10-27): string-valued maps (`type: object` with `additionalProperties: {type: string}`)
are covered; revision 1 (P02-T01, scratchpad script) missed `RendererSetting.parameters_redacted`.
"""
import hashlib, json, sys
schema_path, out_path = sys.argv[1], sys.argv[2]
raw = open(schema_path, 'rb').read()
s = json.loads(raw); d = s['$defs']
WRAPPER = {'WrapperRequest', 'WrapperControl', 'TerminationRecord', 'StreamRecord', 'DescendantCleanup', 'TerminationPolicy'}
def plain(n):
    if not isinstance(n, dict): return False
    if n.get('type') == 'string' and not any(k in n for k in ('enum', 'const', 'format', 'pattern', '$ref')): return True
    if any(plain(x) for x in n.get('anyOf', []) + n.get('oneOf', [])): return True
    if n.get('type') == 'object' and plain(n.get('additionalProperties')): return True
    return n.get('type') == 'array' and plain(n.get('items'))
def shape(n):
    nullable = any(x.get('type') == 'null' for x in n.get('anyOf', []))
    kind = {'array': 'array', 'object': 'map (values; keys `Identifier`)'}.get(n.get('type'), 'string')
    return kind + (', nullable' if nullable else '')
R, S = ('R', 'RedactedText'), ('S', 'SanitisedText')
def N(t): return ('N', t)
def E(t): return ('E', t)
C = {
 'Selector': {'include_patterns': N('PathPattern'), 'exclude_patterns': N('PathPattern')},
 'RepoTarget': {'ref_value': N('GitRefName')},
 'SchemaBinding': {'path_pattern': N('PathPattern'), 'schema_resource': N('SchemaResourceId'), 'dialect': N('Identifier')},
 'RendererSetting': {'framework': N('FrameworkId'), 'renderer_id': N('RendererId'), 'parameters_redacted': R},
 'ScanSettings': {'adapter_ids': E('AdapterId (domain, Data §21)')},
 'FileRecord': {'detected_framework': N('FrameworkId'), 'reason': S},
 'Repository': {'display_name': R, 'slug': N('Slug'), 'remote_url': N('RemoteUrl'), 'canonical_remote': N('CanonicalRemote'),
                'local_path': N('AppRelPath'), 'default_ref_value': N('GitRefName'), 'last_error_redacted': R},
 'Schedule': {'daily_local_time': N('LocalTime'), 'timezone': N('IanaTimezone')},
 'Workflow': {'name': R, 'description': R},
 'Approval': {'purpose': R, 'destinations': N('Destination'), 'resource_constraints': S, 'actor': R},
 'SourceRef': {'resource_identifier': R, 'redaction_version': N('RedactionVersion')},
 'Engine': {'adapter_id': E('AdapterId'), 'adapter_version': N('VersionLabel'), 'tool_name': N('ToolName'),
            'tool_version': N('VersionLabel'), 'ruleset_version': N('VersionLabel')},
 'Check': {'framework': N('FrameworkId'), 'reason_redacted': R, 'adapter_id': E('AdapterId')},
 'Finding': {'rule_id': R, 'title': R, 'description_redacted': R, 'evidence_redacted': R, 'remediation_redacted': R},
 'Artifact': {'display_name': R, 'retained_path': N('AppRelPath')},
 'Event': {'event_type': E('EventType (Arch §6 event names)'), 'message_redacted': R, 'code': N('EventCode')},
 'ReportManifest': {'json_path': N('AppRelPath'), 'md_path': N('AppRelPath'), 'pdf_path': N('AppRelPath'),
                    'pdf_error_redacted': R, 'pdf_renderer_version': N('VersionLabel')},
 'Suppression': {'rule_id': R, 'reason': R, 'actor': R},
 'Chunk': {'text_redacted': R, 'redaction_version': N('RedactionVersion'), 'embedding_key': N('Identifier')},
 'Message': {'text_redacted': R, 'provider_id': N('ProviderId'), 'model_id': N('ModelId')},
 'Conversation': {'title': R},
 'Provider': {'model_id': N('ModelId'), 'credential_reference': N('CredentialRef')},
 'Config': {'app_data_root': N('LocalPath'), 'backup_directory': N('LocalPath'), 'active_theme': N('Identifier'),
            'redaction_version': N('RedactionVersion')},
 'ReportFile': {'repo_name': R, 'remote_identifier_redacted': R, 'selected_ref': N('GitRefName'), 'limitations': S},
 'ProviderPolicy': {'only': N('ProviderId')},
 'InferenceScope': {'model_id': N('ModelId'), 'redaction_version': N('RedactionVersion')},
 'AIEnrichment': {'requested_model': N('ModelId'), 'actual_model': N('ModelId'), 'actual_provider': N('ProviderId'),
                  'redaction_version': N('RedactionVersion'), 'narrative_redacted': R, 'reason_redacted': R},
 'TargetSnapshot': {'selected_ref': N('GitRefName')},
 'EngineSnapshot': {'adapter_id': E('AdapterId'), 'adapter_version': N('VersionLabel'), 'tool_name': N('ToolName'),
                    'tool_version': N('VersionLabel'), 'ruleset_version': N('VersionLabel')},
 'LicenseRecord': {'spdx': N('SpdxExpression'), 'verified_version': N('VersionLabel'), 'license_file': N('RelPath'),
                   'notice_files': N('RelPath'), 'source_obligation_note': S},
 'ToolManifest': {'adapter_id': E('AdapterId'), 'adapter_version': N('VersionLabel'), 'tool_name': N('ToolName'),
                  'tool_version': N('VersionLabel'), 'frameworks': N('FrameworkId'), 'platforms': N('Identifier'),
                  'wrapper_name': N('Identifier'), 'command_policy_id': N('PolicyId'), 'parser_id': N('PolicyId'),
                  'native_format': N('Identifier'), 'network_resources': N('Destination'), 'binary_origin': S, 'limitations': S},
}
rows, missing, extra = [], [], []
for name, dd in d.items():
    if name in WRAPPER: continue
    for p, v in dd.get('properties', {}).items():
        if plain(v):
            c = C.get(name, {}).get(p)
            if c is None: missing.append(f'{name}.{p}')
            else: rows.append((name, p, shape(v), c))
for name, props in C.items():
    for p in props:
        if not any(r[0] == name and r[1] == p for r in rows): extra.append(f'{name}.{p}')
if missing or extra:
    sys.exit(f'unclassified: {missing}; not in schema: {extra}')
counts = {}
for r in rows: counts[r[3][0]] = counts.get(r[3][0], 0) + 1
nt = sorted({r[3][1] for r in rows if r[3][0] == 'N'})
with open(out_path, 'w') as f:
    f.write('# P02-T01 appendix A — FR-31 classification of plain-string store properties\n\n')
    f.write('Status: **approved with P02-T01 (owner, 2026-10-27); revision 2 by Opus in debug cycle P02-D01 (2026-10-27)**. '
            'Generated by `dev/p02_fr31_appendix.py` from '
            f'`schemas/workbench-store-v1.4.schema.json` (sha256 `{hashlib.sha256(raw).hexdigest()}`); the generator fails if any '
            'plain-string property is unclassified or a listed property does not exist. "Plain string" = `type: string` '
            '(or nullable / array items / string-map values of it) without `enum`, `const`, `format`, `pattern` or `$ref`. '
            'Revision 1 missed string-valued maps; revision 2 adds the only one, `RendererSetting.parameters_redacted` (class R: '
            'workflow-author renderer parameters; Data §11.11), found by W1 in P02-T05a.\n\n')
    f.write(f'Scope: the {len(d) - len(WRAPPER)} `$defs` reachable from the ten store roots. The six wrapper-protocol `$defs` '
            '(WrapperRequest, WrapperControl, TerminationRecord, StreamRecord, DescendantCleanup, TerminationPolicy) are not '
            'reachable from any store root and are classified with the wrapper protocol in P06 (request §2, T01-D2).\n\n')
    f.write(f'Totals: {len(rows)} properties — E closed enum {counts.get("E",0)}, N validated newtype {counts.get("N",0)}, '
            f'R `RedactedText` {counts.get("R",0)}, S `SanitisedText` {counts.get("S",0)}.\n\n')
    f.write('Rule of thumb applied: text that can originate from a user, a repository, a scanner/tool or a model → R; text the '
            'application composes from its own constants and validated values → S; machine identifiers with a closed syntax → N; '
            'identifiers with a closed value list in the domain crate → E. T13 turns this table into code and the completeness '
            'test (which must cover string-map values too); any reclassification after approval is a lead change recorded in the '
            'T13 record.\n\n')
    f.write('Validated newtypes used (' + str(len(nt)) + '): ' + ', '.join(f'`{t}`' for t in nt) + '. Rules: request §5.2.\n\n')
    f.write('| # | `$def` | Property | Shape | Class | Type |\n|---|---|---|---|---|---|\n')
    for i, (n, p, sh, (k, t)) in enumerate(rows, 1):
        f.write(f'| {i} | {n} | `{p}` | {sh} | {k} | `{t}` |\n')
print(len(rows), counts, len(nt), nt)
