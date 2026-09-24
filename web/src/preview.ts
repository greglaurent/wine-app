// Static design-review interactions. No worker, database, auth, or sync calls.
import "./preview.css";
import { mergePatch } from "./vendor/datastar.js";

const pages = new Set(["inventory", "add", "storage", "insights", "detail"]);
const themeMedia = matchMedia("(prefers-color-scheme: dark)");
const storageKey = "barback-preview-theme";
type ThemePreference = "system" | "light" | "dark";
let themePreference: ThemePreference = "system";
function readThemePreference(value: string | null): ThemePreference {
  return value === "light" || value === "dark" ? value : "system";
}
function setTheme(): void {
  const theme = themePreference === "system" ? (themeMedia.matches ? "dark" : "light") : themePreference;
  document.documentElement.dataset.theme = theme;
  document.querySelectorAll("[data-theme-toggle]").forEach(button => button.setAttribute("aria-checked", String(theme === "dark")));
  document.querySelectorAll<HTMLElement>("[data-theme-reset]").forEach(button => { button.hidden = themePreference === "system"; });
  const favicon = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
  if (favicon) favicon.href = `/brand/symbol-${theme}.svg`;
  document.querySelector<HTMLMetaElement>('meta[name="theme-color"]')!.content = theme === "dark" ? "#221e22" : "#faf7f2";
}
function chooseTheme(preference: ThemePreference): void {
  themePreference = preference;
  try {
    if (preference === "system") localStorage.removeItem(storageKey);
    else localStorage.setItem(storageKey, preference);
  } catch { /* Keep the in-memory choice when storage is unavailable. */ }
  setTheme();
}
try { themePreference = readThemePreference(localStorage.getItem(storageKey)); }
catch { /* Default to the system when storage is unavailable. */ }
setTheme();
themeMedia.addEventListener("change", setTheme);
document.querySelectorAll("[data-theme-toggle]").forEach(button => button.addEventListener("click", () => {
  chooseTheme(document.documentElement.dataset.theme === "dark" ? "light" : "dark");
}));
document.querySelectorAll("[data-theme-reset]").forEach(button => button.addEventListener("click", () => chooseTheme("system")));
window.addEventListener("storage", event => {
  if (event.key === storageKey || event.key === null) {
    themePreference = readThemePreference(event.newValue);
    setTheme();
  }
});

// Datastar owns conditional visibility after initialization.
document.querySelectorAll<HTMLElement>("[data-show][hidden]").forEach((el) => {
  el.style.display = "none";
  el.removeAttribute("hidden");
});

function navigate(): void {
  closeRackSummary(false);
  const hash = location.hash.slice(1) || "inventory";
  const match = /^wine-([0-7])$/.exec(hash);
  const page = match ? "detail" : pages.has(hash) ? hash : "inventory";
  if (match) mergePatch({ selected: Number(match[1]) });
  mergePatch({ page });
  document.querySelectorAll<HTMLElement>("[data-page]").forEach((el) => { el.hidden = el.dataset.page !== page; });
  document.querySelectorAll<HTMLDialogElement>("dialog[open]").forEach((el) => el.close());
  document.title = `barback. | ${page === "detail" ? "Wine details" : page === "add" ? "Add bottles" : page[0]!.toUpperCase() + page.slice(1)}`;
  window.scrollTo(0, 0);
  if (page === "add") refreshEntrySlots();
  if (location.hash) document.querySelector<HTMLElement>("#main")?.focus({ preventScroll: true });
  if (page === "storage" && pendingPlacement) {
    const request = pendingPlacement;
    pendingPlacement = undefined;
    startPlacement(request);
  }
}
window.addEventListener("hashchange", navigate);

