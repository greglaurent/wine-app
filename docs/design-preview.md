# Barback: design study 01

This is a clickable visual study for feedback, not a production vertical slice.
The decisions in `FIXES.md` remain the implementation roadmap. It uses the current
stack: shared Rust/Askama markup, Axum delivery, Datastar signals, and TypeScript
with Vite. No new UI framework, fonts, icon dependency, or image service is used.

## Run and review

Run `just preview` from the development shell and open `http://127.0.0.1:8091`.
If the flake files are still untracked, use `nix develop path:.` for the shell.
For an Android browser on the same trusted LAN, set `BIND_ADDR=0.0.0.0:8091`
when running the command and use the computer's LAN address. This preview has no
authentication; keep it local while reviewing.

1. **Collection:** hierarchy, text-circle cards, list view, search, filters,
   mobile bottom navigation, and the prominence of Add bottles.
2. **Wine details:** information density, bottle locations, purchase details,
   and the opening/consumption dialog. Consumption changes the sample count,
   marks the chosen example location consumed, and frees its example rack slot.
   Undo reverses this interaction. Each wine supports one sample consumption.
3. **Add bottles:** country-first entry, quantity, per-bottle USD price, optional
   purchase details, inline open-slot selection, live summary, and confirmation.
4. **Storage:** labeled rows/columns, available slots, unplaced bottles,
   placement dialog, and configuring a rack's dimensions.
5. **Cellars:** switching, selecting a cellar, and creating a named sample.
6. **States:** use the top-right status button to preview offline, session
   expired, sign-in, and light/dark appearance. Expiry preserves the collection.
7. **Insights:** a secondary, illustrative dashboard direction.

## Deliberate boundaries

- Fictional sample wines and simple text circles; no catalog service.
- Unknown vintages are omitted from cards and details.
- Bottle entry includes cellar and rack selection plus multiple open slots.
  Selected slots cannot exceed quantity; remaining bottles explicitly stay
  Unplaced. Changing cellar or rack clears previous slot choices.
- Bottle locations demonstrate the layout and are reused across sample wines;
  this is not a reconciled physical inventory dataset.
- Add, placement, tasting, and cellar actions show the proposed flow and
  confirmations. They do not create persistent records. Switching cellars reuses
  the fixture; rack setup changes the example grid only.
- Draft inputs stay in the current page session, not persistent draft storage.
- Passkey sign-in, permissions, sync, history, and revisions are not implemented
  by this study. The separate preview server exposes no production API routes.
- Light/dark themes use the tentatively approved purple, charcoal, gold,
  orange, and warm-white palette. Final contrast and accessibility review still
  belongs in the implementation acceptance criteria; this is not a compliance
  certification.

Review the overall direction, readable density, navigation, and each flow before
wiring in the production domain model. Native dialogs, visible focus styles,
semantic controls, reduced-motion support, and responsive layouts are included.

## Review round 02

Replaced bottle artwork with simple circular producer initials, corrected
"Make room for something good," and removed unset-vintage placeholders.
The add form now handles placement in the same flow, including partial placement.

The dashboard direction follows the visual hierarchy in
[Flighty's Passport screenshot](https://flighty.com/): one cohesive collection
overview, large numbers, compact supporting metrics, and country/style breakdowns.
Flighty is the reference for the dashboard/statistics experience.
No Flighty artwork is embedded in the app.

## Rack quick look

Occupied slots open a compact summary without leaving Storage. Desktop uses the
sidebar beside the rack; mobile uses a nonmodal bottom sheet so another slot can
still be selected. The selected slot is highlighted. Close or Escape dismisses
the summary and returns focus to that slot.

Open this bottle targets that slot, with the existing sample consumption and
Undo behavior. Move bottle opens the location-selection side panel or bottom sheet. View full wine details is the explicit navigation action.
All actions retain the static-preview limitations above.

## Placement entry points

- Find a place starts with a known wine: choose its rack and an open slot.
- An empty rack slot starts with a known location: choose one unplaced bottle
  from the detailed list. The location stays fixed.
- Both use the same side-panel / mobile bottom-sheet presentation as quick look.
- Confirmation demonstrates the placement flow and returns to the rack; it does
  not mutate inventory in this static study.

Appearance uses a light/dark toggle and follows the live system color scheme
until explicitly changed. Persist only the user override; Use system clears it.
The desktop and mobile controls share this preference, including across tabs.

Placement review refinement: mobile placement sheets may use up to 80% of the
viewport height. Desktop unplaced-bottle lists paginate after three entries,
with previous/next controls and a page indicator. Selection survives paging.
Appearance controls show only the left-aligned light/dark toggle and a
right-aligned Use system action when an override is active.
