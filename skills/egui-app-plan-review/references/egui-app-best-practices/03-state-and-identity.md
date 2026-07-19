# State, Identity, and Persistence

## Identity is part of widget correctness

egui uses `Id` to connect ephemeral calls across passes. Identity supports active dragging, focus, text cursor state, scroll offsets, popup state, collapsing state, window geometry, animation, and app/widget data stored in `Memory`.

An ID must be:

- unique among simultaneously relevant widgets;
- stable for as long as retained state should follow the logical object;
- scoped so unrelated modules cannot collide;
- derived from semantic identity, not presentation text that may change.

Labels need no interaction ID. A fixed button can often use its auto ID. A reorderable text edit, scroll area, collapsing header, grid, plot, window with a dynamic title, drag target, or custom stateful widget deserves deliberate identity.

## Stable IDs versus auto IDs

`Ui` has both stable hierarchy identity and a unique position-derived identity. Auto IDs are influenced by widget order. They are appropriate for transient interaction when the widget stays in the same place during that interaction. They are not a durable key for state that must follow an item through reorder/insertion/removal.

For a stable collection:

```rust
for item in &mut items {
    ui.push_id(item.id, |ui| {
        ui.text_edit_singleline(&mut item.name);
        egui::CollapsingHeader::new("Details")
            .id_salt("details")
            .show(ui, |ui| item_details(ui, item));
    });
}
```

Use the domain key (`item.id`, canonical path, database primary key, stable enum key), not the current list index. An index is acceptable only when position truly is identity and items cannot reorder.

Repeated containers need distinct salts even if their child widgets differ. Examples include multiple `ScrollArea`s, `Grid`s, `ComboBox`es without unique labels, `Resize`s, and collapsing regions.

## Scope IDs instead of manufacturing global strings

Preferred tools:

- `ui.push_id(domain_key, |ui| ...)` scopes a subtree.
- `.id_salt(local_key)` distinguishes a stateful builder within its current UI.
- `ui.make_persistent_id(local_key)` derives a stable child ID from the current stable UI scope.
- `response.id.with("handle")` derives sub-control IDs from a parent allocation.
- `Id::new((module_or_kind, domain_key))` creates an intentional root ID for a top-level object such as a viewport or window.

Hierarchical derivation documents ownership and reduces accidental collisions:

```rust
let row_id = ui.make_persistent_id(("asset-row", asset.id));
let body = row_id.with("body");
let remove = row_id.with("remove");
```

Avoid concatenated display strings and `format!` when a tuple hashes the components directly. Avoid pointer addresses, iteration order of an unstable map, timestamps, or freshly generated random values as per-pass ID sources.

A random ID is valid only if generated once, stored in application state, and intentionally represents a new logical object. Calling `Id::new(rand::random::<u64>())` while building UI guarantees instability.

## Dynamic labels need explicit IDs

Some builders use visible text as a default identity source. A window's title and a collapsing header's text are common examples. If the title contains a changing count, filename, localized text, or status, set a stable explicit ID/salt:

```rust
egui::Window::new(format!("Results ({count})"))
    .id(egui::Id::new("results-window"))
    .show(ui.ctx(), results_ui);
```

Two windows with the same title likewise need different IDs. Identity and accessible/display naming are separate responsibilities.

## Important state belongs in the app

`Context::data`/`Memory::data` is a typed ID map for superficial widget state. It may be appropriate for a reusable widget whose state is an implementation detail and naturally attached to an ID. It is not a hidden application database.

Before putting data there, ask:

1. Would losing it corrupt user work or domain behavior? Keep it in the app.
2. Does another non-UI subsystem need it? Keep it in the app.
3. Is it large or expensive to clone? Keep it in an app cache or store an `Arc`-wrapped handle.
4. Is it solely focus/cursor/open/scroll/animation state for one widget? egui memory is plausible.

