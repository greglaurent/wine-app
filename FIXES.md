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

- Tentative product name: **Barback**. The planned application hostname remains
  `wine.the-fold.net` regardless of this name change. Repository/package renaming
  and branding changes are not implemented by this planning decision.
- Immediate milestone: get the application running locally and test from desktop
  and Android on the same home network. Public access for the limited invited
  users is planned shortly after local testing, rather than an indefinite future
  possibility.
- First-release product scope is wine. Spirits and other alcohol categories come
  later; do not add category-specific workflows or generalize the initial product
  around them before the wine experience is complete.
- Initial collection entry is manual; spreadsheet/export import is not required
  for the first release. Photo/label recognition and barcode lookup are desirable
  secondary features, deferred until suitable reference/lookup data is identified.
  Their data sources, coverage, licensing, costs, and integration need separate
  evaluation; no provider is selected or required for the manual-entry workflow.
- Initial scale: two to four test users, potentially for an extended period. This
  is both a production-grade web development learning project and an alternative
  to existing wine/alcohol management applications. Design for secure growth
  without requiring an enterprise-scale deployment at launch.
- Initial hosting: local server, followed by public HTTPS access through
  Cloudflare Tunnel, consistent with the owner's other NAS applications.
  Configuring or opening that tunnel is a subsequent deployment milestone, not
  part of the current planning work. Cloudflare Workers is not the initial
  deployment target. Use a suitable secure local setup for Android/browser
  testing; decide hostnames and certificates before enrolling real passkeys.
- Planned HTTPS hostnames, under the owner's `the-fold.net` domain:
  `keys.the-fold.net` for Keycloak and `wine.the-fold.net` for wine-app.
  These are tentative until deployment configuration is finalized. Keep the
  authentication hostname stable across local testing and tunnel deployment
  where practical; finalize it before real passkey enrollment. DNS, certificates,
  local routing, and Cloudflare Tunnel configuration have not been provisioned
  by this planning decision.
- Initial client: a responsive, mobile-first website, with Android as the first
  mobile platform and Firefox and Chromium-based browsers as initial browser
  targets. No native Android application is planned for the initial release.
  iOS/Safari support is outside the initial target. Browser installation must
  not be required to use the website; any optional install experience is separate.
  Validate offline storage, worker behavior, passkeys, and responsive layouts on
  the target browsers rather than assuming native-app capabilities.
- Deliver one cohesive application with polished desktop and mobile experiences.
  Adapt layout and interaction to each screen/input mode while keeping the same
  account, data, permissions, offline behavior, and core workflows. Desktop is a
  first-class experience, not merely a stretched mobile layout.
- Support normal use across multiple browser tabs. Coordinate local database
  access and synchronization, propagate changes and authentication state between
  tabs, and recover when a coordinating tab closes or crashes. Do not require
  users to manage a single-tab restriction. The coordination mechanism remains
  an implementation decision; duplicate requests must still be safe to retry.
- Authentication provider: self-hosted Keycloak, integrated with Rust/Axum through
  OpenID Connect (OIDC). Operating the identity service is an accepted tradeoff.
  Keep the integration standards-based to allow a later provider change; this
  does not imply that credentials, passkeys, or account mappings migrate
  automatically.
- Support both private cellars and shared cellars with membership roles.
- Authentication, authorization, and HTTPS remain requirements before public
  release. Local development must preserve the agreed access boundaries and use
  an appropriate secure browser context for the features being tested.
- Keep Rust/Axum/Postgres, Datastar, shared Askama rendering, and the WASM/SQLite
  offline client. Correct the existing security, sync, and recovery contracts
  before expanding catalog writes.
- Keep causal IDs. Fix device identity and mutation ordering rather than replacing
  the identifier scheme.
- Use the owner's `causal-id` crate from the `abmac-io` Kellnr registry at
  `sparse+https://kellnr.abmac.io/api/v1/crates/`. The local reference source is
  `~/Workspace/abmac-io/abmac-lib/causal-id`; use the published registry package
  rather than a machine-specific path dependency. Target version 1.1.0 with
  `width-u128` on both native and WASM builds, preserving the existing 32-character
  hex representation. Package adoption alone does not resolve R4 or R5: distinct
  source allocation, durable clocks, remote-clock observation, and mutation
  versioning remain required. Use the library's fallible constructors when
  handling persisted or untrusted values, and define overflow handling.
- The published `causal-id 1.1.0` width features do not forward to `pointer-width`.
  Wine-app explicitly enables `pointer-width/width-u128` and checks the resulting
  width at compile time. An upstream feature-forwarding fix can remove this
  direct dependency later; the upstream library is unchanged by this integration.
- Build ownership boundaries into server access, synchronization, and local
  storage from the beginning.

## Authentication decision and remaining design

Keycloak and OIDC are selected; authentication implementation has not begun.
Passkeys are the primary sign-in method, chosen to reduce the burden of entering
and managing passwords. Password fallback has not been selected. Recovery uses
a link sent to the account's previously verified email address.
The desired sign-in experience is easy cross-device access, ideally scanning a
QR code on a computer and approving on a phone. Evaluate the standard cross-device
passkey flow against the actual supported browsers/devices. This preference does
not yet settle the exact credential-enrollment implementation.

Keep credential revocation, session/token expiration, and cellar-membership
revocation distinct. Passkeys do not inherently provide expiring access. Define
and test how removing a credential or recovering an account affects existing
Keycloak sessions, wine-app sessions, and issued tokens; do not assume deleting
a passkey invalidates all previously authenticated access. Lost-key recovery
uses the shared activation flow below.

Signing in with an available trusted device is distinct from recovering after
losing all credentials. Self-service recovery through a verified email link is
selected from the initial release, rather than requiring administrator-assisted
recovery. The intended flow is: request recovery, receive a link at the previously
verified address, validate the link, and enroll a replacement passkey on the same
account. Recovery must preserve account identity and memberships and must not
enable public registration.

Use one consistent user activation flow for initial activation, account
reactivation, account recovery, and lost-key recovery: receive an email link,
validate it, enroll a passkey, and enter the app. Share the user-facing experience
and implementation rather than introducing separate recovery mechanisms.
Server-side eligibility remains specific to the account state: a new account
requires a valid invitation (or the explicit administrator bootstrap); initial
email verification uses that authorized address. Existing-account recovery and
reactivation use the previously verified address and preserve the same account
identity. Reactivation does not recreate revoked memberships or automatically
restore archived/deleted cellars. The bootstrap user also uses this enrollment
experience after being explicitly provisioned.

