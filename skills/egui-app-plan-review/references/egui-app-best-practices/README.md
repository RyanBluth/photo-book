# Building Applications with egui

This collection is a code-backed field guide to structuring, implementing, and maintaining substantial egui applications. It is written against this checkout of egui `0.35.0` at commit `b865da194` (2026-07-19). egui evolves quickly, so verify API names when applying the guidance to another release.

The guide distinguishes three kinds of statement:

- **Contract**: behavior documented or enforced by egui's public API.
- **Convention**: a recurring pattern in egui, eframe, demos, and examples.
- **Recommendation**: an architectural conclusion drawn from those contracts and patterns.

The first two are evidence about egui. The third is advice, and may have valid exceptions.

## Reading paths

For a new application, read these in order:

1. [Mental model and frame lifecycle](01-mental-model.md)
2. [Application architecture](02-application-architecture.md)
3. [State, identity, and persistence](03-state-and-identity.md)
4. [Layout and containers](04-layout-and-containers.md)
5. [Widgets and interaction](05-widgets-and-interaction.md)
6. [Painting, styling, text, and images](06-painting-style-and-media.md)
7. [Performance and background work](07-performance-and-background-work.md)
8. [Accessibility, testing, and portability](08-accessibility-testing-and-portability.md)

For an existing application, start with [review checklists and failure modes](10-review-checklists.md), then follow its links to the relevant deep dives.

## The short version

These rules prevent most expensive egui mistakes:

1. Treat UI code as a cheap, repeatable projection of application state. It may run many times per second and more than once for one painted frame.
2. Keep important domain state in application-owned structures. egui memory is for superficial widget state such as focus, scroll offsets, and open/closed state.
3. Use stable domain keys for widgets and containers whose state must survive reordering. Scope repeated content with `push_id` and give repeated stateful containers distinct `id_salt`s.
4. Use panels from outside to inside, add `CentralPanel` last, and add windows after top-level panels.
5. Let layout allocate space before painting or interacting. If using `new_child`, explicitly reconcile the child rectangle with the parent cursor and clipping.
6. Prefer built-in widgets and containers. A custom widget should normally follow: size, allocate, interact/mutate, accessibility metadata, then paint-if-visible.
7. Consume `Response` semantics precisely: `clicked` is an action, `changed` means bound data changed, and `dragged` requires a drag `Sense`.
8. Never do slow I/O, decoding, export, database work, or lock waits on the UI thread. Return results through shared state or channels and call `Context::request_repaint` when they arrive.
9. Repaint only while visual output can change. Use `request_repaint_after` for clocks/timeouts and an explicit wakeup for background completions.
10. Virtualize large homogeneous lists with `ScrollArea::show_rows`; use viewport culling for heterogeneous or spatial content.
11. Configure fonts, styles, image loaders, and restored state in the `eframe::CreationContext` where possible.
12. Make custom controls keyboard- and screen-reader-usable, and test them through their AccessKit role/name before relying on image snapshots.

## Primary evidence in this repository

The highest-value sources are:

- [`crates/egui/src/lib.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/lib.rs): immediate mode, integration, multi-pass, widget interaction, and sizing overview.
- [`crates/eframe/src/epi.rs`](https://github.com/emilk/egui/blob/b865da194/crates/eframe/src/epi.rs): `CreationContext`, `App::logic`, `App::ui`, persistence, and platform lifecycle.
- [`crates/egui/src/ui.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/ui.rs): allocation, child UIs, ID scoping, layouts, clipping, and enabled/visible scopes.
- [`crates/egui/src/id.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/id.rs) and [`crates/egui/src/memory/mod.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/memory/mod.rs): identity and retained superficial state.
- [`crates/egui/src/response.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/response.rs): the exact interaction contract returned by widgets.
- [`crates/egui/src/containers/panel.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/containers/panel.rs), [`scroll_area.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/containers/scroll_area.rs), and [`viewport.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/viewport.rs): high-level composition and scaling behavior.
- [`crates/egui_demo_lib/src/demo/toggle_switch.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui_demo_lib/src/demo/toggle_switch.rs): the canonical custom-widget tutorial.
- [`docs/accessibility.md`](https://github.com/emilk/egui/blob/b865da194/docs/accessibility.md) and [`crates/egui_kittest/README.md`](https://github.com/emilk/egui/blob/b865da194/crates/egui_kittest/README.md): accessibility and UI testing.
- [`examples/`](https://github.com/emilk/egui/blob/b865da194/examples): focused eframe, repaint, viewport, font, image, and renderer integrations.

The demo source is valuable, but it is a feature showcase rather than a single prescriptive architecture. Examples also optimize for isolation and brevity. Copy their API technique, not automatically their production architecture.

## Vocabulary

- **Frame**: one integration/render iteration that may produce visible output.
- **Pass**: one execution of the UI code within a frame. A frame is usually one pass, but `request_discard` can cause another sizing/layout pass.
- **Context**: egui's cloneable, internally synchronized global interface and per-frame state.
- **Ui**: a region, layout cursor, style, clip rectangle, ID scope, and painter used to place widgets.
- **Widget**: usually a temporary builder consumed during a pass; it is not a retained object.
- **Response**: the result of allocation and interaction for a widget or region.
- **Point**: egui's logical coordinate unit. Physical pixels equal points multiplied by `pixels_per_point`.
- **Viewport**: a native OS window when supported, or an embedded egui window fallback.
