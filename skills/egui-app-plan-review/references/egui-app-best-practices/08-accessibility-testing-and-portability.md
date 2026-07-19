# Accessibility, Testing, and Portability

## Accessibility is part of widget correctness

eframe enables AccessKit by default where an adapter is available. Built-in semantic widgets generally register roles/names/values. Custom integrations must enable and handle accessibility explicitly.

For separate labels:

```rust
let label = ui.label("Project name:");
ui.text_edit_singleline(&mut draft).labelled_by(label.id);
```

For a custom widget, attach the closest `WidgetType`/`WidgetInfo`, enabled state, label, selected/toggled/value state, and applicable actions. Provide `alt_text` for meaningful images. A visual-only canvas should expose either semantic child nodes or an accessible alternate inspector/list for selecting and editing objects.

Use `Response::clicked` so keyboard and accessibility activation work. Give interactive controls adequate hit targets and focus behavior. Ensure focus order follows visual/logical order, focus is visibly indicated, Escape can leave transient modes, and controls do not depend on hover alone.

Test high zoom, keyboard-only use, screen-reader names/roles, light/dark themes, disabled state, long/localized text, and color-independent status cues.

## Layered testing strategy

Use the cheapest stable test that proves the behavior:

1. **Pure domain tests**: transforms, snapping, validation, commands, serialization, filtering, state machines.
2. **Semantic UI tests**: find roles/names, click/type/focus, advance the harness, assert app state and accessibility values.
3. **Layout invariants**: explicit harness sizes and assertions about bounds/non-overlap/visibility.
4. **Textual/structural snapshots** where useful.
5. **Image snapshots** only for genuinely visual contracts.
6. **Small platform smoke tests** for native/web/backend/resource integration.

Semantic tests are usually faster and less brittle than screenshots, and they simultaneously validate accessibility metadata.

## egui_kittest conventions

```rust
let mut harness = egui_kittest::Harness::new_ui_state(
    |ui, state| { ui.checkbox(state, "Show grid"); },
    false,
);

harness.get_by_label("Show grid").click();
harness.run();
assert!(*harness.state());
```

After sending input, advance the harness before re-querying. Set an explicit window size for responsive cases. Use deterministic injected time/data; avoid network and real filesystem dependencies in component tests. If a view intentionally requests continuous repaint, use bounded steps rather than waiting for stabilization forever.

Test at least:

- normal activation and disabled behavior;
- keyboard focus/Enter/Space/Escape;
- changed-state semantics;
- stable state after reorder/filter;
- narrow/wide and high-zoom layouts;
- empty/loading/error/large-data states;
- modal open/submit/cancel/outside behavior;
- background completion and stale-result rejection;
- persistence restore with missing/old/corrupt data.

## Image snapshot discipline

Use snapshots for appearance that semantic/layout assertions cannot express: custom painting, icons, clipping, theme regressions, complex composite layout.

- render only the component/state under test;
- use a small fixed size and deterministic fonts/time/data;
- keep expected images checked in;
- exclude generated `.diff.png` and `.new.png` files;
- keep tolerance low and verify a known-bad rendering still fails;
- mask only unavoidable dynamic regions;
- collect multiple related snapshots so failures do not stop later states;
- update explicitly with `UPDATE_SNAPSHOTS`, then visually review.

GPU snapshots vary across backend/driver/hardware because of MSAA, filtering, derivatives, and floating-point behavior. Normalize features first; raising tolerance is the last step, not the first.

## Debugging tools and observability

- Keep debug ID-clash warnings enabled and fix root causes.
- Use `Context::repaint_causes` for hot repaint loops.
- Inspect widget/accessibility trees when queries or focus fail.
- Enable egui inspection/debug overlays and profiling for layout/performance work.
- Log background operation start/generation/completion/cancel/error, but avoid a per-pass log flood.
- Render explicit loading/error/retry UI rather than leaving stale content unexplained.
- Test pathological sizes, including very small/zero integration dimensions where relevant.