On successful replacement-passkey enrollment during recovery or reactivation, invalidate all
previous passkeys and all previous Keycloak and wine-app sessions. Other devices
must authenticate again. Invalidate previously issued access and refresh tokens
for wine-app access as part of this boundary; session logout alone is not evidence
that existing bearer tokens can no longer be used. Merely requesting recovery or
opening its link must not revoke access. Do not mark recovery complete until the
replacement credential is usable and the invalidation boundary is enforced.
This does not remotely erase data on disconnected devices.

Recovery implementation requirements:

- [ ] Exercise initial activation, reactivation, recovery, and lost-key recovery
  through the shared flow; verify invitation gating for new accounts and that
  reactivation cannot restore previously revoked access.
- [ ] Verify the recovery email during invitation-gated enrollment before enabling
  it for recovery; protect subsequent recovery-address changes.
- [ ] Use single-use, expiring recovery links bound to the account and recovery
  action, with rate limiting and responses that do not reveal account existence.
- [ ] Configure production email delivery and validate the full Keycloak recovery
  and passkey-enrollment flow; do not assume a password-reset flow is sufficient.
- [ ] Test successful recovery against old passkeys, Keycloak and wine-app
  sessions, access tokens, and refresh tokens; verify they cannot regain server
  access. Test failures and retries so recovery cannot report partial success.

Ordinary sessions have a maximum lifetime of 30 days and expire after seven days
of inactivity, whichever occurs first. Require fresh authentication for sensitive
account or ownership changes. These are session limits, not passkey expiration
or permission to issue 30-day access tokens. Coordinate Keycloak and wine-app
enforcement so refresh cannot extend a session past its absolute lifetime.
The exact sensitive-action list, authentication freshness window, and definition
of activity (including background sync) remain implementation details to specify.
Server session expiration is distinct from offline-cache retention; do not infer
cache deletion solely from an expired session.

After session expiry, allow continued offline access to already-downloaded cellar
data and retain queued offline edits in that account's local storage. Offline
editing remains bounded by the last-known role; expiry grants no new permissions.
Pause authenticated server operations until the user signs in again. Before
resuming sync, verify the same account identity and current cellar permissions;
do not upload one account's queued changes under another account's session.
Confirmed revocation still follows the cache-removal policy below. Expiry alone
must not discard pending work or be treated as confirmed revocation.

Connectivity, authentication, and explicit logout are separate states. Network
loss, timeouts, server failures, and inability to reach Keycloak must never be
interpreted as a user logout or trigger local-data cleanup. An authentication
failure alone is not proof of membership revocation. Preserve cached records and
pending edits through these conditions. Explicit logout also preserves the local
database, pending edits, and sync cursors; account switching must not purge them
either. Logout ends access through the signed-in UI and stops authenticated sync
without deleting persisted data. Isolate retained state by account so another
signed-in user cannot view or upload it. Session expiry retains the previously
agreed offline access; deliberate logout is a distinct user action.

After the same account signs in again and current authorization succeeds, resume
incremental sync from its retained state. Neither the 30-day maximum session
lifetime nor ordinary logout should force a full download. Protocol/epoch recovery
remains a separate case whose pending-work preservation policy is still open.

Local data removal is reserved for explicit removal/reset actions and confirmed
loss of cellar access. Leaving a cellar or confirmed membership revocation clears
only that account's affected cellar data and sync state, not other cellars.
For this web client, explicit browser site-data removal is a cleanup path. If
an optional installed-web experience is provided, verify its removal behavior
rather than assuming it clears browser storage.

Provide a per-cellar **Rebuild local data** action. Preserve unsynced revisions
and their operation grouping durably, download fresh authorized server state,
then reconcile the preserved edits through the normal validation and conflict
rules. Do not implement this as clearing the database before saving pending work.
Keep the existing usable state until a replacement can be installed safely;
interruption, network loss, or failed authorization must not leave partial state
or silently discard edits. Scope the rebuild to the current account and selected
cellar, coordinate it across tabs, and leave other cellars untouched. Preserve
revision IDs, edit timestamps, and device clock/source state rather than issuing
new identities for replay. Confirmed revocation still follows its separate policy.

Show offline/network-unavailable status when authentication cannot be checked;
do not claim the user must sign in again solely because a request failed.

When reauthentication is required, show a persistent, visible indication that the
user must sign in again and synchronization is disabled, with a sign-in action.
Keep offline availability clear and do not imply queued changes have reached the
server. Proposed wording is "Sign in to resume syncing" and "Sync paused", with
a pending-change count; exact presentation remains part of UI design.

- [ ] Verify idle and absolute session expiry, refresh behavior, and fresh
  authentication for sensitive changes across Keycloak and wine-app.
- [ ] Verify session expiry preserves cached access and pending edits while
  blocking sync until reauthentication and current authorization succeed; test
  changed roles, revoked membership, and signing in as a different account.
- [ ] Show the reauthentication and paused-sync state while preserving offline
  work; clear it only when authenticated synchronization can actually resume.
- [ ] Test network loss, timeouts, server/identity-provider failures, and session
  expiry independently from explicit logout. None may purge persisted local data
  or discard pending edits; distinguish them from confirmed membership revocation.
- [ ] Verify explicit logout/account switching retain isolated account data and
  cursors, prevent cross-account access, and resume incremental sync after the
  same account signs in again. Test cellar-scoped cleanup independently.

Keep cellar ownership,
roles, and invitations in wine-app rather than coupling them to provider-specific
organization features. A Cloudflare Tunnel provides connectivity; application
authentication and membership authorization remain required.

Account registration is invitation-only initially. Bootstrap exactly one initial
administrator account for the project owner (Greg). Public self-registration is
outside the current scope and must not be enabled as part of this implementation.
An invited new user must be able to enroll and authenticate before accepting
cellar membership; an existing user signs in with their own account. The exact
enrollment mechanism remains to be designed, with server-side enforcement of the
invitation requirement rather than merely hiding a registration link.

Administrator privileges and per-cellar Owner membership are distinct concerns.
The app administrator can manage accounts and service settings, but viewing or
editing cellar contents requires membership and the corresponding cellar role,
just as for any other user. App administration grants no implicit data-access
bypass or right to self-assign cellar membership. Enforce this across API, sync,
history, and restoration paths, not just the UI. Cellar owners do not thereby
receive identity-provider administration privileges. The exact Keycloak bootstrap
privileges and operational administration setup remain implementation details.