Use temporary versus persisted entries according to desired lifetime. Persistent widget memory means “eligible to serialize with egui memory when enabled,” not “application durability is solved.”

## Draft, commit, and validation state

Many controls directly edit a typed value. That is ideal when every intermediate value is valid. Use an app-owned draft when editing has transactional semantics:

- file/project names that validate on submit;
- multi-field settings with Apply/Cancel;
- destructive actions requiring confirmation;
- numeric/text formats with temporarily invalid intermediate strings;
- search input with debounce and stale-result protection.

```rust
if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
    match validate(&draft) {
        Ok(value) => action = Some(Action::Commit(value)),
        Err(error) => validation_error = Some(error),
    }
}
```

Do not overwrite the draft from the model each pass, or the user cannot type. Initialize it on the transition that opens/selects the editor, then commit or discard on an explicit transition.

## Selection and collection mutation

Store selection by stable domain key, not by borrowed `Response`, screen rectangle, or list index. When the collection changes:

- retain only selected IDs still present;
- define an anchor key for shift/range selection;
- derive displayed indices each pass;
- postpone destructive/reordering mutations until iteration ends;
- ensure drag payloads contain stable IDs;
- use one source of truth for selection visuals and commands.

A common immediate-mode pattern is to collect the requested action while iterating, then mutate afterward. This avoids invalidating iterators and makes same-pass behavior deterministic.

## Focus, keyboard state, and popups

Focus is ID-based. For keyboard-driven custom widgets:

- allocate a focusable/clickable response with a stable ID;
- use `response.request_focus`, `has_focus`, and `lost_focus` rather than a parallel boolean where possible;
- use `ui.input` for the small read needed, and do not call other `Context` methods from inside a context lock closure;
- allow Escape to cancel transient modes and Enter/Space to activate where conventional;
- keep popup/modal IDs stable while visible;
- avoid global shortcuts when a text edit wants keyboard input.

The `Context` is internally locked. Its docs explicitly warn that calling another context-locking accessor from inside `ctx.input`, `memory`, `data`, or related closures can deadlock. Extract values in one closure, release the lock, then act.

## Persistence layers

Think of persistence as layers:

| Layer | Examples | Mechanism |
| --- | --- | --- |
| Domain document | photos, edits, project graph | application file/database |
| User configuration | theme, recent files, export defaults | application config/storage |
| Session/view state | selected pane, drafts if desired | app storage with an explicit schema |
| egui superficial memory | window positions, scroll offsets | `persist_egui_memory` + persistence feature |
| Runtime-only | workers, texture handles, caches | never serialized; reconstruct |

Use stable eframe application IDs and storage keys. Persist only what has a defined restore story. If model state is saved separately, do not let periodic UI-state saves imply that unsaved document changes are safe.

## Identity review checklist

- Can a preceding sibling appear/disappear while this widget is active?
- Can the collection reorder, filter, paginate, or virtualize?
- Does visible text change or localize?
- Are there multiple instances of this `Grid`, plot, scroll area, popup, or collapsing header?
- Does state need to follow the logical item rather than its position?
- Is an ID generated once or accidentally regenerated each pass?
- Could two modules use the same root string? Would hierarchical scoping be clearer?
- Does the custom widget derive sub-IDs from its primary response?

## Evidence map

- ID contract: [`crates/egui/src/id.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/id.rs)
- Stable and unique UI identities, `push_id`, persistent IDs: [`crates/egui/src/ui.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/ui.rs)
- Memory and caches: [`crates/egui/src/memory/mod.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/memory/mod.rs), [`crates/egui/src/util/id_type_map.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/util/id_type_map.rs)
- Stateful container salts: [`crates/egui/src/containers/scroll_area.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/containers/scroll_area.rs), [`crates/egui/src/containers/collapsing_header.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/containers/collapsing_header.rs), [`crates/egui/src/containers/window.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/containers/window.rs)
- Focus and response behavior: [`crates/egui/src/response.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/response.rs)
