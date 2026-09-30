# Production remediation - 2026-09-30

Baseline: d5a4e9f, codex/casy-0.1.3-production-validation.
No changes to live case data, no push or release authorized in this pass.

## Acceptance matrix

| Area | Current pass | Remaining acceptance |
| --- | --- | --- |
| Backup / restore | Foreign source path recognition, native target mapping, Markdown file URLs; regression tests and encrypted foreign archive fixture | Windows execution, Mac-to-Windows archive restore, installed application restart |
| Cases / persons | Propagate row decoding errors instead of silently omitting records; reject duplicate person attachments with absent/blank roles | Native forms, concurrent attachment requests, legacy duplicates assessment |
| Tasks / calendar / deadlines | Existing modular transaction and lifecycle regression rerun | Real case deadline checks, native notification and restart matrix |
| Documents / knowledge | Existing frontend regression and build rerun; restored note attachment test | Actual DOCX/PDF outputs, save/conflict/restart native scenarios |
| Files / OCR / processing | Existing Rust regression suite requested | Large scans, cancellation, disk-full and installed runtime |
| Projects / whiteboard / clients | Not yet fully inspected in this pass | CRUD, relationships, failure recovery and native workflows |
| AI / approvals / external services | Existing modular approval regression rerun | Actual model, external MCP, credentials, WebDAV faults |
| Windows packaging | Original CI failure localized; source path fixes underway | New commit CI, NSIS installation, installed app startup and workflow checks |

## Confirmed defects addressed

- Restore interpreted archived source roots through the host OS path parser. Foreign roots were rejected or not remapped.
- Markdown relocation used host-specific file URL decoding and unescaped native destinations.
- Case/person list readers dropped decoding failures and returned incomplete lists as success.
- SQLite UNIQUE(case_id, person_id, role) does not prevent repeated NULL roles. Attachment now normalizes blank roles and checks duplicates within an immediate transaction.

## Verification boundary

This is an in-progress remediation record, not a whole-product acceptance certificate.
Frontend: 79 files / 355 tests passed; typecheck and build passed, existing large-chunk warning remains.
Script tests: 5 passed.
Initial backup tests: 5 passed before adding the full foreign encrypted archive fixture.
Initial modular and person integrity integration tests: passed before extending duplicate-role coverage.
Final Rust rerun and updated totals will be recorded after completion.

Additional review candidates: silent row-error filtering remains in other modules; do not assume these callers are covered by the case/person patch. General CI still excludes Windows, whereas beta CI includes it. Neither issue is marked complete here.

Further confirmed defect: unified case-view caseType filtering interpolated user input into SQL while other filters used bound parameters. Replaced it with a bound parameter, with regression checks for a quoted legal cause and a SQL predicate injection string.

The full encrypted foreign archive fixture passed on macOS: restored database file path, actual evidence bytes, and the note file URL resolve to the same new attachment. This does not replace Windows execution.

Updated checks:
- All 6 backup tests passed, including the full foreign encrypted archive.
- Updated person/case integrity integration test passed (corrupt rows, NULL/blank duplicate roles, quoted cause, injection-shaped filter).
- Workspace lifecycle regression initially failed because it expected an unencoded path substring. Replaced that assertion with Markdown parsing, URL-to-path resolution, destination equality and actual file-content verification; the test passed.
- Frontend 355 tests, production build/typecheck and 5 script tests passed.
- Full Rust suite is rerunning against the final source, log: /tmp/casy-review-verified-final.log. Earlier full runs do not establish final success.

Final source Rust `cargo test --locked --tests` completed with exit 0. All executed test binaries passed, including workspace lifecycle, person/case integrity, foreign encrypted backup, WebDAV full backup, modular review and Zvec restoration. This is macOS source verification, not Windows or native GUI acceptance. Log: /tmp/casy-review-verified-final.log.

## Round 2

Addressed SQL row-error suppression across drafts, deadline rules/audit, notifications,
whiteboards, links, knowledge-tree children, sync mappings, smart rules, relation detection,
unregistered-file lookup, and inbox party matching. Knowledge import's database-page
fallback now propagates query/row errors instead of importing silently truncated content.
Unregistered-file directory scans now report filesystem read/metadata errors.

Relation detection now runs in an immediate transaction: a later failure rolls back
all newly detected links. Batch smart-rule failures now report partial completion and
failed file IDs instead of returning a success count while only logging errors.

General CI includes windows-2025 again, using the pinned renderer/OpenSSL preparation,
Zvec runtime PATH, test compilation and loader diagnosis before native tests.
No remote execution or successful Windows package is claimed; changes remain local.

New regression test injects corrupt rows into drafts, notifications, whiteboards,
smart rules, knowledge children, audit and links; injects a second-relation failure;
checks rollback, successful repaired reads, repeat detection and batch failure reporting.
New script tests guard against reintroducing silent SQL-row filtering and removal of
Windows CI test prerequisites. Seven script tests passed. Final Rust run pending.

Additional round-2 correction: smart-rule keyword reads no longer turn a database
read error into an empty keyword list and overwrite existing data. Regression keeps
a deliberately invalid original value unchanged when applying the rule fails.

Scope boundary: no claim of complete UI/native acceptance or all future functionality.
Best-effort optional title lookups and non-SQL filtering are not mechanically replaced.
Deferred new features remain distinct from the confirmed data-integrity defects.

Round 2 final verification: Rust full tests exited 0, 357 passed / 5 ignored / 0 failed (excluding duplicate child-process execution). Frontend 355 passed; script tests 7 passed; typecheck and production build passed with the existing large-chunk warning. CI YAML parsed successfully. Log: /tmp/casy-r2-clean.log. Two inbox tests initially lacked the cases table; their fixtures now initialize it instead of relying on swallowed database errors. Windows execution remains unverified until these local changes are committed/pushed and remote gates complete.