function openDialog(id: string): void {
  document.querySelectorAll<HTMLDialogElement>("dialog[open]").forEach((el) => el.close());
  document.querySelector<HTMLDialogElement>(`#${id}`)?.showModal();
}
document.addEventListener("click", (event) => {
  const target = event.target;
  if (!(target instanceof Element)) return;
  const trigger = target.closest<HTMLElement>("[data-dialog]");
  if (trigger?.dataset.dialog) {
    if (trigger.dataset.dialog === "consume") {
      consumingRackSlot = trigger.closest("#rack-summary") ? rackSelection : undefined;
      mergePatch({ rackaction: Boolean(consumingRackSlot), ...(consumingRackSlot ? { slot: consumingRackSlot } : {}) });
    }
    if (trigger.dataset.dialog === "placement") {
      const request = { wine: Number(trigger.dataset.placementWine), source: rackSelection };
      if (location.hash !== "#storage") { pendingPlacement = request; location.hash = "storage"; }
      else startPlacement(request);
      return;
    }
    openDialog(trigger.dataset.dialog);
  }
  const closer = target.closest<HTMLElement>("[data-close]");
  if (closer) {
    closer.closest<HTMLDialogElement>("dialog")?.close();
    if (closer.dataset.navigate) location.hash = closer.dataset.navigate;
  }
});
document.querySelectorAll<HTMLDialogElement>("dialog").forEach((dialog) => {
  dialog.addEventListener("click", (event) => {
    const rect = dialog.getBoundingClientRect();
    if (event.target === dialog && (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom)) dialog.close();
  });
});

