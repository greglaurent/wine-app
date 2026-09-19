# Fixes and release requirements

Updated: 2026-09-19.

This document records the code review, subsequent product decisions, and proposed
order of work. Create and review this record before any further development.
No fix below is implemented merely because it is listed here; all checklist items
remain open. Detailed implementation choices and the final roadmap are to be
settled together.

The [full code review](docs/plans/2026-09-19-code-review.md) contains evidence,
reproduction results, source locations, and limitations for findings R1-R20.
This document supersedes that review's unresolved deployment and cellar-sharing
questions with the decisions below. The review remains a historical assessment.

## Agreed deployment and architecture

- First deployment: a personal cellar accessible over the internet, with multiple
  devices and a path to multiple users.
- Support both private cellars and shared cellars with membership roles.
- Authentication, authorization, and HTTPS are first-release requirements.
- Keep Rust/Axum/Postgres, Datastar, shared Askama rendering, and the WASM/SQLite
  offline client. Correct the existing security, sync, and recovery contracts
  before expanding catalog writes.
- Keep causal IDs. Fix device identity and mutation ordering rather than replacing
  the identifier scheme.
- Build ownership boundaries into server access, synchronization, and local
  storage from the beginning.

## Cellars, accounts, and memberships

A cellar is the access boundary. A private cellar starts with one owner; sharing
adds memberships to that same cellar. A user can belong to multiple cellars and
have a different role in each. Access is attached to the recipient's own account,
never provided by sharing the owner's credentials.

Initial roles:

| Role | Access |
| --- | --- |
| Owner | Manage the cellar, members, invitations, and all cellar records |
| Editor | Add and update inventory and tasting records |
| Viewer | Read cellar records |

Initially, only owners can invite. Invitation and membership operations must not
grant more authority than the inviter possesses. Detailed destructive-operation,
ownership-transfer, and last-owner rules still need specification; the role table
does not implicitly settle them.

Required enforcement:

- [ ] Authenticate data requests and use secure session handling.
- [ ] Authorize every server read, write, and sync operation against the account's
  current membership and role in the requested cellar.
- [ ] Verify that referenced records belong to the authorized scope; accepting a
  cellar identifier from a client is not authorization.
- [ ] Keep private and shared cellar data isolated across accounts and memberships.
- [ ] Scope local data and sync state by account and cellar; define logout and
  account-switch cleanup so one account cannot see another's cached records.
- [ ] Keep server-owned reference vocabulary distinct from cellar-owned user data.
  Decide catalog/reference sharing boundaries explicitly during schema design.
- [ ] Test direct API calls and forged cross-cellar references, not only UI access.

## Simple invitation flow

The agreed interaction is **Invite -> choose role -> send link -> recipient signs
in and accepts**. Acceptance gives the recipient a membership with the selected
access to that cellar. It must remain a short flow for the owner and recipient.

- [ ] Limit invitation creation and management to owners initially.
- [ ] Associate each invitation with its cellar, selected role, and inviter.
- [ ] Require the recipient to sign in before accepting.
- [ ] Make invitations expire and allow owners to revoke them.
- [ ] Validate invitation state and the inviter's authority on the server when
  accepting; a link must not bypass membership authorization.
- [ ] Make acceptance atomic and safe to retry without duplicate memberships or
  unintended privilege changes.
- [ ] Show pending invitations and current members in a simple management view.

The delivery mechanism, expiration duration, reusable versus single-use behavior,
and whether links are bound to a named recipient remain implementation/product
decisions. This document does not authorize sending invitations or messages.

## Agreed offline revocation policy

Revoking membership immediately blocks further server access. It cannot erase
data that a disconnected device has already downloaded; this limitation is
accepted.

On reconnection, the server rejects further synchronization for that membership
and the app removes that cellar's local cache. This includes preventing queued
offline writes from being accepted after revocation. Role changes must likewise
be enforced using current server permissions.

- [ ] Check access on every operation, including requests from stale clients.
- [ ] Distinguish revoked membership from temporary network/server failures;
  a transient outage must not trigger cache deletion.
- [ ] Remove the revoked cellar's local data and sync state when revocation is
  confirmed, without affecting other accounts or cellars.
- [ ] Define user-facing handling of pending writes rejected after revocation;
  do not silently report them as synchronized or bypass the access decision.