## Native and web setup

For an ordinary all-egui app, prefer eframe. `run_native` plus an `AppCreator` supports persistence and renderer resources. `run_ui_native` is a simpler native-only option when advanced lifecycle/persistence is unnecessary. Configure the root OS window through `NativeOptions.viewport` and use a stable application name/ID for storage and desktop integration.

For web, follow the official template/build flow, retain the `WebRunner`, and handle panic/destroy integration. Account for canvas/web limitations rather than assuming desktop behavior:

- native-style multiple viewports are unavailable;
- browser accessibility support differs and may be experimental;
- mobile on-screen keyboard/text editing has platform limitations;
- browser find/font/color-preference behavior is not automatic;
- file access, clipboard, URL opening, persistence quota, CORS, and threading differ;
- blocking work is especially damaging and worker support needs web-compatible design.

Use target-specific `cfg` only at integration/service boundaries. Keep domain and most view code portable.

## Viewports and close behavior

Declare a child viewport each pass it should remain open and use a stable `ViewportId`. Handle `ViewportClass::EmbeddedWindow` fallback. Prefer deferred viewports for independent repaint rates; their callbacks need synchronized/channel state.

Route window operations through `ViewportCommand` rather than backend-specific APIs when possible. For close confirmation:

1. observe `close_requested`;
2. immediately send `CancelClose`;
3. show an app-owned confirmation state;
4. send `Close` only after explicit confirmation.

Screenshot responses and some platform commands arrive on later passes; model them as requests/results, not synchronous calls.

## Custom integration contract

A non-eframe integration must correctly:

1. gather `RawInput` in logical points;
2. call `Context::run_ui` (preferred for multipass);
3. handle `PlatformOutput` and output commands (cursor, clipboard, URLs, IME, etc.);
4. process every viewport output;
5. apply texture sets before painting;
6. tessellate shapes at returned pixels-per-point;
7. paint clipped primitives with correct blend/gamma/scissor behavior;
8. free requested textures after painting;
9. install a repaint callback that wakes the event loop, including calls from workers.

Use egui's input-wants APIs to arbitrate pointer/keyboard ownership when embedded in a game/engine.

## Release portability checklist

- Compile and smoke-test every supported target/backend and feature combination.
- Confirm required Linux Wayland/X11/system packages and desktop app ID behavior.
- Test high DPI and runtime zoom on each OS.
- Test clipboard, IME/non-Latin text, file dialogs, URL opening, drag/drop, close, and persistence paths.
- Handle missing GPU features/device limits and surface/device errors.
- Do not enable both heavy render backends/features without a deployment reason.
- Verify web assets, MIME/CORS, local storage failure, browser focus, and small/mobile view behavior.
- Verify assistive technology on actual supported platforms; a semantic tree test is necessary but not sufficient.

## Evidence map

- Accessibility guide: [`docs/accessibility.md`](https://github.com/emilk/egui/blob/b865da194/docs/accessibility.md)
- Testing: [`crates/egui_kittest/README.md`](https://github.com/emilk/egui/blob/b865da194/crates/egui_kittest/README.md), [`crates/egui_demo_app/tests/test_demo_app.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui_demo_app/tests/test_demo_app.rs)
- Platform lifecycle: [`crates/eframe/src/lib.rs`](https://github.com/emilk/egui/blob/b865da194/crates/eframe/src/lib.rs), [`crates/eframe/README.md`](https://github.com/emilk/egui/blob/b865da194/crates/eframe/README.md)
- Viewports: [`crates/egui/src/viewport.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/viewport.rs), [`examples/multiple_viewports/`](https://github.com/emilk/egui/tree/b865da194/examples/multiple_viewports/), [`examples/confirm_exit/`](https://github.com/emilk/egui/tree/b865da194/examples/confirm_exit/)
- Integration contract: [`crates/egui/src/lib.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/lib.rs), [`crates/egui/src/data/output.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/data/output.rs)