const rack = document.querySelector<HTMLDivElement>("#rack")!;
const rackSummary = document.querySelector<HTMLElement>("#rack-summary")!;
const unplacedPanel = document.querySelector<HTMLElement>(".unplaced-panel")!;
const placementPanel = document.querySelector<HTMLElement>("#placement-panel")!;
type PlacementRequest = { wine?: number; slot?: string; source?: string };
let pendingPlacement: PlacementRequest | undefined;
let placementRequest: PlacementRequest | undefined;
let placementSlot: string | undefined;
let placementBottle: number | undefined;
let placementReturnFocus: HTMLElement | null = null;
const placementRack = document.querySelector<HTMLSelectElement>("#placement-rack")!;
const placementConfirm = document.querySelector<HTMLButtonElement>("#confirm-placement")!;
const placementDesktop = matchMedia("(min-width: 701px)");
const placementPageSize = 3;
let placementPage = 0;
function paginatePlacement(): void {
  const bottles = Array.from(document.querySelectorAll<HTMLElement>(".placement-bottle-option"));
  const pages = Math.max(1, Math.ceil(bottles.length / placementPageSize));
  placementPage = Math.min(placementPage, pages - 1);
  bottles.forEach((bottle, index) => {
    bottle.hidden = placementDesktop.matches && Math.floor(index / placementPageSize) !== placementPage;
  });
  document.querySelector<HTMLElement>("#placement-pagination")!.hidden = !placementRequest?.slot || !placementDesktop.matches || pages <= 1;
  document.querySelector<HTMLButtonElement>("#placement-previous")!.disabled = placementPage === 0;
  document.querySelector<HTMLButtonElement>("#placement-next")!.disabled = placementPage === pages - 1;
  document.querySelector("#placement-page-status")!.textContent = `Page ${placementPage + 1} of ${pages}`;
}
for (const [id, direction] of [["placement-previous", -1], ["placement-next", 1]] as const) {
  document.querySelector(`#${id}`)!.addEventListener("click", () => {
    placementPage += direction;
    paginatePlacement();
    placementPanel.scrollTop = 0;
    document.querySelector<HTMLElement>("#placement-panel-title")!.focus({ preventScroll: true });
  });
}
placementDesktop.addEventListener("change", () => {
  if (placementDesktop.matches) {
    const bottles = Array.from(document.querySelectorAll(".placement-bottle-option"));
    const selected = bottles.findIndex(bottle => bottle.getAttribute("aria-pressed") === "true");
    if (selected >= 0) placementPage = Math.floor(selected / placementPageSize);
  }
  paginatePlacement();
});
let rackSelection: string | undefined;
let consumingRackSlot: string | undefined;
function closeRackSummary(restoreFocus = true): void {
  const previous = rackSelection;
  rackSelection = undefined;
  rackSummary.hidden = true;
  placementPanel.hidden = true;
  placementRequest = undefined;
  unplacedPanel.hidden = false;
  document.querySelector(".storage-layout")!.classList.remove("has-summary");
  rack.querySelectorAll("[aria-expanded]").forEach(button => button.setAttribute("aria-expanded", "false"));
  if (restoreFocus && previous) rack.querySelector<HTMLButtonElement>(`[data-slot="${previous}"]`)?.focus({ preventScroll: true });
}
document.querySelector("#close-rack-summary")!.addEventListener("click", () => closeRackSummary());
document.addEventListener("keydown", event => {
  if (event.key === "Escape" && (!rackSummary.hidden || !placementPanel.hidden) && !document.querySelector("dialog[open]")) {
    event.preventDefault();
    if (!placementPanel.hidden) closePlacement();
    else closeRackSummary();
  }
});
const rackWines = Array.from(document.querySelectorAll<HTMLAnchorElement>("[data-wine-card]")).map(card => ({
  href: card.hash,
  initials: card.querySelector(".wine-monogram")!.textContent!,
  style: card.querySelector(".wine-kind")!.textContent!,
  producer: card.querySelector("h2")!.textContent!,
  unplaced: card.querySelector(".location")!.textContent!.includes("Unplaced"),
  count: Number(card.querySelector(".card-bottom strong")!.textContent),
}));
function wineSummary(id: number): DocumentFragment {
  const fragment = document.createDocumentFragment();
  const source = document.querySelector(`[data-summary-wine="${id}"]`)!;
  for (const selector of [".rack-wine-heading", ".rack-wine-origin", ".rack-wine-facts"]) {
    fragment.append(source.querySelector(selector)!.cloneNode(true));
  }
  return fragment;
}
function closePlacement(): void {
  const focus = placementReturnFocus;
  const fromSlot = Boolean(rackSelection);
  closeRackSummary();
  if (!fromSlot && focus?.isConnected && focus.checkVisibility()) focus.focus({ preventScroll: true });
}
function updatePlacementConfirmation(): void {
  const ready = placementRequest?.slot ? placementBottle !== undefined : Boolean(placementSlot);
  placementConfirm.disabled = !ready;
  const rackName = placementRack.selectedOptions[0]!.textContent;
  document.querySelector("#placement-selection")!.textContent = ready
    ? `${rackName} · ${placementSlot}${placementBottle !== undefined ? ` · Bottle ${placementBottle + 1}` : ""}`
    : placementRequest?.slot ? "Choose one unplaced bottle for this slot." : "Choose a rack and open slot for this bottle.";
}
function renderPlacementSlots(): void {
  placementSlot = undefined;
  const slots = placementRack.value === "main"
    ? Array.from(rack.querySelectorAll<HTMLButtonElement>("button:not(.occupied)")).map(button => button.dataset.slot!)
    : ["B1", "C2"];
  const group = document.querySelector("#placement-open-slots")!;
  group.replaceChildren();
  document.querySelector<HTMLElement>("#placement-no-slots")!.hidden = slots.length > 0;
  for (const slot of slots) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = slot;
    button.setAttribute("aria-pressed", "false");
    button.addEventListener("click", () => {
      placementSlot = slot;
      group.querySelectorAll("button").forEach(el => el.setAttribute("aria-pressed", String(el === button)));
      updatePlacementConfirmation();
    });
    group.append(button);
  }
  updatePlacementConfirmation();
}
function startPlacement(request: PlacementRequest): void {
  placementReturnFocus = document.activeElement as HTMLElement;
  closeRackSummary(false);
  placementRequest = request;
  placementPage = 0;
  rackSelection = request.slot ?? request.source;
  placementBottle = undefined;
  placementRack.value = "main";
  placementRack.options[0]!.textContent = document.querySelector(".rack-panel h2")!.textContent;
  placementPanel.hidden = false;
  unplacedPanel.hidden = true;
  document.querySelector(".storage-layout")!.classList.add("has-summary");
  const list = document.querySelector<HTMLElement>("#placement-bottle-list")!;
  const known = document.querySelector<HTMLElement>("#placement-known-wine")!;
  list.replaceChildren(); known.replaceChildren();
  list.hidden = !request.slot;
  document.querySelector<HTMLElement>("#placement-location-fields")!.hidden = Boolean(request.slot);
  document.querySelector("#placement-context")!.textContent = request.slot ? `${placementRack.options[0]!.textContent} · ${request.slot}` : request.source ? `Moving from ${request.source}` : "UNPLACED BOTTLE";
  document.querySelector("#placement-panel-title")!.textContent = request.slot ? "Choose a bottle" : "Find a place";
  if (request.slot) {
    placementSlot = request.slot;
    rack.querySelector(`[data-slot="${request.slot}"]`)?.setAttribute("aria-expanded", "true");
    rackWines.forEach((wine, id) => {
      if (!wine.unplaced) return;
      for (let index = 0; index < wine.count; index++) {
        const button = document.createElement("button");
        button.type = "button"; button.className = "placement-bottle-option";
        button.setAttribute("aria-pressed", "false");
        button.append(wineSummary(id));
        const label = document.createElement("span"); label.className = "placement-bottle-number"; label.textContent = `Bottle ${index + 1} of ${wine.count} · Unplaced`;
        button.append(label);
        button.addEventListener("click", () => {
          placementBottle = index;
          list.querySelectorAll("button").forEach(el => el.setAttribute("aria-pressed", String(el === button)));
          updatePlacementConfirmation();
        });
        list.append(button);
      }
    });
    if (!list.children.length) list.textContent = "No unplaced bottles in this cellar.";
    updatePlacementConfirmation();
  } else {
    known.append(wineSummary(request.wine!));
    renderPlacementSlots();
  }
  paginatePlacement();
  document.querySelector<HTMLElement>("#placement-panel-title")!.focus({ preventScroll: true });
  requestAnimationFrame(() => {
    if (placementPanel.hidden || !matchMedia("(max-width: 700px)").matches) return;
    const anchor = rackSelection ? rack.querySelector(`[data-slot="${rackSelection}"]`) : rack.querySelector("button");
    if (!anchor) return;
    const overlap = anchor.getBoundingClientRect().bottom - placementPanel.getBoundingClientRect().top + 12;
    if (overlap > 0) window.scrollBy(0, overlap);
  });
}
placementRack.addEventListener("change", renderPlacementSlots);
document.querySelector("#close-placement")!.addEventListener("click", closePlacement);
placementConfirm.addEventListener("click", () => {
  if (placementConfirm.disabled) return;
  mergePatch({ notice: `Placement preview: ${placementRack.selectedOptions[0]!.textContent}, ${placementSlot}. No inventory was changed.`, undoid: -1 });
  closePlacement();
});
const originalOpenSlots = ["C2", "D3", "F4", "E4", "F3", "B4"];
const releasedSlots: string[] = [];
let lastReleasedSlot: string | undefined;
let rackLayout = { rows: 4, columns: 6, empty: false };
function drawRack(rows = 4, columns = 6, empty = false): void {
  rackLayout = { rows, columns, empty };
  rack.replaceChildren();
  rack.style.gridTemplateColumns = `22px repeat(${columns}, minmax(44px, 1fr))`;
  const cell = (text: string): void => {
    const el = document.createElement("span"); el.className = "rack-axis"; el.textContent = text; rack.append(el);
  };
  cell("");
  for (let col = 0; col < columns; col++) cell(String.fromCharCode(65 + col));
  for (let row = 1; row <= rows; row++) {
    cell(String(row));
    for (let col = 0; col < columns; col++) {
      const name = `${String.fromCharCode(65 + col)}${row}`;
      const available = empty || releasedSlots.includes(name) || originalOpenSlots.includes(name);
      const wine = rackWines[col % rackWines.length]!;
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.slot = name;
      button.className = available ? "" : `occupied wine-marker ${wine.style}`;
      button.setAttribute("aria-label", `${name}: ${available ? "available, place a bottle" : `${wine.producer}, occupied, view wine`}`);
      const icon = document.createElement("span");
      icon.className = available ? "" : `wine-monogram wine-marker ${wine.style}`;
      icon.textContent = available ? "+" : wine.initials;
      icon.setAttribute("aria-hidden", "true");
      button.append(icon);
      {
        button.setAttribute("aria-controls", available ? "placement-panel" : "rack-summary");
        button.setAttribute("aria-expanded", "false");
      }
      button.addEventListener("click", () => {
        if (available) startPlacement({ slot: name });
        else {
          closeRackSummary(false);
          rackSelection = name;
          mergePatch({ selected: Number(wine.href.slice(6)), rackslot: name });
          document.querySelector("#summary-rack-name")!.textContent = document.querySelector(".rack-panel h2")!.textContent;
          rack.querySelectorAll("[aria-expanded]").forEach(el => el.setAttribute("aria-expanded", "false"));
          button.setAttribute("aria-expanded", "true");
          rackSummary.hidden = false;
          unplacedPanel.hidden = true;
          document.querySelector(".storage-layout")!.classList.add("has-summary");
          document.querySelector<HTMLElement>("#rack-summary-title")!.focus({ preventScroll: true });
        }
      });
      rack.append(button);
    }
  }
}
drawRack();
document.querySelector("[data-consume]")!.addEventListener("click", () => {
  const slot = consumingRackSlot ?? document.querySelector<HTMLInputElement>("input[name=consume-slot]:checked")?.value;
  if (!slot) return;
  releasedSlots.push(slot);
  lastReleasedSlot = slot;
  drawRack(rackLayout.rows, rackLayout.columns, rackLayout.empty);
  if (consumingRackSlot) {
    closeRackSummary();
    consumingRackSlot = undefined;
  }
});
document.querySelector("[data-undo]")!.addEventListener("click", () => {
  if (lastReleasedSlot) releasedSlots.splice(releasedSlots.lastIndexOf(lastReleasedSlot), 1);
  lastReleasedSlot = undefined;
  drawRack(rackLayout.rows, rackLayout.columns, rackLayout.empty);
});

