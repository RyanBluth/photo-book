# Mental Model and Frame Lifecycle

## Immediate mode means “recompute the view,” not “discard all state”

Each pass, application code reads current state, lays out widgets, handles this pass's interactions, and emits shapes. Calling `ui.button` does not create a retained button object; it returns a `Response` and the temporary builder is gone. Bound values remain because the application owns them. egui retains only the UI state it needs, keyed by `Id`.

This suggests a useful model:

```text
domain state + current input + egui's superficial memory
                       │
                       ▼
             repeatable UI projection
                       │
          actions/state changes + shapes + platform output
```

The projection should be inexpensive and safe to repeat. It should not rely on a widget object remaining alive, and it should not put the same domain truth in both a model and a separate UI mirror unless an explicit draft/commit workflow requires it.

The crate-level docs demonstrate that a slider reads and modifies the application-owned `f32`; egui does not own the value. `Memory` likewise says its data map is not for important data. Those are contracts, not merely style preferences.

## Frame, pass, and repaint are different concepts

Most frames run the UI once. Some widgets need information that is only known after layout. A `Grid`, for example, may request that the first result be discarded so another pass can use measured column widths. `Context::run_ui` therefore accepts `FnMut`, and `eframe` can execute UI code more than once before presenting one frame.

Consequences:

- Do not assume “my `ui` method ran” means “the user saw a new frame.”
- Keep rendering code idempotent with respect to non-UI side effects.
- A click should still be handled in the UI projection, but do not append telemetry, enqueue saves, increment frame-independent counters, or launch jobs merely because a layout closure ran unless the operation is guarded by a real transition/action.
- Closures used for explicit sizing may run once to measure and again to render. Honor `ui.is_sizing_pass()` in custom low-level components when expensive work or side effects are avoidable.
- Use `Context::request_discard` only to correct same-frame layout. It adds CPU work and is not a general animation or async mechanism.

`App::logic` and `App::ui` make the distinction clearer in this checkout:

- `logic` is called before `ui`, and may also be called while UI is hidden if a repaint was requested. It must not show UI or paint.
- `ui` builds the root viewport's interface and may be called many times per second.

Use `logic` for cheap application polling/transitions that do not require a `Ui`. Use background workers for expensive work. Use `ui` for projection and immediate interaction.

## Event-driven repainting

egui does not need to redraw continuously while idle. Input, animation, or explicit repaint requests wake the application.

Choose the smallest correct policy:

| Situation | Policy |
| --- | --- |
| Static UI waiting for input | Request nothing; the integration wakes on input. |
| Smooth animation or continuous drag-dependent visualization | `ctx.request_repaint()` while active. |
| Clock, debounce, tooltip deadline, periodic status | `ctx.request_repaint_after(duration)` aligned to the next meaningful change. |
| Background worker completes | Worker/result publisher calls `ctx.request_repaint()`. |
| Child viewport changed | Request the appropriate viewport with `request_repaint_of` when needed. |
| First-pass layout is visibly wrong | Rarely, `ctx.request_discard(...)`, not a timed repaint. |

Calling `request_repaint` every pass creates a permanent hot loop. Conversely, mutating shared state on another thread without requesting repaint can leave the visible UI stale until the next user event. `examples/external_eventloop_async` shows both correct patterns: an async completion requests a repaint, while a blinking indicator schedules only its next half-second boundary.

Use `Context::repaint_causes()` and the inspection/debug tooling when an application unexpectedly redraws continuously.

## Input and interaction timing

egui resolves widget interaction at the beginning of a frame using widget geometry from the previous pass. Later-added overlapping widgets have input priority because they are considered on top. Practical effects:

- Keep a widget's identity and position reasonably stable during an active drag.
- Do not build multiple overlapping click targets accidentally.
- A new overlay is not meaningfully clickable at the instant it first appears; this is normally imperceptible.
- Pick `Sense` deliberately. A click-only child permits a drag to pass through to a scroll area; a drag-sensitive child captures the gesture.
- Use the widget `Response` instead of re-reading raw pointer state for ordinary controls. The response incorporates layers, capture, click thresholds, touch behavior, keyboard activation, and accessibility activation.

Raw input is appropriate for application-wide shortcuts, canvas tools, multi-touch, or a novel interaction that widgets do not model. Even then, check focus and pointer ownership so a shortcut does not steal text-edit input or a canvas does not react through a popup.

## Logical coordinates and deferred painting

egui coordinates are logical points. The backend maps them to physical pixels using `pixels_per_point`; app-level zoom participates in that scale. Store domain geometry in domain units or logical points as appropriate, but do not casually mix texture pixels, physical screen pixels, and egui points.

Painting is deferred. `Painter` calls append shapes; eframe renders after `App::ui` returns. Backend resources referenced by a paint callback must therefore remain valid until the callback executes. Custom integrations must also process texture deltas and platform output in the required order.

For custom rendering, remember:

- clipping is part of the painter/UI state;
- egui colors/rendering expect premultiplied alpha at the renderer boundary;
- a correct high-DPI scale is essential for sharp text;
- shapes are not a substitute for allocation—layout and input still need an owned rectangle.

## What belongs where

| Kind of state | Owner | Examples |
| --- | --- | --- |
| Domain truth | Application model | documents, selected object IDs, edits, jobs, errors |
| Draft UI value with explicit commit | Application/view model | uncommitted filename text, modal form fields |
| Superficial widget state | egui memory | cursor position, focus, scroll offset, open header |
| Derived expensive value | App cache or `Memory::caches` | laid-out domain data, filtered index, generated mesh |
| Long-running operation | Worker/runtime | file scan, decode, export, database query |
| GPU/image resource | App resource manager or egui loader cache | texture handles, decoded images, render pipelines |

The durable principle is single ownership. If a value matters after the UI layout changes or after egui memory is cleared, the application must own it.

## Evidence map

- Immediate mode, passes, widget interaction, auto-sizing: [`crates/egui/src/lib.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/lib.rs)
- Context locking, `run_ui`, repaint APIs: [`crates/egui/src/context.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/context.rs)
- eframe lifecycle: [`crates/eframe/src/epi.rs`](https://github.com/emilk/egui/blob/b865da194/crates/eframe/src/epi.rs)
- Superficial memory and caches: [`crates/egui/src/memory/mod.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/memory/mod.rs)
- Async repaint example: [`examples/external_eventloop_async/src/app.rs`](https://github.com/emilk/egui/blob/b865da194/examples/external_eventloop_async/src/app.rs)
