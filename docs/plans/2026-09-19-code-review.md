# Code review - 2026-09-19

The current implementation is a useful prototype, but it is not ready to hold
irreplaceable cellar data or accept untrusted network traffic. The most serious
problems are reproducible sync data loss and server-to-client SQL injection.
The Rust/Axum/Postgres + shared Askama + WASM/SQLite architecture can stay. Its
identity, synchronization, recovery, and trust-boundary contracts need correction
before expanding the catalog write path.

This is a review and an input to a jointly agreed roadmap, not an implementation
plan already approved. Application source and dependency locks were not changed
during the review. Severity assumes the app may eventually serve real data on
multiple devices; the initial deployment/access model remains to be confirmed.

## Scope and evidence

Reviewed all application modules, templates, TypeScript worker/service-worker
code, migrations, reference-data loading, dependency manifests, Nix/direnv/just
setup, Docker/Compose configuration, and design/handoff documents. The authored
wine-labeling data was reviewed as software data, not independently audited for
legal or historical accuracy.

The full native/WASM/frontend build, TypeScript checks, Clippy, and seven Rust
tests passed immediately before this review in the new Nix environment. These
tests cover vocabulary and seed validation, not the sync protocol or browser.

For this review, an isolated PostgreSQL 16.15 database and the existing compiled
server were started with disposable data under `/tmp`. Both were stopped after
testing. No development database or Docker volume was touched. Client SQL was
tested using the exact `apply_pull` format string and SQLite's native
prepare/step API; browser orchestration was tested with extracted source functions
and controlled dependencies. Those are not full browser/OPFS tests.

Observed results:

| Probe | Result |
| --- | --- |
| Anonymous read / cross-origin form-shaped write | Both returned HTTP 200 |
| Push followed by pull from acknowledged cursor | Two server rows, zero rows returned; one was never seen by that client |
| Equal-timestamp update with a higher revision | HTTP 200 and acknowledgement, but server retained old status |
| Two devices submit the same generated identity | Two intended bottles collapsed into one row |
| Valid then invalid row in one push | HTTP 500, first row remained committed |
| Lower sequence commits after higher sequence | Cursor advanced to 171; subsequently committed row at 169 was never returned |
| Crafted ID travels through server into client SQL | Server accepted it; client changed another bottle to `consumed` and cleared its dirty flag |
| Invalid domain values | Arbitrary ID, unknown status, and revision -5 accepted |
| Local count refresh throws | Sync remains permanently marked busy |
| Another sync request arrives during an active sync | No follow-up run is scheduled |
| Local GET includes `?country=FR` | Worker receives pathname only, losing the parameter |

Review-session artifacts: `/tmp/wine_review.py`, `/tmp/wine_review_sync.mjs`, and
`/tmp/wine-review-wf2i02sk/results.json`. These temporary paths are evidence from
this machine, not a durable regression suite. The scenarios below specify what
should become repository tests.

## Findings requiring attention before real data or network exposure

### R1 - High: untrusted sync rows become executable client SQL

Locations: [client sync](../../crates/client/src/sync.rs), lines 56 and 78;
[client state](../../crates/client/src/store.rs), line 93;
[server push](../../crates/server/src/sync.rs), line 168.

The server correctly binds Postgres parameters, but accepts arbitrary strings for
IDs and statuses. The client later interpolates those strings into SQL. A crafted
ID accepted by `/sync/push` can terminate the intended INSERT and introduce a
different conflict-update clause. `sqlite3_prepare_v2` executes the first statement
and ignores the trailing SQL because the tail is not inspected. The isolated
test changed an unrelated local row and cleared its unsynced flag.

Even an ordinary apostrophe in an accepted string can poison a pull and prevent
the cursor from progressing. Parameterize every SQLite value, validate incoming
IDs/domain values on both sides, and explicitly constrain table/column mappings
for generic sync. Authentication alone does not fix this boundary.

Regression: malformed payloads must be rejected or stored literally, without
changing another row or blocking subsequent valid synchronization.

### R2 - High: push acknowledgements skip changes the client has never pulled