- [ ] Test offline use followed by revocation, reconnection, and account switching.

Membership revocation is distinct from a server reset or backup restore. The
accepted revocation policy does not authorize automatic destruction of unsynced
work during ordinary recovery; that recovery policy is still to be decided.

## Review fix register

All items are open. Priorities and evidence are detailed in the full review.

| ID | Required correction | Acceptance evidence |
| --- | --- | --- |
| R1 | Parameterize SQLite values; validate incoming data; constrain generic table/column mappings. | A crafted synced row cannot execute SQL, alter another record, clear its dirty flag, or poison future pulls. |
| R2 | Stop advancing pull cursors from push acknowledgements; return accepted/conflict outcomes and reconcile winners. | A device receives unseen changes from other devices after pushing its own changes. |
| R3 | Establish commit-safe change ordering and consistent pull boundaries for every writer. | Reordered commits and writes during a pull never cause permanently skipped rows. A snapshot alone is insufficient. |
| R4 | Persist distinct device/source identities and durable clocks, including server-writer rules. | Identical starting snapshots, offline creation, restarts, and restores never reuse object IDs. |
| R5 | Separate mutation version from object identity; use deterministic conflict comparison and version-specific acknowledgements. | Equal timestamps converge; stale acknowledgements cannot clear newer local edits; clock skew has a defined outcome. |
| R6 | Implement authentication, membership authorization, suitable origin/CSRF protection, and safe development bindings; retire the demo server write route. | Anonymous, unauthorized, cross-cellar, and disallowed cross-origin operations cannot access or mutate cellar data. |
| R7 | Introduce shared validated values and explicit API errors for IDs, status, revision, timestamps, references, and size limits. | Invalid values cannot persist or corrupt clock recovery; invalid client requests receive actionable errors. |
| R8 | Define safe reset/restore recovery; validate epochs on data operations; preserve pending work according to that policy. | Server replacement, failed metadata checks, interrupted rehydration, and backup restoration do not silently lose offline work. |
| R9 | Define transactional operation boundaries for server writes, local applies and cursors, reset state, and catalog chains. | Failure midway leaves a coherent result; retries are safe and outcomes explicit. |
| R10 | Add worker starting/ready/failed states, bounded waits, pending-request rejection, correct VFS retry, and a multi-tab ownership policy. | Initialization failure, worker termination, and a second tab produce recoverable behavior instead of indefinite waits. |
| R11 | Fix sync lock release, queue follow-up work, add bounded retry/network waits, and expose sync status. | Writes during sync are eventually pushed; local refresh failure cannot permanently disable sync; failures are visible. |
| R12 | Cache a validated HTML shell reliably, coordinate it with asset updates, and retain cache writes for the event lifetime. | First installation and application upgrades both reopen offline; error responses do not replace a working shell. |
| R13 | Preserve query parameters, request bodies, response status, and cancellation semantics in local transport. | Country-filtered GETs reach the worker with their state; local errors are not always HTTP 200. |
| R14 | Encapsulate SQLite connections/statements with RAII, narrow unsafe FFI, and propagate meaningful errors. | Error paths release resources; query failure is never mistaken for absent state, a valid count, or successful partial results. |
| R15 | Add transactional local schema migrations and a protocol compatibility policy. | Older local databases and cached clients upgrade safely with dirty rows intact; schema version, epoch, and cursor stay distinct. |
| R16 | Bound sync batches/pages and field sizes, with continuation and explicit error semantics. | Large offline backlogs progress within transport limits while preserving operation atomicity. |
| R17 | Triage and update dependency advisories; complete frontend auditing and establish ongoing checks. | Reachable advisories are fixed or explicitly assessed; audit failures are visible rather than treated as clean results. |
| R18 | Align container and development toolchains/lock enforcement, verify vendored assets, use an unprivileged runtime, and wait for DB readiness in development. | Reproducible builds, startup smoke tests, and deployment checks pass. |
| R19 | Define reference ownership, removal, tombstoning, and revival; normalize natural keys and reconcile transactionally. | Removed/reintroduced vocabulary synchronizes correctly without breaking historical references; equivalent natural keys cannot evade validation. |
| R20 | Remove obsolete duplicate models and unused abstractions; consolidate validated types, inventory-count semantics, and architecture docs. | One active status definition exists; cellar counts match the agreed lifecycle rules; documentation describes actual behavior. |

