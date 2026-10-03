# Bunny review remediation - 2026-10-02

Pushed existing commit 194b314 to the validation branch. Beta run 36964015959
started; frontend passed, Windows/macOS packaging in progress at last inspection.
Original docs/bunny reports are user-provided untracked material, preserved unchanged.

## First verified implementation scope

- Task defer/undefer uses explicit ok/error results, including native null success.
- Copilot knowledge cards emit the existing parent expansion event.
- Case filter changes reset pagination and query through a store action. Missing
  backend deadline/hearing/operator filter fields remain a separate open issue;
  this does not claim all thirteen controls work.
- Calendar risk checks use dueDate, matching the backend warning DTO.
- Home task completion checks the actual completed field and exposes write errors.
- File reveal success is conditional on backend success.
- Inbox classification accepts pending items only, rechecks at write time, and
  propagates auto-route failure. Replayed receipts restore filed status atomically.
- Legacy Feishu case pull reads numeric/string remote versions, skips unchanged
  versions, and commits case/map writes together. Missing versions and detected
  local conflicts fail without overwriting. Same-second timestamps are treated
  conservatively as conflicts; full revision/hash-based sync remains open.
- Direct MCP write dispatch now uses the pending approval queue and audit rather
  than bypassing it. Approval workflow itself is unchanged.
- Normal CI is configured to trigger on the validation branch as well as main.

## Verification and remaining scope

Frontend: 358 tests passed; typecheck/build passed (existing chunk warnings).
Rust targeted checks in progress; final results must be appended after completion.
No real Feishu server, installed Windows application, or native UI acceptance
is claimed by mocked component tests or source inspection.

Reports still require further detailed review and remediation, notably migration
backup, hearing import rollback, field-filter propagation, OCR finalization,
previous successful OCR fallback, tray/inbox events, service configuration and
other domain/security findings. Review assertions are hypotheses until checked;
raw grep counts are not runtime coverage measurements. No all-findings closure.

Final targeted results for this pass: Feishu pull regression passed; inbox capture
integration passed; direct MCP dispatch + subsequent approval integration passed
(no task before approval, one task after approval, no extra pending request).
Copilot card click + filter-store + native-null IPC regressions: 4 passed.
Scripts: 7 passed. Full Rust suite not rerun for this patch; Windows build in
progress belongs to the already pushed 194b314, not this uncommitted patch.

## Second implementation pass

- Added deadline/hearing range and operator fields to case-list filters, TS DTO,
  store forwarding and SQL row/count conditions; regenerated Specta bindings.
  Page sizes now have a lower bound. Uses existing unified-view date semantics.
- Memo saves use the case store; kanban logs preserve the pre-change status.
- Related-knowledge lookup queries case/client terms separately and deduplicates;
  case goal/task writes expose errors; hearing deletion has confirmation UI.
- Import preview explicitly shows raw source values; failed-row reports no longer
  display unconditional all-success headings.
- Feishu hearing/client auxiliary writes propagate errors and abort the enclosing
  transaction; dump imports count inserted rows rather than ignored duplicates.
- Deadline rule writes validate supported date fields, algorithms and positive
  limits; UI no longer advertises unsupported negative offsets. Invalid holiday
  JSON fails instead of silently reverting to builtin dates.
- Existing on-disk databases are snapshotted via VACUUM INTO before version
  upgrades. Encrypted WAL fixture verifies old schema version and original data
  survive in an encrypted snapshot; failed snapshot aborts migration.
- npm test now has an OS-portable runner creating an isolated profile and including
  script tests and the document-engine models feature suite. Runner itself has
  not yet been exercised end-to-end on Windows.

Checks: frontend 359 passed; build/typecheck passed; encrypted migration snapshot
regression passed. Full Rust rerun: /tmp/casy-bunny-complete.log, pending.
Remaining findings are not closed: OCR finalization/asset storage/fallback, complete
Feishu setup and conflict protocol, approval concurrency, tray events, other domain
logic, AI/db row filtering and test coverage. No claim of all Bunny items fixed.

Second-pass final verification: full main Rust tests exited 0 on macOS. Log:
/tmp/casy-bunny-complete.log. Includes advanced case filters, invalid rules/corrupt
holiday JSON, encrypted migration snapshot, dump-import rollback/counting and
previous inbox/MCP/Feishu regressions. Frontend 359 passed, scripts 7 passed,
typecheck/build passed. This does not close remaining Bunny findings or establish
Windows/runtime acceptance. Current changes remain uncommitted.

## Third implementation pass (in progress)

- Workspace preview selects the latest successful result instead of letting a
  failed retry hide it; original file hash verification still rejects stale data.
- Inbox listens for native new-item events. Root UI handles tray note/file/clipboard
  actions and opens capture before delivering dropped files; listeners are cleaned up.