## Cellars, accounts, and memberships

A cellar is the access boundary. A private cellar starts with one owner; sharing
adds memberships to that same cellar. A user can belong to multiple cellars and
have a different role in each. Access is attached to the recipient's own account,
never provided by sharing the owner's credentials.

Any activated user can create a new cellar, including additional cellars, and
becomes its Owner. Create the cellar and its initial Owner membership atomically.
This does not open public account registration; the invitation-only account
policy remains in effect.

Members managing the same cellar share its catalog, inventory, purchases, storage
locations, and tasting notes according to their roles; these are not isolated
per spouse or per account within that cellar. Tasting-note authorship still
restricts editing as specified below.

Initially, user-entered catalog data (including producers, wines, vintages, and
custom grape details) belongs to the cellar in which it is entered. Do not share
it automatically with unrelated cellars. A cross-cellar shared catalog is
deferred. Server-maintained reference vocabulary remains distinct from these
user-entered records; reference use must not expose another cellar's catalog.
Physical storage locations belong within a cellar and do not define its access
boundary. One cellar may contain multiple physical locations, such as a kitchen
rack, basement, or off-site storage, with optional finer placement such as shelf
or bin. These locations share the cellar's membership and role rules; they are
not separate cellars merely because they are physically separate.

Support grid-based storage with explicit row and column coordinates, for example
Row 1 / Column A, scoped to a particular rack or storage grid within the cellar.
Each individual slot holds at most one active bottle. Also support broader
locations such as shelves or boxes holding multiple bottles without mandatory
individual-slot assignments. Use stable slot identities with row/column metadata
so occupancy and history do not depend on parsing display labels. Exact grid
configuration and presentation remain design work.

Grid setup requires user-supplied configuration for each rack/grid: its name,
row count, column count, and row/column labeling needed to identify physical
positions. Generate addressable slots from that configuration rather than
assuming a universal cellar layout. Allow multiple configured grids within a
cellar alongside non-grid storage. Configuration changes must preserve existing
bottle assignments and history or explicitly resolve affected placements; never
silently drop occupied slots. The detailed setup UI remains design work.

Only Owners can define or resize racks/grids. Editors and Owners can place and
move bottles within configured storage; Viewers remain read-only. Enforce these
permissions on the server, including synced offline operations, independently
of whether the UI exposes configuration controls.

Initial roles:

| Role | Access |
| --- | --- |
| Owner | Manage the cellar, members, invitations, and inventory; create and edit own tasting notes; recoverably remove own or others' notes |
| Editor | Add and update inventory; create, edit, and recoverably delete own tasting notes; mark bottles consumed, gifted, or otherwise removed |
| Viewer | Read cellar records |

Initially, only owners can invite. Invitation and membership operations must not
grant more authority than the inviter possesses. Detailed destructive-operation
rules still need specification; the role table does not implicitly settle them.

Every active cellar must retain at least one owner. Its sole remaining owner
cannot leave, be removed, or be demoted until another member becomes an owner
or the cellar enters the archived lifecycle described below.

Multiple owners are allowed. An owner can promote an existing cellar member to
Owner through member management. After that promotion succeeds, the initiating
owner can remain an owner, step down to Editor, or leave. Each operation must
preserve last-owner protection; an attempted or failed promotion does not permit
the sole owner of an active cellar to step down or leave.

### Whole-cellar lifecycle

An Owner's "delete cellar" action initially archives the cellar: its data becomes
read-only and a current Owner can restore it. Lifecycle operations such as
restoration and owner departure remain possible despite the data being read-only.

Archived cellars are an exception to last-owner protection. Owners may remove
themselves or delete their accounts. When the last owner departs, transition the
archived cellar to deleted, pending retention cleanup, even if Editor or Viewer
memberships remain. It is no longer available as an ordinary read-only cellar.
Retain its records until the future retention policy purges them.

That future cleanup removes the deleted cellar's records and memberships but
never purges associated accounts, even if they have no other active cellar.
Accounts remain available for possible restoration/reactivation. Cellar cleanup
must not affect a user's other cellars or their data. Retention timing remains
deferred.

An account-deletion request is therefore a recoverable account deactivation,
not permanent erasure of the identity record. Before deactivation, a user who is
the sole owner of any active cellar must transfer ownership or explicitly archive
that cellar. Do not silently archive an active cellar as a side effect. Apply the
archived last-owner transition above when the account departs. Deactivation must
block authentication and invalidate existing access; account reactivation must
not imply restoration of data already purged by retention. Reactivation uses the
shared email-verification and passkey-enrollment flow above; its implementation
and coordination with Keycloak remain to be designed.

### Record deletion and restoration

Routine removal from inventory is a lifecycle change, not deletion of the bottle
record. Editors and owners can mark bottles consumed, gifted, or otherwise
removed while retaining their records. Deleting an inventory record entered by
mistake is restricted to owners and must remain recoverable. Any current Owner
can restore a deleted inventory record; Editors and Viewers cannot. The exact
lifecycle values and restoration UI remain to be specified.

Retain recoverably deleted records indefinitely initially, with no automatic
permanent deletion. A data-retention policy is deferred as an internal concern,
not a prerequisite decision for the initial release. The eventual whole-cellar
cleanup scope is agreed above, but no purge job is to be enabled before its
retention policy and account safeguards are specified.

Tasting-note content is editable only by its author, subject to current cellar
write access. Cellar owners can recoverably remove inappropriate or mistaken
notes but cannot rewrite another person's tasting opinion. Author identity must
not be reassigned to bypass this restriction. Authorship does not override a
membership revocation or the read-only Viewer role. Authors can recoverably
delete their own notes while they retain Editor or Owner access to the cellar.
Authors can restore notes they deleted themselves while they retain Editor or
Owner access. Restoring a note removed by an Owner requires current Owner access;
authorship alone cannot undo an Owner's removal. Preserve server-validated
deletion provenance so restoration checks can distinguish these cases.

Tastings support optional free-text notes and an optional rating displayed on a
0–5 star scale in 0.1-star increments: 0.0, 0.1, 0.2, through 5.0. Neither is
required to record bottle consumption. An absent rating is distinct from an
explicit zero. Store the equivalent 0–100 integer score: each 0.1 star equals
two points (4.2 = 84; 4.5 = 90; 5.0 = 100). User-entered ratings therefore map
to even integer scores. Validate range and increments consistently on client
and server and avoid floating-point drift. Display the numeric rating to one
decimal place alongside fractional stars; do not round to half or whole stars.