const entryForm = document.querySelector<HTMLFormElement>("#add-form")!;
const entryQuantity = entryForm.querySelector<HTMLInputElement>("[name=quantity]")!;
const entryCellar = entryForm.querySelector<HTMLSelectElement>("[name=entry-cellar]")!;
const entryDestination = entryForm.querySelector<HTMLSelectElement>("[name=destination]")!;
const entryGrid = document.querySelector<HTMLDivElement>("#entry-slot-grid")!;
let entrySlots: string[] = [];

function refreshEntrySlots(): void {
  const quantity = Math.max(0, Math.floor(Number(entryQuantity.value) || 0));
  const destination = entryDestination.value;
  const unplaced = destination === "Unplaced";
  const mainRack = destination === "Main rack";
  const home = entryCellar.value === "Home cellar";
  const layout = mainRack && home ? rackLayout : { rows: mainRack ? 3 : 2, columns: mainRack ? 4 : 3, empty: false };
  const slots: { name: string; available: boolean }[] = [];
  if (!unplaced) {
    for (let row = 1; row <= layout.rows; row++) {
      for (let col = 0; col < layout.columns; col++) {
        const name = `${String.fromCharCode(65 + col)}${row}`;
        const available = mainRack && home
          ? layout.empty || releasedSlots.includes(name) || originalOpenSlots.includes(name)
          : (mainRack ? ["B1", "D1", "A3", "C3"] : ["B1", "C2"]).includes(name);
        slots.push({ name, available });
      }
    }
  }
  entrySlots = entrySlots.filter(name => slots.some(slot => slot.name === name && slot.available)).slice(0, quantity);
  const focused = entryGrid.contains(document.activeElement) ? (document.activeElement as HTMLElement).dataset.slot : undefined;
  entryGrid.replaceChildren();
  entryGrid.style.gridTemplateColumns = `repeat(${layout.columns}, minmax(44px, 1fr))`;
  for (const { name, available } of slots) {
    const selected = entrySlots.includes(name);
    const button = document.createElement("button");
    button.type = "button";
    button.dataset.slot = name;
    button.disabled = !available || (!selected && entrySlots.length >= quantity);
    button.setAttribute("aria-pressed", String(selected));
    button.setAttribute("aria-label", `${name}: ${!available ? "filled" : selected ? "selected" : "open"}`);
    const label = document.createElement("span"); label.textContent = name;
    const state = document.createElement("small"); state.textContent = !available ? "Filled" : selected ? "Selected" : "Open";
    button.append(label, state);
    button.addEventListener("click", () => {
      entrySlots = selected ? entrySlots.filter(slot => slot !== name) : [...entrySlots, name];
      mergePatch({ draft: true });
      refreshEntrySlots();
    });
    entryGrid.append(button);
    if (focused === name) button.focus({ preventScroll: true });
  }
  const remaining = quantity - entrySlots.length;
  const remainder = `${remaining} ${remaining === 1 ? "bottle" : "bottles"} Unplaced`;
  const summary = entrySlots.length
    ? `${destination}: ${entrySlots.join(", ")}${remaining ? ` · ${remainder}` : ""}`
    : remainder;
  document.querySelector<HTMLElement>("#entry-placement")!.hidden = unplaced;
  document.querySelector("#entry-slot-count")!.textContent = `${entrySlots.length} of ${quantity} placed`;
  mergePatch({ entryslots: entrySlots, placementsummary: summary });
}
entryQuantity.addEventListener("input", refreshEntrySlots);
entryDestination.addEventListener("change", () => { entrySlots = []; refreshEntrySlots(); });
entryCellar.addEventListener("change", () => {
  entrySlots = [];
  entryDestination.value = "Unplaced";
  mergePatch({ destination: "Unplaced" });
  refreshEntrySlots();
});

