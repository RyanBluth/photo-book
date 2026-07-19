# Application Architecture

## Start with eframe unless there is a concrete integration need

`egui` is platform-agnostic; `eframe` is its official native/web application framework. For an all-egui desktop or web application, eframe already handles the event loop, input translation, clipboard, cursor, texture deltas, rendering, repaint wakeups, persistence plumbing, and viewport commands. A custom integration is justified when embedding into an existing engine/event loop or controlling rendering at a lower level—not merely to obtain a differently organized application.

At startup, use `NativeOptions`/`WebOptions` for integration settings and the `AppCreator` closure's `CreationContext` for one-time setup:

```rust
eframe::run_native(
    "Acme Editor",
    native_options,
    Box::new(|cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        configure_fonts(&cc.egui_ctx);
        configure_style(&cc.egui_ctx);
        let app = EditorApp::restore_or_default(cc.storage)?;
        Ok(Box::new(app))
    }),
)
```

Prefer this location for installing loaders, fonts, styles, renderer resources, and restoring persistence. A boolean in the first `ui` pass can work, but it complicates multi-pass reasoning, may show a partially initialized view, and spreads lifecycle work into rendering.

## Organize around model, services, view state, and UI functions

A scalable `App` normally has four categories of fields:

```rust
struct EditorApp {
    model: Document,
    view: ViewState,
    services: Services,
    inbox: Receiver<AppEvent>,
}
```

- **Model** contains durable application truth and domain operations.
- **View state** contains app-owned navigation, selection, modal drafts, tool mode, and other state that is not merely an egui implementation detail.
- **Services** own workers, storage, resource caches, or integration handles.
- **Inbox/events** move results from background/external systems into the UI thread.

Split large UIs by coherent region and pass only the state each function needs:

```rust
fn editor_panel(ui: &mut egui::Ui, document: &mut Document, view: &mut EditorView) -> Vec<Action>;
fn inspector_panel(ui: &mut egui::Ui, selection: &Selection, draft: &mut InspectorDraft) -> Action;
```

This is more testable than a global service locator and avoids every widget reaching into a shared lock graph. It also helps Rust borrowing: split disjoint fields before entering closures, return a small action, then apply cross-cutting mutations after the UI borrow ends.

## Use actions when mutation crosses ownership boundaries

Immediate local changes are idiomatic:

```rust
if ui.checkbox(&mut view.show_grid, "Show grid").changed() {
    // local view state is already updated
}
```

For operations that replace parent state, modify several subsystems, close the current view, or require a mutable borrow already held by a UI closure, return a semantic action:

```rust
enum Action {
    None,
    Open(DocumentId),
    Delete(LayerId),
    StartExport(ExportOptions),
}

let action = sidebar(ui, &mut self.view, &self.model);
match action {
    Action::Delete(id) => self.delete_layer(id),
    Action::StartExport(options) => self.services.export.start(options),
    _ => {}
}
```

Do not turn every click into ceremony. Actions are useful at module/ownership boundaries; direct mutation remains clearer inside a small cohesive widget.

## Separate logic from UI without inventing a retained widget tree

This checkout's `App::logic` is an explicit hook for non-painting logic. Suitable work includes:

- draining a bounded number of worker events;
- updating a state machine from completed work;
- checking a cheap timer/deadline;
- applying external menu commands;
- requesting a repaint if those transitions change visible state.

It is not suitable for blocking I/O or showing panels. The UI method should still call panels, windows, and view functions each pass. Avoid creating a parallel retained hierarchy of widget objects; reusable widgets should usually be small value builders or functions that borrow current state.

## Compose top-level regions in a predictable order

Top-level panels consume the remaining rectangle in call order:

```rust
egui::Panel::top("menu").show(ui, menu_bar);
egui::Panel::left("browser").show(ui, |ui| browser(ui, &mut self.browser));
egui::Panel::right("inspector").show(ui, |ui| inspector(ui, &mut self.inspector));
egui::CentralPanel::default().show(ui, |ui| editor(ui, &mut self.editor));

// Windows/overlays after top-level panels.
self.show_windows(ui.ctx());
```

The panel source is explicit: outermost first, `CentralPanel` last, and no top-level panel opened from within another top-level panel. In an eframe `App::ui`, the provided root `Ui` has no frame/margin, so wrap it in `CentralPanel` or an appropriate `Frame` unless the application intentionally paints edge-to-edge.