Allow multiple dated tasting entries for the same wine/vintage, optionally linked
to a specific physical bottle. Each tasting has its own stable record identity,
author, tasting date, notes, and optional rating; a new tasting does not overwrite
an earlier experience. Different physical bottles of the same wine/vintage can
be consumed years apart. Present their dated ratings and notes so users can
follow how that wine develops over time. Keep tasting date separate from revision
edit time and sync time: correcting an old note must not move that tasting to
the correction date in its history or trend. Missing ratings are not zero scores.

In shared cellars, show each person's dated rating trend separately by default.
Offer an optional combined cellar view while preserving attribution so individual
preferences remain visible. Any aggregate presentation must distinguish itself
from individual ratings; the combined-view design remains UI work.

Required enforcement:

- [ ] Enforce tasting-note authorship and current write access on the server for
  edits, including sync. Reject edits to another author's content even by an
  Owner; support recoverable Owner removal without rewriting the original note.
  Permit recoverable deletion by the author only with current Editor or Owner
  access, including when an offline deletion is submitted after a role change.
- [ ] Enforce note restoration permissions using current membership and deletion
  provenance. Reject author restoration of an Owner-removed note without current
  Owner access, including stale offline restores; restoration must not change
  authorship or rewrite the original content.
- [ ] Distinguish bottle lifecycle updates from recoverable record deletion in
  the API and sync protocol. Reject deletion and restoration by Editors and Viewers on the
  server, including queued offline operations and direct requests.
- [ ] Authorize owner promotion on the server and support ownership transfer
  through promotion followed by an optional demotion to Editor or departure.
- [ ] Enforce last-owner protection atomically on the server, including concurrent
  leave, removal, and demotion requests; two owners acting simultaneously must
  not leave an active cellar with no owner.
- [ ] Enforce archived cellar read-only access on the server and in sync; only
  Owners may archive or restore a cellar. Atomically mark an archived cellar
  deleted when its final owner leaves or deletes their account. Reject subsequent
  data access and writes, including stale offline operations, and apply the
  agreed cache-removal behavior on reconnection.
- [ ] Authenticate data requests and use secure session handling.
- [ ] Authorize every server read, write, and sync operation against the account's
  current membership and role in the requested cellar.
- [ ] Verify that referenced records belong to the authorized scope; accepting a
  cellar identifier from a client is not authorization.
- [ ] Keep private and shared cellar data isolated across accounts and memberships.
- [ ] Scope local data and sync state by account and cellar; logout and account
  switching retain persisted data but must not expose it to another account.
- [ ] Keep server-owned reference vocabulary distinct from cellar-owned user data.
  Scope user-entered catalog reads, writes, references, and sync to the cellar;
  test sharing among authorized members and isolation from unrelated cellars.
- [ ] Test direct API calls and forged cross-cellar references, not only UI access.

## Simple invitation flow

The agreed interaction is **Invite -> specify recipient email and role -> send
link -> recipient enrolls or signs in and accepts**. Acceptance gives the recipient
a membership with the selected access to that cellar. It must remain a short flow
for the owner and recipient.

Each invitation is bound to a specific recipient email address. Acceptance
requires the authenticated account's verified email to match that recipient.
Forwarding the invitation link alone must not grant another account access.

Invitations are single-use and expire seven days after issuance. Owners can
revoke pending invitations. Reissuing an invitation for the same cellar and
recipient invalidates the previous pending link and starts a new seven-day
validity period. Successful acceptance consumes the invitation; retrying that
same acceptance must remain safe without granting access a second time.

Creating or reissuing an invitation automatically sends an email to its intended
recipient. Also provide a **Copy link** option for direct sharing. Both delivery
paths use the same invitation and retain its verified-email binding, expiry,
single-use behavior, and revocation rules. Copying a link does not reissue it or
extend its validity.

Invitations never overwrite an existing cellar membership's role. If the
recipient already belongs to the cellar, preserve their current role and direct
the owner to member management for an explicit role change. Enforce this at
acceptance as well as invitation creation so concurrent membership changes or
previously issued links cannot silently promote or demote an existing member.

- [ ] Limit invitation creation and management to owners initially.
- [ ] Associate each invitation with its cellar, selected role, and inviter.
- [ ] Require the recipient to sign in before accepting.
- [ ] Enforce the verified recipient-email match on the server for enrollment
  and acceptance; test forwarded links and accounts with unverified emails.
- [ ] Enforce seven-day expiry, single-use acceptance, owner revocation, and
  invalidation of the previous pending link on reissue, including concurrent
  acceptance/revocation/reissue attempts.
- [ ] Validate invitation state and the inviter's authority on the server when
  accepting; a link must not bypass membership authorization.
- [ ] Make acceptance atomic and safe to retry without duplicate memberships or
  unintended privilege changes; test existing memberships with different roles
  and membership creation concurrent with invitation acceptance.
- [ ] Show pending invitations and current members in a simple management view.
- [ ] Send invitation emails automatically and provide Copy link; expose delivery
  failures so the owner can retry or share the link directly without accidentally
  changing the invitation's permissions or validity.

There is no existing SMTP service selected. Email delivery setup is an explicit
deployment task, targeting TrueNAS and likely a container managed through Komodo.
The exact mail software and direct-delivery versus upstream-relay arrangement
remain implementation choices. This records intended application behavior;
no invitations or messages are to be sent during this planning discussion.

- [ ] Set up email delivery on TrueNAS, preferably using the owner's Komodo
  container workflow; configure sender identity, credentials, and required domain
  mail records for the chosen delivery arrangement.
- [ ] Connect wine-app and Keycloak to the service and verify invitation,
  activation, and recovery delivery end to end before inviting real users.
  Local development may use a mail-capture service until delivery is ready.

## Agreed primary navigation

Open directly to searchable, filterable inventory rather than a statistics
dashboard. Prioritize finding wine already owned to drink, adding bottles and
purchases, and recording consumption, with prominent Add bottles and Record
consumption actions. Adapt these core workflows for polished desktop and mobile
use. Analytics and tasting trends are supporting views, not the default landing
experience. Exact layout and filter controls remain design work.

Establish a new Barback design for the first usable milestone. The current UI
was strictly for early testing and is not the design baseline to refine or
preserve. Design the agreed workflows coherently for desktop and mobile, including
inventory, bottle entry, storage grids, consumption, authentication, and visible
offline/sync states. Existing functional requirements remain applicable, but
prototype layouts, styling, and interactions are not binding. Visual direction
and detailed interaction design remain to be developed with the owner.