- Email records, inbox insertion and UID advancement are one transaction; filtered
  mail advances UID and committed intake emits the native inbox event.
- Feishu selected table is persisted through a typed service call; backend table
  configuration commits both keys atomically and rejects empty values.
- AI summaries/reports/recursive checks and schema column readers now propagate
  SQL row errors. Guard script expanded to commands, AI and DB directories.
- PDF search-layer construction emits finalization progress after every page.
  This closes total-finalization-duration timeout across progressing pages, but
  does not solve an individual render or final save taking >15 minutes; heartbeat
  / subprocess timeout design remains open, not marked fully resolved.

Tests added for failed OCR retry fallback, atomic email intake and filtered UID,
and corrupt AI summary rows. Document engine: 19 passed / 5 ignored. Full Rust
and updated frontend verification running; final results to follow.

Also corrected EDATE negative offsets using checked signed month addition/subtraction;
added leap-day and cross-year tests. Third-pass frontend: 359 passed; build passed;
script guard: 7 passed; engine: 19 passed / 5 ignored. Main Rust final run remains
in progress (/tmp/casy-follow-last.log). No full-report completion claim.

## UI/UX plan 10 scope accepted

Read the entire 244-line plan. Preserve the current Vue/Element Plus/docket themes;
prioritize connected behavior, then shared primitives/state handling, then reachable
900/1100/1360 desktop layouts. Do not bulk-delete narrow media rules or introduce
another design framework. Native 800x600 / 1024x720 / 1440x900 theme checks remain
required and are not replaced by browser fixtures.

First implementation: UiDataState composes existing skeleton/empty/error components,
keeps last results during refresh/error, and separates filtered-empty from first-use
empty. Adopted in persons; project failures/skeletons use the same existing primitives.
DegradedBanner supports non-dismissible generic errors and semantic theme colors;
SkeletonCard uses theme tokens and reduced motion. Project narrow layout breakpoint
moved from 600 to 899px. State matrix tests: 2 passed; initial typecheck passed.
Case mock fixtures now use the generated Case DTO, real nullable defaults, string
attorneys and caseProgress; phantom caseType/dueDate fields removed. Typecheck running.
Plan 10 is NOT marked complete: shared primitives adoption, remaining mock DTOs,
all-module four-state handling and native layout/theme validation remain open.

## Next continuation

Windows run 36964015959 completed: macOS success; Windows failed in
`delivery_lifecycle_test` during full restore with Access denied (os error 5).
Prior portable-path tests passed. Identified read-only File::open + sync_all in
restore/capture/snapshot code; changed these to writable handles as required by
Windows FlushFileBuffers. Added source guard; native Windows rerun still required.

Relation selection now loads every page through CasesService.listAll, reloads on
opening the dialog and rejects partial query failure. Network view uses the same
full list and actual Connection icon. Added two-page/failing-page regression.
UiPill and UiTruncate adopted in projects, and UiDataState adopted in inbox.
All-report completion and local commit remain pending.

Approval race fix: pending MCP writes are claimed as approved in an immediate
transaction before execution; reject also checks/transitions within an immediate
transaction. Concurrent approval regression requires exactly one success and one
created task. Crash reconciliation of approved/in-flight writes remains a separate
open item; do not call this exactly-once across process crashes.

Schema comparison test added for baseline schema + migrations through 1/20/30/40
versus fresh installation, including a second initialization. It validates checked-in
migration paths, not arbitrary historical customer databases. Execution pending.
UI CaseDetail case-type label now reads CaseTypeMetrics, not nonexistent Case.caseType.

Latest checks: frontend 362 passed; scripts 8 passed; build/typecheck passed.
Backup/restore/migration/workspace targeted tests passed after writable flush change
(on macOS only). Schema equivalence test passed. Concurrent approval test initially
failed with both calls returning errors; after diagnostic assertion/rebuild it passed
six consecutive executions. Treat initial failure as an unresolved reproducibility
risk until full-suite/concurrency validation, not as proof it never occurs.
Full latest Rust run started at /tmp/casy-latest-full.log. No local commit yet.

Browser UI observation: personnel page at current preview rendered a real error
banner with retry (no fake empty state), but exposed `browser-mode: no mock` and
showed a misleading zero count. Replaced the diagnostic with a user-facing preview
limitation and hide entity count while loading/failed. Browser preview is not native
DB acceptance. Screenshot inspected through computer-use; native size/theme matrix
still pending.

Personnel preview correction verified live in browser: user-facing preview limitation
replaces raw no-mock diagnostic; zero count hidden on failed read; retry remains visible.
This is browser UI evidence only. Latest parallel frontend run hit the whiteboard
beforeAll timeout while Rust linking consumed resources; rerun serially before claiming
final frontend pass. No ignored/failed test is treated as passed.
