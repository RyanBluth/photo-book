# Widgets and Interaction

## Reuse semantic widgets first

Built-in widgets and containers carry more behavior than their pixels suggest: hit target sizing, disabled visuals, keyboard focus, screen-reader metadata, touch semantics, text selection, animation, and `Response` conventions. Compose or style them when their behavior matches. Build a custom widget when the interaction or visual primitive is truly different.

Reusable custom components generally take one of these forms:

- a function `fn control(ui, &mut value) -> Response`;
- a builder implementing `Widget` and consumed by `ui.add`;
- a view object with `fn ui(&mut self, ui)` when it owns app-level view state;
- a container returning `InnerResponse<T>` or a small domain response/action.

Do not store `Button`, `Label`, `Response`, `Painter`, or `Ui` across passes. Store the bound value and rebuild the widget.

## Canonical custom-widget sequence

The repository's toggle tutorial defines the core recipe:

1. Determine desired size from content and `ui.spacing()`/style.
2. Allocate exactly once with the narrowest correct `Sense`.
3. Process interaction and mutate the bound value.
4. Call `mark_changed` if and only if the bound content changed.
5. Attach semantic `WidgetInfo` and value/name metadata.
6. Paint only if visible, using visuals derived from the response.
7. Return the primary `Response`.

```rust
fn toggle(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());

    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, "")
    });

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, *value);
        // paint using rect and visuals
    }
    response
}
```

For a keyboard-adjustable value, also register focus, handle keys/accessibility actions, and report value metadata. The source of built-in `Checkbox`, `Slider`, and `TextEdit` is the best reference for fully featured controls.

## Understand `Response` precisely

- `clicked()` is the normal activation API. It includes primary pointer, Enter/Space when focused, and accessibility activation.
- `clicked_by(button)` is intentionally physical-pointer-specific. Do not use it for an ordinary button.
- `secondary_clicked()` includes touch long-press and is the usual context-menu signal.
- `changed()` means underlying bound data changed. It may be true because a value was clamped, even without a pointer action.
- `hovered()` considers interaction capture; during drag-and-drop, use `contains_pointer()` for a potential target.
- `drag_started`, `dragged`, `drag_delta`, and `drag_stopped` require an appropriate drag sense.
- `has_focus`, `gained_focus`, `lost_focus`, `request_focus`, and `surrender_focus` model focus without retained widget handles.
- Response adapters such as tooltips/context menus should be applied to the returned response instead of duplicating its rect and ID.

Use `Response::union` when multiple rectangles form one logical hover/click surface, but preserve a clear primary ID for focus and accessibility. If subregions have distinct actions, keep their responses distinct and optionally return a custom response struct.

## Choose the narrowest `Sense`

| Behavior | Sense |
| --- | --- |
| Pure display | hover or noninteractive allocation as appropriate |
| Button/toggle/link | click |
| Slider/handle/pan surface | drag |
| Select on click and reorder on drag | click-and-drag |

`click_and_drag` has a recognition tradeoff and can intercept scroll gestures. A click-only button inside a scroll area deliberately allows a touch drag to reach the scroll area. Do not mark an entire form/canvas drag-sensitive merely to observe pointer location.

## Raw input and shortcuts

Use response semantics for local widget interaction. Use raw input for:

- global commands/shortcuts;
- custom canvas tools;
- multi-touch/gesture interpretation;
- integration arbitration with a game/engine;
- low-level input diagnostics.

For commands that only one handler should receive, consume the shortcut through `input_mut`. Match the most specific combination first; logical shortcut matching can ignore extra Shift/Alt, so Save As should be tested before Save.

Keep global shortcuts outside menu/window contents because closed container closures may not run. Suppress or reroute shortcuts when a focused text edit owns the same keys. Treat key repeat (`key_pressed` can repeat) differently from held state (`key_down`). If held state changes visible output, schedule repaint.

Never re-enter a context lock from inside `ctx.input`, `memory`, `data`, or similar closures. Extract the needed primitive first. Response methods may also lock their cloned context, so do not call them from inside those closures.

## Compound controls and interaction regions

Decide intentionally:

- Is the whole row clickable or only the checkbox/icon?
- Does a close icon steal the row's selection click?
- Does dragging a child mean scrolling, reordering, or adjusting?
- Which ID owns keyboard focus?
- What name/role does a screen reader announce?

Allocate hit targets at least as large as the visual convention requires (`interact_size`). Paint can be smaller than the hit rectangle. For nested interactive children, prefer a sensed child `Ui`/container so child buttons retain priority while the parent owns its background response.

For drag-and-drop, use stable domain payload IDs, `contains_pointer` on targets, clear accepted/rejected visuals, and commit only on release. Keep the domain mutation after collection iteration.

## Custom containers

A container has more responsibilities than a custom leaf widget:

- begin with a stable scoped ID and layer;
- establish max rect, layout, style, enabled state, and clipping;
- insert a background placeholder if its final size is unknown;
- run child content;
- compute final extent and update the parent cursor;
- reconcile retained state;
- register container accessibility/interaction;
- paint background before content or use the appropriate shape insertion index.

Study `Frame`, `ScrollArea`, `Panel`, and popup implementations before inventing begin/end APIs. Prefer closure-based `show` functions so finalization cannot be forgotten on early return.

## Custom canvas controls

For movable handles or editor objects:

- derive handle IDs from `canvas_response.id.with((object_id, handle_kind))`;
- store model coordinates, derive screen rectangles each pass;
- use the response drag origin/delta consistently rather than accumulating rounded frame deltas when precision matters;
- constrain/snapping logic belongs in a testable domain function;
- mark changed only when model geometry changes;
- request focus on pointer activation if keyboard nudging/deletion is supported;
- expose a semantic role/name/value or an accessible alternative UI for otherwise visual manipulation.

## Common widget mistakes

- Painting before allocation, so hover visuals use the wrong response or layout overlaps.
- Using raw pointer clicks and losing keyboard/accessibility activation.
- Using hard-coded hover/active/disabled colors instead of `Style::interact*`.
- Calling `mark_changed` on hover, scrolling, selection paint, or cursor changes.
- Mutating bound data without calling `mark_changed`.
- Giving every subregion `click_and_drag`, breaking touch scrolling.
- Returning only `bool` when callers need the response for focus/tooltips/context menus.
- Omitting intrinsic/content sizing so parent layout cannot reason about the control.
- Unconditionally changing the cursor outside the response hover/drag state.
- Expensive painting without `is_rect_visible` or `Painter::is_visible`.

## Evidence map

- Widget model and interaction timing: [`crates/egui/src/lib.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/lib.rs)
- Response: [`crates/egui/src/response.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/response.rs)
- Sense: [`crates/egui/src/sense.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/sense.rs)
- Canonical custom widget: [`crates/egui_demo_lib/src/demo/toggle_switch.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui_demo_lib/src/demo/toggle_switch.rs)
- Full built-in controls: [`crates/egui/src/widgets/checkbox.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/widgets/checkbox.rs), [`crates/egui/src/widgets/slider.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/widgets/slider.rs), [`crates/egui/src/widgets/text_edit/`](https://github.com/emilk/egui/tree/b865da194/crates/egui/src/widgets/text_edit/)
- Canvas patterns: [`crates/egui_demo_lib/src/demo/painting.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui_demo_lib/src/demo/painting.rs), [`paint_bezier.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui_demo_lib/src/demo/paint_bezier.rs)