Start with reviewable desktop/mobile mockups and simple clickable flows using
static sample data. The first design prototype should validate rendering,
navigation, hierarchy, and how the workflow feels; it does not need real data
mutation, authentication, or synchronization. Simulate necessary states clearly
without treating prototype actions as implemented production behavior. Review
reference designs with the owner before committing to a visual direction.

Researched award-winning references:

| Reference | Verified recognition | Proposed Barback inspiration |
| --- | --- | --- |
| [Things](https://culturedcode.com/things/features/) | [Apple Design Award 2017](https://culturedcode.com/things/blog/2017/06/back-from-wwdc/) | Restrained lists, hierarchy, and focused entry interactions |
| [Crouton](https://crouton.app/) | [Apple Design Award 2024, Interaction](https://developer.apple.com/design/awards/2024/) | Collection organization and approachable task flows |
| [Flighty](https://flighty.com/) | [Apple Design Award 2023, Interaction](https://developer.apple.com/design/awards/2023/) | Clear status hierarchy and progressive detail |

Pocket Casts was considered and rejected by the owner based on prior experience;
do not use it as a design reference. The owner selected **Crouton and Flighty**
as the primary references, combining their approaches where appropriate to each
workflow. Use Crouton as guidance for collection browsing and approachable entry
flows, and Flighty primarily for the dashboard and statistics experience, with
clear status, information hierarchy, and progressive detail. Establish one coherent Barback design rather than separate visual
styles per screen. Things remains background research, not a selected direction.
The owner supplied the [starting palette](https://coolors.co/44355b-31263e-221e22-eca72c-ee5622):
purple `#44355B`, deep purple `#31263E`, charcoal `#221E22`, gold `#ECA72C`,
and orange `#EE5622`. Preserve these brand colors, with contextual variants where
needed for readability. Approved warm white: `#FAF7F2`. Proposed dark orange for
text on warm white: `#96330E`. The palette and contrast guidance are saved as the
design starting point; the owner will review the contextual variants and visual
treatment more closely later. See the saved
[light/dark palette preview](docs/plans/cellarius-palette-preview.html).

Calculated WCAG sRGB contrast for opaque, solid color pairs:

| Pair | Ratio | Intended use |
| --- | --- | --- |
| Warm white / charcoal | 15.39:1 | Primary light/dark text and backgrounds |
| Warm white / deep purple | 13.30:1 | Dark surfaces and light text |
| Warm white / purple | 10.31:1 | Light-theme links, headings, purple buttons |
| Charcoal / gold | 7.93:1 | Gold actions with dark labels; dark-theme accent text |
| Dark orange / warm white | 7.08:1 | Contextual orange text in light theme |
| Charcoal / original orange | 4.69:1 | AA normal text, below the preferred 7:1 target |
| Original orange / warm white | 3.28:1 | Not normal-size text; limit to suitable non-text accents |
| Gold / warm white | 1.94:1 | Decorative only unless a contrasting boundary/label supplies the required information |

Target WCAG 2.2 AA throughout and enhanced 7:1 contrast for normal reading text
where practical. Normal text requires at least 4.5:1; qualifying large text and
required non-text control/state indicators require 3:1 under their respective
criteria. The dark brand colors contrast only 1.16–1.49:1 with one another;
do not rely on those differences alone to identify required control boundaries.
Use accessible contextual borders/focus rings and pair status colors with text
or icons. Recheck actual hover, focus, selected, error, and composed surfaces;
these pair calculations do not establish whole-interface accessibility.
Sources: [text minimum](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html),
[enhanced text contrast](https://www.w3.org/WAI/WCAG22/Understanding/contrast-enhanced.html),
[non-text contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html).

These are references for interaction and visual hierarchy, not templates to copy
or platform requirements. Apply suitable patterns to responsive Firefox/Chromium
web layouts and Android touch use. Award dates refer to the recognized designs;
current product screenshots may reflect later evolution.

The chosen visual direction is clean, functional, modern, and visually simple.
Prioritize usability and mistake prevention over decorative complexity or dense
dashboards. Keep primary tasks obvious, make context (cellar, bottle, location,
quantity, and save/sync state) clear, validate inputs with actionable feedback,
and provide safe recovery from mistakes. Use proportionate confirmation for
destructive actions without adding friction to routine work. These are design
goals backed by validation and authorization, not a claim that errors are
impossible. Apply them to both desktop and touch interactions.

Support light and dark themes. Follow the device/system preference by default
and provide a manual override. Maintain readable contrast and clear focus,
validation, selection, and sync-status indicators in both themes; theme choice
must not change workflow behavior.

Provide immediate **Undo** for routine actions such as consuming or moving a
bottle rather than requiring confirmation on each action. Undo creates a new
revision/operation under current permissions; it does not erase history or
blindly replay an old snapshot. Check for intervening changes and current slot
occupancy so Undo cannot overwrite newer work or reclaim a slot occupied by
another bottle. Explain when a direct undo is no longer possible and offer a
safe resolution; do not report success for a partial reversal. Routine Undo is
part of the first usable workflow even though full history browsing comes later.

Automatically preserve unfinished form drafts locally, scoped to the account and
cellar. Require explicit **Save** or **Add bottles** to commit domain changes.
Typing and draft autosave do not create committed revisions, alter inventory, or
enter normal sync. At explicit save, validate and atomically create the new full
revision(s) and operation, assigning their edit timestamps and causal IDs then.
Retain draft state through interruption until a successful local commit or an
explicit discard; avoid duplicate commits on repeated submission. Restore drafts
only in their proper account/cellar context and recheck permissions and newer
record state before committing. Distinguish Draft saved locally, Saved locally /
pending sync, and Synced in the UI.

Include a secondary dashboard for useful information and insights. It must not
replace inventory as the landing experience or obstruct finding, adding, placing,
and consuming bottles. Dashboard contents and timing remain to be prioritized;
the initial milestone stays focused on the agreed end-to-end core workflow.

The default inventory view shows bottles currently available to drink. Provide
easy filters for consumed, gifted, and otherwise removed bottles, preserving
access to their history. Default inventory quantities count available bottles,
not every historical bottle record or revision. Recoverably deleted mistaken
records follow their separate restoration workflow.

During normal signed-in use, open the last-used accessible cellar regardless of
role. After an explicit logout followed by sign-in, instead default to the
last-used cellar in which the user currently has Owner access. Remember these
two choices separately per account; visiting an Editor/Viewer cellar changes
the ordinary last-used choice without replacing the last-used owned choice.
While explicitly logged out, show sign-in rather than cellar contents, despite
retaining local data. Session expiry or temporary network loss is not explicit
logout and must not trigger this change of default.

Provide an easy cellar switcher and an optional All my cellars search, limited
to authorized cellars and showing each bottle's cellar and physical location.
Validate membership and cellar lifecycle before using a remembered default.
After sign-in following explicit logout, if the user has no accessible active
owned cellar, show the cellar picker and let them choose. Do not automatically
fall back to the last-used Editor/Viewer cellar.
If there are no accessible cellars to choose from at all, show an empty state
with **Create new cellar** as the way forward, consistent with the permission
for any activated user to create a cellar and become its Owner.

## Agreed vintage representation

Vintage is an optional year: enter it when known, otherwise leave it unset.
Do not introduce separate Unknown and Non-vintage states or require the user to
choose between them. Use a single absent-year representation consistently in
validation, display, identity matching, and synchronization; do not use a fake
year as a sentinel. Detailed catalog uniqueness rules remain schema work.

## Agreed bottle identity and grouping

Each physical bottle has its own stable logical causal ID, independent of its
revision IDs. Adding a quantity of N creates N bottle records. The interface
groups matching bottles and displays quantities without replacing individual
records with a single quantity counter. Individual bottles can retain distinct
locations, purchase details, and consumption history. Grouping and selection
must preserve those distinctions when acting on one bottle or a subset.

For the initial wine workflow, opening a bottle marks it consumed. Consumption
date and a tasting note are optional. Retain the bottle and its history after
consumption; this is not deletion of the record. Do not track remaining liquid
volume or require a separate partially consumed state for wine. Revisit
remaining-volume tracking when adding spirits, where partial bottles are common;
defer its design until that product scope is addressed.

When consuming one of several matching bottles, designate it by its physical
location/slot, not by purchase price or other differing metadata. Resolve that
selection to the individual bottle ID. Where the target is unambiguous, retain
a simple consumption action. On consumption, release the bottle's active slot
occupancy so that slot becomes available again, while preserving its previous
location in history. Persist consumption and slot release atomically locally
and on the server; retries must not consume a second matching bottle or release
a slot now occupied by a different bottle.

When different bottles claim the same slot through concurrent/offline placement,
the later placement edit wins, with its revision causal ID breaking equal-time
ties. Arrival time does not decide occupancy. Keep the losing bottle in inventory
under an **Unplaced** category, with its identity, details, and history intact;
never delete a bottle as a side effect of resolving a slot conflict. An unrelated
edit to a bottle's notes or purchase details must not become a new placement
claim. Implement occupancy reconciliation so all devices converge on at most
one active bottle per slot without rewriting immutable revision history.

The Unplaced category also supports bottles not assigned to a physical storage
location yet. These bottles still belong to their logical cellar, retain its
permissions, and count toward available inventory. Users with write access can
assign another location/slot. An Owner may recoverably delete a bottle confirmed
to be an accidental duplicate, following existing deletion permissions; conflict
resolution must not infer duplication merely from matching wine details.

- [ ] Test two devices claiming one slot, reversed sync order, equal placement
  times, retries, subsequent moves/consumption, and unrelated bottle edits; verify
  convergence, preserved bottle inventory, and visible Unplaced results.

Purchase price, purchase date, and seller are optional. Adding existing bottles
must not require reconstructing receipts or inventing purchase values. Preserve
the distinction between an absent price and a recorded zero price. Purchase
details remain attached to the individual bottles even when the UI groups them.

Enter purchase price per bottle. When adding multiple bottles in one submission,
apply that entered unit price equally to every bottle; it is not a batch total
to divide. Bundle/total-price entry and allocation are deferred to a later feature.
Show **Last price paid** in the detailed view
for the particular wine, derived from its purchase records within the authorized
cellar. Use purchase chronology rather than revision edit time or sync arrival
to determine recency. Do not label a value definitively "last" when missing
purchase dates make the ordering uncertain; the precise fallback presentation
remains UI work.

First-release purchase pricing supports dollars only (USD). Do not expose a
currency selector or implement exchange-rate conversion for the initial release.
Preserve USD as the denomination of recorded amounts.

If multiple currencies are added later, default currency belongs to the user,
based on their location/locale, rather than the cellar. That preference and any
per-purchase override are deferred. Changing a future default must not relabel
or convert existing USD amounts.

## Agreed bottle-entry atomicity

One bottle-entry submission, including all N bottles and any newly created
catalog records such as a producer or wine, is one atomic operation. Either all
of it saves or none of it does. Apply this independently to the local transaction
and the server transaction; offline local success is not a claim of server sync.
Preserve operation grouping during synchronization so transport batching cannot
leave a partially accepted entry.

Allocate stable operation, record, and revision identifiers before retryable
submission and reuse them on retries. A lost response or repeated submission of
the same operation must not create duplicate bottles or catalog records. Report
local-save and sync outcomes distinctly, preserving pending work when server
acceptance fails. Authorization and validation apply to the complete operation.

- [ ] Test failure midway through local and server saves, offline submission,
  lost acknowledgements, and retries; verify all-or-nothing persistence and no
  duplicate effects for the same operation.

## Agreed conflict-resolution policy

Last edit wins at the record level, based on when the edit was made, not when it
was uploaded, received, or committed during synchronization. Capture the edit
version at mutation time and preserve it through offline storage and retries.
Do not replace it with sync time. Field-level merging and user conflict-selection
dialogs are not the selected approach.

Model each editable domain record as a stable logical record with immutable,
full-record revisions. Every creation or edit produces a new revision with its
own globally unique causal ID, the logical record ID, edit timestamp, actor, and
complete record contents. Relationships reference the stable logical ID so
editing a record does not break its references. Preserve revisions for history,
including authorized, valid edits that lose conflict resolution.

The active revision is the greatest eligible revision by the ordered pair
`(edit timestamp, revision causal ID)`, using a consistent canonical causal-ID
ordering on every platform. Thus causal ID deterministically breaks equal-time
ties; arrival order and the logical record's ID do not. A new revision ID is
allocated at edit time and reused on retry, never regenerated at sync time.
Distinct source identities and durable clocks are still required for uniqueness.

Persist a full revision atomically and maintain any stored active-revision
reference transactionally. A delayed losing revision may add history but must
not displace the winner. An identical revision retry is idempotent; the same
revision ID with different contents is an error, not an update to history.
Future-dated revisions remain local until eligible, per the guard below.
History access follows the same cellar boundaries and must not bypass record
permissions. Revision storage and sync must remain bounded/pageable; history
retention is deferred with the broader retention policy.

Offline availability includes the latest successfully synchronized current
records and local edits. A disconnected client cannot promise changes made
elsewhere since its last successful sync. Keep locally created revisions;
do not download the entire historical revision set upfront. Fetch older history
on demand when the user opens it online, with clear unavailable-offline behavior
for history not held locally. This does not weaken current-record offline access
or permit dropping pending revisions.

Reverting a record creates a new full revision containing the chosen historical
contents, with a new edit timestamp and causal ID on the same logical record.
Record the reverting actor and source revision as provenance; do not overwrite
history or move the active reference directly back to an old revision. Reversions
use normal validation, authorization, atomic persistence, and last-edit-wins
sync rules. A reversion cannot change immutable authorship or bypass deletion,
restoration, or membership permissions by copying historical contents.

Example: device A edits offline at hour 0 and syncs at hour 24. Device B edits
the same record at hour 12 and syncs immediately. B's edit remains the winner
when A finally syncs. Reconcile A to that winning version without mistaking an
acknowledgement of A's old mutation for acknowledgement of a newer local edit.

Authorization and validation apply before conflict resolution; a newer edit
cannot override permissions or lifecycle invariants. Edit-clock implementation
remains to be specified. Causal ordering and physical
edit-time ordering must not be treated as interchangeable without an explicit
clock design, particularly for edits made on disconnected devices.

Only edits timestamped at or before the authoritative server's current time may
be accepted into synchronized state. Enforce this on the server, not solely
against a potentially drifting phone clock. Future-dated edits remain pending
locally with a visible clock/sync warning; do not discard them, acknowledge them
as synchronized, silently retimestamp them, or let them participate in conflict
resolution. Recheck eligibility on later attempts. Sync other eligible edits
where operation dependencies and transactional boundaries permit.

This guard prevents accepting future timestamps but cannot prove the actual
physical order of disconnected edits with inaccurate clocks. A future-dated
edit eventually passes the guard once server time reaches its timestamp, and
slow-clock edits can already appear to be in the past. Handling these residual
clock inaccuracies remains part of the edit-clock design; the guard alone must
not be described as eliminating clock skew.

- [ ] Verify the delayed-offline-edit example, reversed arrival order, retries,
  equal edit timestamps, clock skew, and version-specific acknowledgements.
- [ ] Verify immutable full-revision writes, stable logical references,
  deterministic causal-ID tie-breaking, retention of valid losing revisions, and
  atomic active-revision selection. Reject reuse of a revision ID for different
  contents and ensure retry acknowledgements cannot clear a newer local revision.
- [ ] Verify reversion creates a new revision, leaves prior revisions unchanged,
  records provenance, and converges through normal sync without bypassing current
  permissions or lifecycle rules.
- [ ] Reject future-dated edits at the server boundary, preserve them pending
  locally, and show the clock warning. Test ongoing clock drift, later eligibility,
  and continued progress for independent eligible edits.

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
  a transient outage or expired session must not trigger cache deletion.
- [ ] Remove the revoked cellar's local data and sync state when revocation is
  confirmed, without affecting other accounts or cellars.
- [ ] Define user-facing handling of pending writes rejected after revocation;
  do not silently report them as synchronized or bypass the access decision.
- [ ] Test offline use followed by revocation, reconnection, and account switching.

Membership revocation is distinct from a server reset or backup restore. The
accepted revocation policy does not authorize automatic destruction of local
work during ordinary recovery.

## Agreed server-restore recovery policy

After a server backup restore, preserve locally held revisions and reconcile
them with the restored server instead of wiping the device and replacing its
state with the older backup. Include both pending edits and locally retained
revisions previously acknowledged by the server that the backup may now lack.
Recovery can replay only revisions actually held locally; on-demand history
loading does not make every device a complete historical backup.
Validate the authenticated account, current permissions, record constraints,
and future-edit guard before accepting replay. Reuse original revision IDs and
edit timestamps, with the agreed last-edit-wins comparison and idempotent writes.

Compare revision causal-ID sets within authorized scopes to identify missing
revisions on either side and reconcile those differences. Do not infer complete
history from a maximum causal ID alone; missing revisions can have lower IDs.
After reconciliation, select the active revision by edit timestamp and then
revision causal ID, preserving the distinction between identity and edit time.

A restore invalidates assumptions about old sync cursors and acknowledgements;
detect the recovery boundary explicitly and reconcile safely across it. Missing
records in an older backup are not by themselves proof of deletion or membership
revocation. Preserve work while authorization is uncertain, without granting
access or replaying writes. Do not replay client-held account or membership state
as authorization. The restore procedure must establish trusted access state;
an older backup may itself predate a revocation.

Keep reconciliation resumable and transactional, and show its progress/failures.
Confirmed revocation continues to follow the separate policy above. The detailed
protocol and treatment of uncertain authorization require implementation design.

- [ ] Exercise backup restore with pending and previously acknowledged local
  revisions, stale cursors, repeated/interrupted reconciliation, concurrent
  device recovery, and changed or uncertain permissions; verify no silent loss
  of local work or unauthorized resurrection of access.

## Review fix register

All items are open. Priorities and evidence are detailed in the full review.

| ID | Required correction | Acceptance evidence |
| --- | --- | --- |
| R1 | Parameterize SQLite values; validate incoming data; constrain generic table/column mappings. | A crafted synced row cannot execute SQL, alter another record, clear its dirty flag, or poison future pulls. |
| R2 | Stop advancing pull cursors from push acknowledgements; return accepted/conflict outcomes and reconcile winners. | A device receives unseen changes from other devices after pushing its own changes. |
| R3 | Establish commit-safe change ordering and consistent pull boundaries for every writer. | Reordered commits and writes during a pull never cause permanently skipped rows. A snapshot alone is insufficient. |
| R4 | Persist distinct device/source identities and durable clocks, including server-writer rules. | Identical starting snapshots, offline creation, restarts, and restores never reuse object IDs. |
| R5 | Keep stable logical IDs and immutable full-record revisions with unique causal IDs; select the active revision by edit time then revision causal ID, with version-specific acknowledgements. | Equal timestamps converge regardless of arrival order; history retains valid losing edits; stale acknowledgements cannot clear newer local revisions; clock skew has a defined outcome. |
| R6 | Implement authentication, membership authorization, suitable origin/CSRF protection, and safe development bindings; retire the demo server write route. | Anonymous, unauthorized, cross-cellar, and disallowed cross-origin operations cannot access or mutate cellar data. |
| R7 | Introduce shared validated values and explicit API errors for IDs, status, revision, timestamps, references, and size limits. | Invalid values cannot persist or corrupt clock recovery; invalid client requests receive actionable errors. |
| R8 | Define safe reset/restore recovery; validate epochs on data operations; preserve pending work according to that policy. | Server replacement, failed metadata checks, interrupted rehydration, and backup restoration do not silently lose offline work. |
| R9 | Define transactional operation boundaries for server writes, local applies and cursors, reset state, and catalog chains. | Failure midway leaves a coherent result; retries are safe and outcomes explicit. |
| R10 | Add worker starting/ready/failed states, bounded waits, pending-request rejection, correct VFS retry, and coordinated multi-tab database/sync access. | Multiple tabs work normally without corruption or duplicate effects; initialization failure, worker termination, and coordinator loss recover instead of hanging. |
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
later. The immediate target is local operation; do not expose the application
publicly while early phases are incomplete. Backup destination selection,
automated backups, restore drills, and public-access deployment work are future
concerns to complete before any public release, not blockers for starting locally.
This deferral does not remove the agreed data-preserving recovery design.

The agreed first usable milestone is one complete workflow: **sign in -> create
cellar and storage -> add bottles -> find a bottle -> consume it and free its
slot**, including offline operation and synchronization. Validate it on desktop
and Android on the home network. The phases below supply this milestone's
foundations; do not treat isolated backend completion as a usable application.
Build immutable revision storage and the agreed conflict rules from the start,
but defer history-browsing UI and rating-trend visualizations until after this
workflow is usable. Their deferral does not permit discarding revision history.
Establish the new desktop/mobile design before implementing the milestone's
production UI; design work can proceed alongside the foundational fixes.

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
   [entry-form design](docs/plans/2026-07-03-entry-form-design.md), revisiting its
   presentation within the new Barback design rather than polishing the test
   UI. Complete cellar/storage setup, inventory search,
   and location-based consumption with atomic slot release to deliver the agreed
   first usable workflow. Retire demo scaffolding as real entry replaces it.
5. **Limited-user internet launch after local testing.** Choose and configure backup storage,
   automate backups and exercise restores; verify HTTPS, hardened deployment,
   dependency checks, and supported browser/device behavior before enabling
   public access through Cloudflare Tunnel. Public registration remains separate
   from public reachability and is still outside scope.

After the first usable milestone, add history browsing/reversion UI and dated
rating-trend visualizations. Their order relative to limited-user deployment can
be decided after local testing; neither replaces the internet launch checks.

## Decisions still to settle

- Safe per-cellar rebuild implementation and browser site-data/removal behavior.
- With Keycloak/OIDC, primary passkey sign-in, and invitation-only registration
  selected: invitation-gated enrollment, operational Keycloak bootstrap setup,
  shared activation/recovery implementation with the agreed invalidation policy,
  and enforcement of the agreed session limits and sensitive-action authentication.
  Public registration is outside current scope.
- Account deactivation/reactivation implementation and Keycloak coordination.
  Retention policy is deferred; retain deleted cellar records without automatic
  purging initially and preserve accounts independently of cellar retention.
- Schema mapping of server-maintained reference vocabulary versus cellar-owned
  user catalog data; a cross-cellar shared catalog is deferred.
- Device source allocation, source exhaustion, edit-clock design under the agreed
  server-time future-edit guard, residual clock-skew handling, and
  revision-schema and sync details for the agreed last-edit-wins conflict policy.
- Recovery protocol and trusted authorization state after server reset/restore,
  implementing the agreed preservation and reconciliation policy.
- Exact test matrix for Firefox/Chromium and Android, multi-tab coordination, browser
  storage durability, and eviction handling.
- Optional-vintage catalog uniqueness, soft-delete revival, country-scoped relationship validation,
  grape/clone consistency, percentage/position bounds, and hierarchy-cycle rules.
- Implementation of atomic multi-bottle/catalog entry and idempotent sync.
- New Barback visual and interaction design across desktop and mobile; the
  existing test UI is not a design baseline.

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

## Design review implementation

The first Barback layout study is available through `just preview`, using the
existing Rust/Axum/Askama, Datastar, TypeScript, and Vite stack. See
[the review guide](docs/design-preview.md) for flows, review order, and limitations.
This is an isolated, database-free sample preview for desktop/mobile feedback;
it does not complete the security, persistence, authentication, or sync work above.

Review round 02: use simple circular text marks in place of bottle illustrations
until a dedicated bottle design exists. Omit unknown vintage values entirely
from collection cards and wine details. Bottle creation includes cellar/rack and
open-slot selection in the same form; choose at most one slot per bottle and
leave any bottles without a selected slot explicitly Unplaced for later placement.
The revised preview demonstrates these choices, including quantity changes and
clearing slot selections when changing cellar or rack.

Brand identity: use the supplied bottle-cutout B plus `arback.` wordmark for
branded headers, and the standalone rounded-square symbol for favicons/app icons.
Use ordinary lowercase `barback.` in prose, contact information, and page titles.
Provide charcoal-on-light and warm-white-on-dark assets with a gold wordmark dot.
See [the brand asset notes](docs/brand/README.md) for sources and regeneration.

Rack interaction: an occupied slot opens a quick wine summary while remaining
on Storage. Use a side panel on desktop and a compact bottom sheet on mobile,
with selected-slot context, Open and Move actions, and an explicit link to full
wine details. Dismissing the summary restores focus to the selected slot.

Placement follows the same desktop side-panel / mobile bottom-sheet pattern.
Find a place starts with a known wine and asks for rack and open slot. Selecting
an empty rack slot fixes that destination and asks which unplaced bottle to put
there, with wine details shown in the selection list. Both keep Storage in view.

Appearance uses a light/dark toggle and follows the live system color scheme
until explicitly changed. Persist only the user override; Use system clears it.
The desktop and mobile controls share this preference, including across tabs.

Placement review refinement: mobile placement sheets may use up to 80% of the
viewport height. Desktop unplaced-bottle lists paginate after three entries,
with previous/next controls and a page indicator. Selection survives paging.
Appearance controls show only the left-aligned light/dark toggle and a
right-aligned Use system action when an override is active.