entryForm.addEventListener("submit", (event) => {
  event.preventDefault();
  refreshEntrySlots();
  mergePatch({ draft: false, added: true });
  openDialog("saved");
});
document.querySelector<HTMLFormElement>("#tasting-form")!.addEventListener("submit", (event) => {
  event.preventDefault();
  document.querySelector<HTMLDialogElement>("#tasting")!.close();
  mergePatch({ notice: "Tasting preview complete. No note was saved to your collection.", undoid: -1 });
});
document.querySelector<HTMLFormElement>("#cellar-form")!.addEventListener("submit", (event) => {
  event.preventDefault();
  const form = event.currentTarget as HTMLFormElement;
  const name = String(new FormData(form).get("name") || "").trim();
  if (!name) return;
  mergePatch({ cellar: name, notice: "New cellar preview. Set up a rack to try the storage flow.", undoid: -1 });
  location.hash = "storage";
  document.querySelector<HTMLDialogElement>("#new-cellar")!.close();
});
document.querySelector<HTMLFormElement>("#storage-form")!.addEventListener("submit", (event) => {
  event.preventDefault();
  const form = event.currentTarget as HTMLFormElement;
  const name = form.querySelector<HTMLInputElement>("[data-bind\\:setupname]")!.value.trim();
  const rows = Number(form.querySelector<HTMLInputElement>("[data-bind\\:rows]")!.value);
  const columns = Number(form.querySelector<HTMLInputElement>("[data-bind\\:columns]")!.value);
  closeRackSummary(false);
  drawRack(rows, columns, true);
  document.querySelector(".rack-panel h2")!.textContent = name;
  document.querySelector(".rack-panel .section-heading p")!.textContent = `${rows} rows · ${columns} columns`;
  mergePatch({ notice: "Storage layout preview updated. Nothing is saved to a database.", undoid: -1 });
  document.querySelector<HTMLDialogElement>("#setup")!.close();
});

const results = document.querySelector("#wine-results")!;
const emptyResults = document.querySelector<HTMLElement>("#empty-results")!;
new MutationObserver(() => {
  emptyResults.hidden = Array.from(results.children).some((el) => (el as HTMLElement).style.display !== "none");
}).observe(results, { attributes: true, subtree: true, attributeFilter: ["style"] });

document.querySelector("#reset-preview")!.addEventListener("click", () => location.reload());
document.addEventListener("keydown", (event) => {
  if (event.key === "/" && event.target instanceof Element && !event.target.closest("input, textarea, select, [contenteditable]")) {
    event.preventDefault(); location.hash = "inventory";
    requestAnimationFrame(() => document.querySelector<HTMLInputElement>("[data-bind\\:search]")?.focus());
  }
});
// The vendor initializes attributes on DOMContentLoaded. Start afterward.
function ready(): void { navigate(); document.body.classList.add("ready"); }
if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", ready);
else ready();