Locations: [client acknowledgements](../../crates/client/src/sync.rs), lines 49-60;
[server push response](../../crates/server/src/sync.rs), line 199;
[sync ordering](../../web/src/offline.ts), lines 110-123.

A client pushes its dirty rows, stores the server's current maximum sequence as
its pull cursor, then pulls strictly after that cursor. Any pre-existing changes
from other devices are skipped. This requires no race: another device can have
written yesterday. Rejected writes are also acknowledged without returning the
winning row, so a losing client can become clean while remaining divergent.

Push acknowledgement must not advance the download cursor. Return explicit
accepted/conflict outcomes and reconcile the authoritative winning version.
Only successfully applied pulls should advance the pull cursor.

Regression: A writes, B independently writes, B syncs, and both records appear
on B without a full reset. Include losing updates and tombstones.

### R3 - High: sequence allocation is not a safe committed-change cursor

Locations: [sequence trigger](../../migrations/0001_init.sql), lines 19-27;
[bottle pull](../../crates/server/src/sync.rs), lines 123-147;
[reference pull](../../crates/server/src/sync.rs), lines 85-106.

Two independent problems exist. Bottle pull selects rows and then queries a newer
maximum in a separate statement, so the response can acknowledge a row absent
from its payload. More fundamentally, concurrent transactions can allocate
sequences in one order and commit in the opposite order. The test held row 169
uncommitted, pulled committed row 171, committed 169, and then got an empty delta.
Reference pull's single query avoids the first race but not the second.

A repeatable-read snapshot alone does not fix commit-order gaps. Establish a
commit-safe ordering contract covering every writer. A simple candidate for this
app is transactional serialization of synced writes before sequence allocation,
combined with consistent pull boundaries; assess that against expected scale.
Do not generalize the present trigger scheme unchanged to the catalog.

