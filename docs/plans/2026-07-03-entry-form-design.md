# Entry form -- design (IN PROGRESS)

The first real feature: a quick "log the bottles I just got" flow. See
[PROJECT-HANDOFF.md](PROJECT-HANDOFF.md) for full project context.

**STATUS: brainstorming. The functional design below is SETTLED with the owner.
It is PAUSED on the VISUAL direction** -- the owner rejected a plain wireframe
("bland, not what I want in the end") and wants to design the look/feel before any
code. Resume by nailing the visual direction (see "Open" below), then implement in
the phases at the bottom.

---

## Settled decisions

- **Bottle-first workflow.** The primary action is "record the bottle(s) I just
  acquired," creating/picking the wine inline. The catalog builds up as a side effect.
  (Not catalog-first, not a separate wine editor -- those can come later.)
- **Country-first cascade.** Country is the FIRST, required field and drives every
  country-scoped option below it (this is the whole reason the reference vocab is
  country-scoped). Pick US -> appellation types are AVA/County/State and the
  classification section is hidden (US has none). Pick France -> AOC/IGP + the 1855 /
  Saint-Emilion classifications appear. The form "shapes itself to the country."
- **Country is ALWAYS known** at add time -- required, no "skip/fill later" escape.
- **v1 scope = identity + location + classification.** Fields, in order:
  1. Country (required; the driver)
  2. Appellation type (options filtered to the country)
  3. Appellation (the region; autocomplete existing OR create inline, under the type)
  4. Producer (autocomplete existing OR create inline)
  5. Wine (name; autocomplete, scoped to the producer)
  6. Vintage (year; blank = non-vintage)
  7. Classification (IN v1): system (filtered to country) -> level. Country-filtered;
     absent for countries with no system (e.g. US). Owner wants this in v1.
  8. Quantity (default 1)
  9. Location (free text, e.g. "Cellar rack A")
  10. Row / Column (optional numbers)
  11. Status (dropdown from `BottleStatus`, defaults to In cellar)
- **Layout A -- one screen.** All fields on one screen with autocomplete, one Add
  button. (Considered and rejected for v1: B two-step identify-then-place; C
  search-first. C-style dedicated search is a good LATER enhancement once the cellar
  is large.)
- **Quantity -> N bottle rows.** A bottle is one physical unit, so quantity 6 creates
  6 `bottle` rows sharing the location. Row/Column apply cleanly at quantity 1; for a
  batch, leave per-slot row/column blank (slot individually later) and just record the
  location for all N.
- **Position = free-form fields on the bottle for v1** (`position_rack/row/column/...`),
  NOT the structured `location` entity. The `location` self-nesting hierarchy and its
  management UI are deferred.
- **Write path = client-only; NO `Store` trait needed for writes.** Because the app is
  local-first (patched fetch always routes to the worker), ALL writes happen in the
  client wasm; the server is a pure sync sink that just upserts pushed rows. There is
  nothing to share between server and client for entry logic, so no shared `Store`
  trait is required for writes. (The "one renderer" -- shared Askama templates in
  `core` -- still applies to rendering.)

---

## Open / unresolved -- RESUME HERE

**The visual + interaction direction.** The owner wants something richer than a plain
form and wants to discuss/wireframe it before building. Directions floated (as sparks,
none chosen yet):

- Editorial / label-forward (the wine as a card you curate; type-driven, color accent
  from the wine).
- Spatial / cellar-as-hero (a visual rack grid; place bottles by tapping slots;
  position becomes a gesture, not row/column boxes).
- Fast capture (photo the label -> prefill; or one smart search bar).
- Moody / immersive (dark, tactile, wine-toned, subtle motion).

Questions put to the owner, still to be answered:
1. What FEELING do they want when adding a bottle (fast/invisible? indulgent/rich?
   proud/collector-y?).
2. Any reference app (wine or otherwise) whose look/feel to emulate.
3. Is the rack/position something to SEE and TOUCH (a visual layout) or just a field.

Next step once answered: mock up 1-2 richer visual concepts to react to (use the
visualize/show_widget tooling), converge, THEN implement.

Other deferred concern (noted, not blocking):
- **Multi-device dedup.** Resolve-or-create matches the LOCAL catalog, so two devices
  independently adding "Chateau Margaux" create two producer rows (LWW does not dedupe
  by name). Fine for v1 (single device); real dedup is a later multi-device concern.

---

## Wireframes produced so far (in-chat, not saved as files)

1. Three layout options for the flow: A one-screen (chosen), B two-step, C search-first.
2. The country-driven cascade shown as two states side by side: France (AOC appellation
   + 1855 classification visible) vs United States (AVA, classification section hidden).
   This demonstrated the "form shapes itself to the country" behavior.

These were exploratory/functional wireframes (deliberately plain). The final visual is
the open question above.

---

## Implementation plan (once the visual is agreed)

Build in phases; each is independently verifiable.

**Phase 1 -- generalize user-table sync (push).**
Today only `bottle` pushes to the server. Generalize the dirty-push + server upsert to
cover the user/catalog tables (producer, wine, wine_vintage, lot, appellation,
vintage_classification, bottle). This is the mirror image of the generic reference
PULL already built (see PROJECT-HANDOFF section 7): a generic table-tagged dirty-row
push + a generic server upsert with LWW. Verify with curl / browser (create rows
locally -> push -> server has them).

**Phase 2 -- local catalog schema + resolve-or-create write handler.**
- Grow the client SQLite (`crates/client/src/store.rs` `ensure_schema`) to hold
  producer/wine/wine_vintage/lot/appellation/bottle (+ `wine_descriptor`,
  `vintage_classification`), alongside the synced reference mirror tables. Add these to
  `DATA_TABLES` (reset-epoch) and to the dirty-push set.
- Client write handler (in `crates/client/src/handle.rs` or a new module): on form POST,
  RESOLVE-OR-CREATE the chain -- producer (by name+country), wine (by producer+name),
  wine_vintage (by wine+year), lot (a default lot per vintage), appellation (by
  type+name), the classification link -- then create N bottles with client Lamport ids,
  `dirty=1`. Render the result (updated count / confirmation).

**Phase 3 -- the country-driven form template + datastar cascade.**
- `core` Askama template(s) for the form, rendered on both sides (client via the worker
  `handle` GET route).
- Cascading selects via datastar: changing the country fires a datastar `@get` to the
  worker for a fragment; the worker reads the country-scoped reference data from the
  local store and returns the filtered appellation-type / classification `<select>`
  options, morphed in. Same for producer/wine/appellation autocomplete (fragment from
  the worker querying the local catalog).
- Apply the agreed visual design.

**Then:** retire the demo `ensure_seed` / `demo_lot` and the demo `add_bottle`.