Use:

- `Panel` for persistent application chrome;
- `CentralPanel` for the remaining primary workspace;
- `Window` for floating in-viewport tools;
- `Modal` for blocking in-viewport decisions;
- popup/menu helpers for transient anchored choices;
- `Area` for low-level floating regions;
- viewports for real OS windows when supported.

## Model asynchronous work as a state machine

A production background operation should expose explicit state:

```rust
enum LoadState<T> {
    Idle,
    Running { generation: u64, cancel: CancelToken },
    Ready(T),
    Failed(String),
}
```

Recommended flow:

1. A user action creates an immutable job request and records `Running`.
2. A worker owns slow work; it does not hold an application/UI mutex for the duration.
3. The worker sends a result tagged with the request/generation and requests repaint.
4. `logic` or the start of `ui` drains results without blocking.
5. The app discards stale generations, updates state, and renders progress/error/success.
6. Shutdown/cancellation behavior is explicit.

This prevents stale search/image/export results overwriting newer input. Bound queues and per-pass drain limits prevent a result flood from monopolizing the UI thread.

Clone `egui::Context` into a worker only as a wakeup/integration handle. Do not pass `&mut Ui`, `Response`, or ephemeral pass data across threads.

## Persistence is schema-bearing application code

With eframe's persistence feature, restore from `CreationContext::storage` and implement `App::save`. Use stable, namespaced keys and serialize application-owned state deliberately. Treat persistence as a schema:

- include a format version for meaningful data;
- use defaults/migrations for added fields;
- do not serialize runtime handles, channels, caches, or secrets;
- decide whether paths are portable or machine-local;
- handle corrupt/missing state without preventing startup;
- do not mistake `persist_egui_memory` for persistence of the document/model.

egui memory persistence is useful for window positions and similar superficial choices. Critical content must be stored through the application's own persistence path.

Avoid disk writes on every pass. eframe calls `save` on shutdown and at `auto_save_interval`; document autosave should be debounced, atomic, error-aware, and moved off the UI thread if serialization/I/O is material.

## Multiple native windows

egui calls OS windows “viewports.” They must be requested every frame they remain visible.

- Deferred viewports repaint independently and are the performance-preferred option, but their callback can run later/multiple times. Communicate through channels or synchronized shared state.
- Immediate viewports capture ordinary `FnOnce` access and are simpler, but parent and child repaint together. CPU cost can approach the number of viewports.
- Web or unsupported integrations may embed them as egui `Window`s. Design for the fallback or feature-gate the workflow.
- Give every viewport a stable ID and route commands/results to the correct viewport.

## Avoid these architecture traps

- A global `Mutex<AppState>` locked throughout the whole UI pass. It obscures ownership, increases deadlock risk, and makes slow lock holders freeze interaction.
- Starting a thread/task every pass because a result is not ready. Record an in-flight state.
- Polling with unconditional repaint instead of waking on completion.
- Doing file dialogs, filesystem scans, decode/export, or network calls synchronously inside widget code.
- Treating indices as durable object identity in editable/reorderable collections.
- Combining drawing, domain mutation, storage, and worker orchestration in one thousand-line UI method.
- Calling UI code from `App::logic` or worker threads without a carefully designed parallel-egui integration.
- Assuming examples' `unwrap`, global logger initialization, or simplified shutdown behavior are production policy.

## Evidence map

- eframe setup, lifecycle, persistence: [`crates/eframe/src/lib.rs`](https://github.com/emilk/egui/blob/b865da194/crates/eframe/src/lib.rs), [`crates/eframe/src/epi.rs`](https://github.com/emilk/egui/blob/b865da194/crates/eframe/src/epi.rs)
- Panel ordering: [`crates/egui/src/containers/panel.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/containers/panel.rs)
- Viewport tradeoffs: [`crates/egui/src/viewport.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/viewport.rs), [`examples/multiple_viewports/src/main.rs`](https://github.com/emilk/egui/blob/b865da194/examples/multiple_viewports/src/main.rs)
- Background completion wakeup: [`examples/external_eventloop_async/src/app.rs`](https://github.com/emilk/egui/blob/b865da194/examples/external_eventloop_async/src/app.rs)
- Custom integration loop: [`crates/egui/src/lib.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/lib.rs), [`examples/external_eventloop/`](https://github.com/emilk/egui/tree/b865da194/examples/external_eventloop/)
