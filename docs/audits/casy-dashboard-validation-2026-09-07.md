# Dashboard and calendar projection validation - 2026-09-07

## Changes

Dashboard counts use actual cases and valid tasks. Timed hearings are included today and at the end of the upcoming window, with no 50-row truncation. Waiting-party tasks count toward overdue follow-up; blank dates do not. Completion trends count the latest completion of currently completed, undeleted tasks, avoiding duplicate completion events and reopened tasks. Sections fail independently, support retry, and refresh after mutations and day rollover. Chart geometry, scaled tooltips, accessible tables and drill-down routes are corrected.

Calendar maps native eventType and timing fields, splits hearing dates and times, includes month-end timestamps, validates months, and renders timed/all-day events in day and week views. Hearing links open the source case. Case choices read paginated results. Failed calendar reads show retry; stale requests cannot replace a newer month.

## Evidence

The staged index was exported to /tmp/casy-dashboard-index.Dmoiho. Final production build and 159 frontend tests passed on 2026-09-07. The same batch previously passed the full native suite. The new native integration test covers source/mirror disagreement, repeated completion, reopening, blank dates, 51 upcoming hearings, timed window boundaries, month end, year rollover and invalid months.

Private artifacts are under /Users/only/Documents/Casy-Local-Test/2026-09-06-DNFuEj/. dashboard-profile-66n72T completed 86 real Rust commands: KPI/chart values, 6/12-month trends, failure/retry, completion/undo, waiting-party filtering, case filtering, hearing time/case labels, all-day and early events, source-case navigation, mutation refresh and empty state. Integrity was ok, foreign-key errors zero, unexpected page errors zero; one trend failure was intentionally injected. Desktop and narrow-screen screenshots remain private.

ui-profile-U74F2D reran the private 59-case snapshot import, individual and related cases, third-party editing, shared tasks and byte checks of 20 case-file references to 17 assets. Integrity was ok, foreign-key and page errors zero. No Feishu or external AI calls were made; the production database and installed release were not changed.

## Reproduction

Run npm run build and npx vitest run --config vitest.config.ts. Set CASY_TEST_DATA_DIR to an isolated directory for cargo test --lib --tests. Build dashboard_local_bridge and intake_local_bridge examples, then run tests/e2e/dashboard-local.mjs or intake-local.mjs with CASY_QA_URL, CASY_QA_DIR and CASY_PLAYWRIGHT_MODULE.

## Remaining work

This is not full calendar or delivery acceptance. Still inspect task/deadline deduplication, cross-month week views, node editing/deletion, event end-time preservation, fabricated default times/durations, working-day/workload calculations, unknown-track drill-down and retained filters. Cross-window behavior, large libraries, real model quality and long-document OCR remain open.