[PostgreSQL's sequence documentation](https://www.postgresql.org/docs/16/functions-sequence.html)
explains that sequence allocation is independent of transaction rollback; the
commit-order failure above was additionally reproduced against Postgres.

### R4 - High: browser identities collide deterministically

Locations: [client source](../../crates/client/src/store.rs), line 13;
[ID generation on add](../../crates/client/src/handle.rs), line 23;
[ID factory](../../crates/core/src/ids.rs), lines 65-73.

Every browser uses source 2 and resumes from the maximum bottle tick. Two devices
with the same starting data generate exactly the same next ID. Two fresh clients
start at the same first ID. The later push overwrites or loses a physical bottle.
The dependency's factory is a logical incrementing counter here, not a source of
randomness or wall-clock uniqueness.

Keep causal IDs, but persist a distinct device/source identity and a durable
clock independent of the current row set. Define source provisioning, exhaustion,
device reset, and restoration behavior. Multiple server writers also need a
source/clock policy; the current server always uses source 1.

Regression: multiple devices starting from identical snapshots, offline creation,
restart, and restoration must never reuse an object identity.

### R5 - High: conflict resolution cannot break ties between row versions

Locations: [server conflict predicate](../../crates/server/src/sync.rs), lines 175-176;
[client merge predicate](../../crates/client/src/sync.rs), line 85;
[clock source](../../crates/client/src/handle.rs), line 13.

On a conflict on `id`, `excluded.id > bottle.id` can never be true: the IDs are
equal. Two different versions with equal millisecond timestamps are resolved by
arrival order on the server and retained differently on clients. `revision` is
neither compared nor validated. A higher revision at the same timestamp was
acknowledged but did not update the server in the test. Clock skew or a far-future
timestamp can also prevent subsequent ordinary changes from winning.

Define a version identity separate from immutable object identity, with the same
deterministic comparison on server and client. Decide whether conflicts should
automatically choose a winner or preserve both versions for selected fields.
Acknowledgements must identify the version sent, not only the object ID: once
editing exists, an in-flight acknowledgement must not clear a newer local edit.

### R6 - High: all data routes are unauthenticated and writes lack origin protection

Locations: [router](../../crates/server/src/main.rs), lines 59-70;
[server add](../../crates/server/src/web.rs), line 41;
[Compose ports](../../docker-compose.yml), line 26;
[development DB port](../../docker-compose.override.yml), line 7.

Any network client that can reach the server can read the inventory and submit
mutations. `/bottles` also accepts a cross-origin, form-shaped POST without
checking its origin. The test confirmed HTTP 200 for both anonymous reads and
that request shape; browser-specific private-network restrictions were not tested.
The default app and development database bindings expose all host interfaces.

Before exposure, select an authentication boundary, enforce authorization on
every read/write/sync route, remove the temporary server add route, and apply
origin/CSRF protection appropriate to the chosen authentication mechanism.
Use loopback bindings by default for local development. Define whether accounts
share one cellar or need isolated data; the existing user/session tables alone
provide neither authentication nor cellar isolation.

### R7 - High: sync input lacks domain validation

Locations: [wire DTO](../../crates/core/src/sync.rs), lines 8-17;
[push handler](../../crates/server/src/sync.rs), line 167;
[ID resume](../../crates/server/src/db.rs), lines 24-44.

The API accepts arbitrary-length/non-hex IDs, invalid status codes, negative
revisions, and unchecked timestamps. Tests persisted `arbitrary-id`,
`not-a-status`, and revision -5. Invalid IDs also undermine max-ID recovery:
an unparsable maximum becomes tick zero, and a maximum-width ID can saturate the
generator. One invalid lot causes HTTP 500 instead of a useful client error.

Introduce shared validated value types and explicit API validation/errors.
Validate references, deletion semantics, record sizes, and timestamp/version
rules before persistence. A pure sync sink still owns these invariants.

## Reliability and architecture findings

### R8 - High production-policy risk: reset can destroy the only copy of offline work

Locations: [epoch handling](../../crates/client/src/sync.rs), line 105;
[local reset](../../crates/client/src/store.rs), line 80;
[sync orchestration](../../web/src/offline.ts), lines 89-105.

An epoch change drops every local data table, including dirty bottles, before
replacement data has been successfully downloaded. This is intentional in the
existing development design, but means a server rebuild can destroy purchases
recorded offline that the server has never seen. Reset is also not transactional.
The old `default_lot` survives in `sync_state` until a successful bottle pull.

The metadata check fails open: network errors and non-OK responses do not stop
push/pull. Epoch is absent from the push/pull contract, so reset between separate
requests is not fenced. A restored older server backup retains its old epoch,
which also needs a recovery policy.

Separate an explicit development wipe from production recovery. Preserve or
export dirty data, validate epochs on data operations, and make reset/rehydration
recoverable. This policy needs an owner decision before implementation.

### R9 - Medium: writes, local applies, and cursor changes are not atomic

Locations: [server push loop](../../crates/server/src/sync.rs), line 167;
[client sync](../../crates/client/src/sync.rs), lines 55, 73, and 163;
[demo chain creation](../../crates/server/src/db.rs), line 65.

A push containing one valid row followed by an invalid FK returned HTTP 500 but
left the first row committed. Client apply/reset operations likewise commit
statement by statement. Crashes or errors expose partial batches; repeated retries
cannot provide a clear operation-level outcome. Reference reconciliation also
updates its dependency graph without a transaction.

Define the unit of atomicity. Use transactions for a catalog operation, a local
pull plus its cursor, and local reset metadata. If push is deliberately partial,
return explicit per-operation results and make retries version-aware. Do not
silently extend the current row loop to the producer/wine/vintage/lot/bottle graph.

### R10 - High availability: worker initialization failures leave requests pending forever

Locations: [worker ready signal](../../web/src/worker.ts), lines 16-19;
[main-thread readiness](../../web/src/offline.ts), lines 17-44;
[VFS installed flag](../../crates/client/src/store.rs), lines 18-24.

Readiness only resolves on success. There is no initialization-failed response,
worker error/messageerror handler, timeout, or rejection of pending RPCs. OPFS
failure or a failed WASM import leaves all local requests waiting indefinitely.
The VFS installed flag is set before installation succeeds, preventing a clean
retry within the same worker.

Each tab starts a worker against the same default SAH pool directory. The library
acquires exclusive sync access handles; there is no cross-tab ownership policy.
A second-tab initialization conflict is an additional expected failure path,
supported by dependency inspection but not browser-reproduced in this review.

Add an explicit starting/ready/failed state, propagate initialization failures,
reject pending calls on termination, and select a single-writer/multi-tab policy.
See the [SyncAccessHandle documentation](https://developer.mozilla.org/en-US/docs/Web/API/FileSystemSyncAccessHandle).

### R11 - Medium: synchronization has no reliable retry/scheduling lifecycle

Locations: [sync function](../../web/src/offline.ts), lines 82-97;
[HTTP result handling](../../web/src/offline.ts), lines 102-130;
[event triggers](../../web/src/offline.ts), lines 144-147.

If `refreshCount()` throws in `finally`, `syncing = false` is never reached. This
was reproduced. Requests arriving while sync is busy are discarded; a bottle
added after the dirty snapshot can remain unpushed until another user action or
reload. There is no periodic/focus pull for updates from another device, timeout,
or bounded retry/backoff. Non-OK HTTP responses are silently ignored, and users
cannot distinguish locally saved, pending, failed, or synced data.

Always release the running flag, retain a pending rerun signal, classify HTTP
errors, bound network waits, and expose durable sync state. Test changes arriving
during a push and the server recovering while the browser stays online.

### R12 - High offline-readiness risk: the HTML shell is not reliably available offline

Locations: [service worker](../../web/src/sw.ts), lines 10-35;
[registration](../../web/src/offline.ts), lines 150-152;
[precache list](../../web/vite.config.ts), line 33.

The worker is registered after the first page load, and HTML is not precached.
The initial navigation cannot have populated the worker's navigation cache.
Taking a fresh installation offline before another controlled online navigation
therefore leaves no cached shell. The cache write is also detached from the
event lifetime, and every HTTP response, including errors, can replace `/`.

Updates use immediate activation while the unversioned HTML shell can still refer
to old content-hashed assets. Coordinate shell and asset versions, explicitly
cache a validated shell, await cache writes with the event, and avoid replacing a
working shell with errors. Test first visit -> offline reload and application
upgrade -> offline reload in real browsers. These are code/lifecycle findings;
full browser reproduction remains outstanding.

### R13 - Medium: local fetch transport discards query parameters

Location: [fetch router](../../web/src/offline.ts), lines 67-72.

The worker receives `url.pathname`, never `url.search`, and an empty body for GET.
The test sent `/fragments/options?country=FR`; the worker received only
`/fragments/options`. This will break the planned country cascade/autocomplete,
including Datastar GET state. The transport also loses abort semantics and
cannot represent a local HTTP error: a resolved handler result becomes HTTP 200.

Define a small request/response contract retaining query, body, status, and
cancellation semantics before implementing the form. Keep URL routing and shared
rendering; a framework replacement is unnecessary.

### R14 - Medium: raw SQLite ownership and error handling leak resources and hide failure

Locations: [SQLite helpers](../../crates/client/src/sqlite.rs), lines 10-79;
[handler](../../crates/client/src/handle.rs), lines 19-53;
[sync functions](../../crates/client/src/sync.rs), line 19 onward.

Many `?` returns bypass `close(db)`. The `result` variable in `handle` does not
create a return boundary: `?` still exits the function. Failed open can leave a
handle unclosed, close ignores its return code, and query helpers turn prepare/
step failures into `None`, -1, or an apparently successful truncated result set.
That can masquerade as missing state and reset a cursor or ID clock.

Wrap connections and statements in RAII guards, parameterize values, return
typed errors with `sqlite3_errmsg`, and distinguish end-of-results from execution
errors. Restrict unsafe FFI to this layer. SQLite documents that unsuccessful
[connection cleanup can leave connections open](https://www.sqlite.org/c3ref/close.html).

### R15 - Medium: no local schema/protocol upgrade path

Locations: [local schema](../../crates/client/src/store.rs), line 30;
[wire contracts](../../crates/core/src/sync.rs);
[immediate worker activation](../../web/src/sw.ts), line 12.

`CREATE TABLE IF NOT EXISTS` does not migrate existing tables. Server epoch
identifies a database instance, not schema/API compatibility. Cached older
clients can send requests to newer servers, while newer WASM can open older
SQLite layouts. Adding the real catalog fields without versioned local migrations
will fail on existing installations or tempt destructive resets.

Introduce transactionally applied local schema versions and a compatible protocol
version policy. Keep database identity, schema version, and sync cursor separate.
Test upgrades with dirty offline rows, not only empty databases.

### R16 - Medium: sync volume is unbounded and lacks a progress contract

Locations: [server pulls](../../crates/server/src/sync.rs), lines 86 and 123;
[dirty extraction](../../crates/client/src/sync.rs), line 22;
[push DTO](../../crates/core/src/sync.rs), line 22.

Pulls fetch every changed row into memory and dirty extraction uploads everything
at once. There is no page/batch size, field-size cap, or continuation contract.
Axum's normal JSON body limit provides a transport ceiling, not batching: once an
offline backlog exceeds it, the client keeps retrying the same oversized body
without an actionable error. Generic catalog sync will make this easier to hit.

Add bounded batches and pages as part of the corrected cursor protocol, with
operation atomicity preserved and explicit retryable/permanent errors.

## Dependencies, build, and maintainability

### R17 - Medium: dependency advisories need triage and updates

Current RustSec audit: two vulnerability entries plus an unsoundness warning and
a yanked dependency. Dependency-tree inspection distinguishes exposure:

| Package | Finding | Assessment |
| --- | --- | --- |
| rustls 0.23.41 | [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285.html), patched in 0.23.45 | Used through sqlx-core; relevant when establishing DB TLS. It is not the app's HTTP TLS server. |
| event-listener 5.4.1 | [RUSTSEC-2026-0221](https://rustsec.org/advisories/RUSTSEC-2026-0221.html), patched in 5.4.2 | Used through sqlx-core; affected API misuse in this application's dependency path was not demonstrated. |
| rsa 0.9.10 | [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html), no published fix in the audit database | Present in lockfile; `cargo tree -i rsa` found no active default-build dependency path. Do not report this as a demonstrated application key-recovery vulnerability. |
| spin 0.9.8 | Yanked-package warning | Lockfile hygiene/feature-path review; not by itself evidence of an exploitable vulnerability. |

The RustSec database was updated on the review date. Frontend audit could not
complete: npm's advisory endpoint returned HTTP 503 maintenance after retries.
The frontend dependency security check remains open; the successful pnpm install
and supply-chain policy check do not substitute for it.

### R18 - Medium: production builds do not share the new development guarantees

Locations: [Dockerfile](../../Dockerfile), lines 5-29 and 32-41;
[Datastar download](../../web/scripts/vendor.ts), lines 11-20;
[new toolchain pin](../../rust-toolchain.toml).

Docker still uses floating Rust and Debian tags, does not copy
`rust-toolchain.toml`, and builds Cargo without `--locked`. The wasm-bindgen CLI
version is duplicated manually. The runtime runs as root. Datastar is fetched
on every build/watch from a tag URL without a committed checksum or bytes, so
successful locked package installation does not make the frontend build fully
reproducible or independently verifiable.

Align container toolchain/lock enforcement with development, run as an
unprivileged runtime user, verify vendored bytes, and define an update process.
No Docker image build or deployed TLS/reverse-proxy validation was performed in
this review. The newly added `just dev` also starts the DB without `--wait`;
explicit health waiting and a startup smoke test would make first-run behavior
more reliable.

### R19 - Medium: reference reconciliation does not handle removal/revival

Locations: [reference reconciliation](../../crates/server/src/seed.rs), lines 21-110;
[seed validation](../../crates/core/src/seed/mod.rs), lines 283-294.

Reconciliation upserts only rows still present in the RON. Removing an entry does
not tombstone it. An existing soft-deleted entry is not revived, because lookup
and comparisons ignore `deleted_at`; unchanged user-owned rows are not normalized
to seed ownership either. In validation, `condition: None` and
`condition: Some("default")` are different keys, while the server maps both to
the same database natural key.

Specify ownership, tombstoning, and revival rules, normalize natural keys before
validation, and reconcile transactionally. Preserve foreign-key/historical meaning
when retiring vocabulary rather than hard-deleting it.

### R20 - Medium: duplicate models and placeholder seams invite inconsistent behavior

Locations: [model](../../crates/core/src/model.rs), lines 12-39;
[generated vocabulary](../../crates/core/src/vocab.rs), line 15;
[Store trait](../../crates/core/src/store.rs), line 18;
[count](../../crates/server/src/web.rs), line 58;
[local count](../../crates/client/src/handle.rs), line 37.

Two `BottleStatus` definitions exist. Legacy `WineColor`/`WineStyle` contradict the
agreed descriptor model; the `Store` trait has no implementation and is not used
by the active handlers. Several comments and the README describe an earlier
architecture. Both counts include every non-deleted bottle, even consumed/gifted
ones, while the UI labels the result "Bottles in cellar."

Remove obsolete public scaffolding, consolidate validated types, define inventory
count semantics, and update architecture documentation. Shared rendering is useful;
shared validated contracts matter more than an unused storage abstraction.

## Decisions to settle before catalog work

These are design questions or missing guarantees, not claims that unimplemented
features have already failed:

- A single shared cellar versus separate account/cellar ownership. This determines
  authorization, sync scope, cache partitioning, and logout behavior.
- Recovery expectations for offline writes after server reset or backup restore.
- Supported device/browser set and behavior when a second tab owns OPFS, storage
  is unavailable, or browser storage is evicted. No persistent-storage request,
  export path, or visible durability state currently exists.
- Conflict policy and device source allocation while keeping causal IDs.
- Non-vintage identity: `UNIQUE(wine_id, year)` allows multiple NULL years in
  Postgres, while `wine.is_nv` duplicates information implied by vintage year.
- Re-adding soft-deleted natural keys: existing uniqueness rules still include
  tombstones. Revival versus new identity must be explicit.
- Validation of country-scoped relationships, classification applicability,
  grape/clone consistency, position bounds, percentages, and hierarchy cycles.
  Ordinary foreign keys alone do not enforce these domain relationships.
- Transaction and retry boundaries for one entry that creates a catalog chain
  and N physical bottles. User-catalog bidirectional sync is still unimplemented.

## What is worth preserving

The crate boundary keeps core rendering free of server I/O, Askama escapes normal
template values, server SQL uses bind parameters, reference data has natural keys
and validation, and vocabulary generation avoids duplicated hand-maintained lists
in active enum code. The source tree is small enough to correct these contracts
without a platform rewrite. The pinned development environment successfully
builds both execution targets and checks the frontend.

## Inputs to the roadmap discussion

Proposed ordering to discuss, not a finalized roadmap:

1. Protect existing data paths: bound/validate inputs, parameterize SQLite,
   establish the deployment trust boundary, and capture the reproduced failures
   as regression tests.
2. Correct the sync contract: unique device sources, versioned mutations,
   commit-safe cursors, acknowledgements, transactions, and reset/recovery rules.
3. Make browser storage dependable: initialization errors, multi-tab ownership,
   durable pending writes, retry/status behavior, shell lifecycle, and migrations.
4. Build transactional catalog sync and the agreed bottle-entry experience.
5. Finish release operations: dependency gates, container hardening, TLS,
   backup/restore rehearsal, and browser/device acceptance tests. Security
   requirements needed for the selected deployment move earlier, not to the end.

The release gate should be behavioral: two devices converge under delay, retry,
conflict, and reordered commits; crashes do not acknowledge unapplied data;
offline writes survive upgrades/recovery; unauthorized or malformed inputs do not
alter data; and a first-time installed app actually reopens offline. The current
seven unit tests establish none of those guarantees.