### Dependency audit snapshot

The review on 2026-09-19 reported:

- `rustls 0.23.41`: RUSTSEC-2026-0285; fixed in 0.23.45. Active through SQLx;
  relevant to database TLS.
- `event-listener 5.4.1`: RUSTSEC-2026-0221; fixed in 5.4.2. Active through SQLx;
  exploitability of the affected API was not demonstrated in this app.
- `rsa 0.9.10`: RUSTSEC-2023-0071; present in the lockfile but absent from the
  inspected active default-build dependency tree. Do not label it a demonstrated
  application key-recovery vulnerability.
- `spin 0.9.8`: yanked-package warning requiring dependency-path review.
- Frontend audit: incomplete because npm's advisory endpoint returned HTTP 503
  maintenance. Successful installation is not a substitute for this audit.

Advisory links and supporting evidence are in the full review. Recheck the current
advisory database when implementing dependency fixes.

## Proposed work order

This ordering captures the roadmap discussion so far; it is not a claim that
detailed designs or individual fixes have already been approved or completed.
Account/cellar boundaries must inform the sync design even where their UI comes
later. Do not expose the application while early phases are incomplete.

1. **Security and data integrity.** Capture reproduced defects as regression tests;
   parameterize SQLite; validate inputs; correct identity, versions, cursors,
   acknowledgements, transactions, and recovery contracts. Incorporate cellar scope
   into those contracts rather than adding it after catalog sync.
2. **Accounts and access.** Implement authentication, secure sessions, cellar
   ownership and memberships, role enforcement, simple invitations, and revocation.
   Verify isolation through direct server/API tests.
3. **Reliable offline operation.** Finish worker lifecycle, multi-tab behavior,
   local migrations, account/cellar cache isolation, retries, visible sync status,
   shell caching, and recovery. Test with pending offline work.
4. **Bottle entry.** Implement transactional catalog synchronization and the
   bottle-first, country-driven flow described in the
   [entry-form design](docs/plans/2026-07-03-entry-form-design.md). Its visual
   direction remains unresolved. Retire demo scaffolding as real entry replaces it.
5. **Internet launch gate.** Verify HTTPS, hardened deployment, dependency checks,
   backups/restores, and supported browser/device behavior. Implement prerequisites
   earlier where needed; this gate validates them before public use.

## Decisions still to settle

- Authentication mechanism, account creation/recovery, and session lifecycle.
- Exact role permissions for deletion, ownership transfer, last-owner protection,
  and tasting-record ownership.
- Invitation delivery, recipient binding, lifetime, reuse, and acceptance when
  membership already exists.
- Which catalog entities are cellar-owned versus deliberately shared; physical
  storage locations must remain distinct from a cellar's access boundary.
- Device source allocation, source exhaustion, and conflict-resolution policy.
- Preservation/export/reconciliation of offline work after server reset or restore.
- Supported browsers/devices, multi-tab policy, storage durability, and eviction
  handling.
- Non-vintage identity, soft-delete revival, country-scoped relationship validation,
  grape/clone consistency, percentage/position bounds, and hierarchy-cycle rules.
- Atomicity and retries for one entry creating catalog records and N bottles.
- Entry-form visual and interaction design.

## Release acceptance criteria

- [ ] Two devices converge under retries, disconnection, equal-time conflicts,
  delayed acknowledgements, and reordered server commits.
- [ ] Crashes cannot acknowledge unapplied changes or expose incomplete catalog
  operations as successful.
- [ ] Pending offline work survives supported upgrades and recovery procedures.
- [ ] Anonymous users, viewers attempting writes, and users of other cellars cannot
  bypass authorization through direct API or sync requests.
- [ ] Invitations respect role authority, expiry, revocation, and retry-safe
  acceptance.
- [ ] Revocation blocks server access and removes the affected cache when the
  device reconnects, without deleting unrelated data.
- [ ] First installation and application upgrades can reopen offline on supported
  devices; worker/storage failures have recoverable, visible outcomes.
- [ ] Native Rust, WASM, frontend types, lint, regression tests, and dependency
  checks pass; browser/OPFS and container startup tests supplement unit tests.
- [ ] HTTPS deployment and backup restoration are exercised before internet launch.

The previously passing build, lint, TypeScript checks, and seven vocabulary/seed
unit tests are the baseline. They do not yet establish these release guarantees.
