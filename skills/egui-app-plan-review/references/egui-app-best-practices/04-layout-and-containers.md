# Layout and Containers

## Think in constraints and cursor advancement

A `Ui` combines a maximum rectangle, a minimum/used rectangle, a layout cursor, style, clip rectangle, painter, ID scope, and enabled/visible state. Adding a widget asks for a desired size, allocates a rectangle according to the layout, advances the cursor, registers interaction, and usually paints.

Important distinctions:

- `available_size()` is the remaining capacity at the current cursor, not the window size. It changes as widgets are added.
- `max_rect()` is the region the UI may use; `min_rect()` grows to cover what it did use.
- `available_rect_before_wrap()` describes remaining capacity before the next wrap decision.
- `set_min_*` can grow a UI; `set_max_*` cannot retroactively shrink content already placed.
- `set_width`/`set_height` set both minimum and maximum. Apply constraints before adding content.
- `add_sized` supplies a desired outer size, but a widget may overflow when its intrinsic minimum cannot fit.

Use logical points and style metrics. An interactive control's minimum height should normally be based on `ui.spacing().interact_size`, not an arbitrary physical-pixel constant.

## Top-level composition has a strict order

`Panel`s remove space from their parent in call order. The source declares these rules:

1. The first panel is outermost.
2. Never open one top-level panel from within another top-level panel.
3. Add `CentralPanel` last.
4. Add floating `Window`s/`Area`s after panels.

```rust
egui::Panel::top("menu").show(root, menu_ui);
egui::Panel::bottom("status").show(root, status_ui);
egui::Panel::left("sources").resizable(true).show(root, source_ui);
egui::Panel::right("inspector").resizable(true).show(root, inspector_ui);
egui::CentralPanel::default().show(root, workspace_ui);
show_floating_windows(root.ctx());
```

Panel and area IDs are structural. Keep them stable and unique. Use a resizable panel's full space with a `ScrollArea`, justified content, a separator/text edit, `take_available_space`, or an explicit last allocation; otherwise auto-shrink behavior can make the panel snap back.

## Pick the highest-level container that owns the behavior

| Need | Prefer |
| --- | --- |
| Persistent outer chrome | `Panel` / `CentralPanel` |
| Unknown-size background, margin, border | `Frame` |
| Scroll/clipping and retained offset | `ScrollArea` |
| Floating in-app tool | `Window` |
| Blocking in-app decision | `Modal` |
| Anchored transient choice | popup/menu APIs |
| Frameless floating/overlay primitive | `Area` |
| Native OS window | viewport APIs |
| Simple aligned cells | `Grid` |
| Data table/header/column sizing | `egui_extras::TableBuilder` |
| Sequential heterogeneous strip | `egui_extras::StripBuilder` |

These containers reconcile identity, allocation, clipping, accessibility, interaction layers, and retained state. Hand-rolling them is appropriate only when the product behavior is materially different.

## Nested layout APIs and their ownership

- `horizontal`, `vertical`, and `horizontal_wrapped` are normal sequential composition.
- `with_layout` creates a nested layout that consumes all available space. Use `allocate_ui_with_layout` when the nested region should occupy a bounded size.
- `centered_and_justified` is intended for exactly one inner widget.
- `scope`/`scope_builder` are the normal way to apply temporary layout, style, enabled/visible, max-rect, layer, or ID-salt changes.
- `Frame::show` measures child content, then paints the correctly sized background behind it.
- `new_child` is low-level: it does not allocate or advance the parent.

That last point is a frequent source of bugs. If manually creating a child:

```rust
let rect = /* allocated or otherwise owned by the parent */;
let mut child = ui.new_child(
    egui::UiBuilder::new()
        .id_salt("inspector-body")
        .max_rect(rect)
        .layout(egui::Layout::top_down(egui::Align::Min)),
);
body(&mut child);
ui.advance_cursor_after_rect(rect.union(child.min_rect()));
```

Depending on the design, allocate the rect first with `allocate_ui`, `allocate_rect`, or `allocate_exact_size` instead. The invariant is that the parent must know what space the child used. Also bound and, when needed, shrink the child's clip rect. Do not use `child.id()` as an interaction ID without checking uniqueness; sibling child UIs can share stable IDs unless salted.

## Manual layout and painting

For a custom canvas or spatial editor:

1. Allocate the viewport rectangle once.
2. Treat its `Response` as the primary interaction surface.
3. Define a model-to-screen transform; keep domain geometry out of window pixel coordinates.
4. Derive sub-control IDs from the canvas ID plus stable object/handle IDs.
5. clip painting to the canvas rectangle;
6. register each independent interactive handle with the intended `Sense`;
7. paint in back-to-front order consistent with interaction layering.

Painting into `available_rect_before_wrap()` without allocation/cursor advancement makes later layout overlap and gives scroll containers the wrong content extent. Conversely, allocating the same logical area multiple times can create overlapping responses and confusing focus.

## Text and responsive layout

Text is a layout input, not decoration added after sizing. Explicitly choose overflow behavior:

- wrap body/prose and variable-length descriptions;
- use `horizontal_wrapped` for toolbars, chips, or assembled inline metadata;
- truncate single-line labels in constrained sidebars, and expose the full value with a tooltip/details view where necessary;
- scroll/edit multiline documents in a bounded region;
- avoid unbounded no-wrap layout for user/file data;
- do not scale font size directly with viewport width—use stable typography plus responsive region structure and global zoom.

Test long strings, empty strings, non-Latin text, high zoom, and narrow windows. Layout that only works for the developer's filenames and DPI is not robust.

## Scrolling and virtualization

`ScrollArea::show` executes all child UI. It clips painting but does not make a ten-thousand-item loop cheap.

For uniform rows:

```rust
egui::ScrollArea::vertical().show_rows(ui, row_height, items.len(), |ui, range| {
    for index in range {
        let item = &mut items[index];
        ui.push_id(item.id, |ui| row_ui(ui, item));
    }
});
```

`show_rows` computes the visible range and skips auto IDs consistently. For variable-height lists, galleries, timelines, or canvases, use `show_viewport`, a measured index, or spatial culling. Preserve total content extent while building only visible content.

Virtualization introduces identity constraints: use stable item keys, do not let visible-range indices become durable IDs, and keep selection/domain state outside instantiated rows.

Avoid nested same-direction scroll areas unless the interaction is intentional. Give sibling scroll areas distinct salts. Consider `stick_to_bottom` only for log/chat workflows and do not fight a user who has scrolled away from the end.

## Windows, areas, popups, and modals

- An egui `Window` is not an OS window. Use a viewport for the latter.
- Window titles are default IDs; dynamic or duplicate titles need `.id(stable_id)`.
- `default_size` is initial only. `fixed_size` prevents resizing. Non-resizable windows may still auto-size to content.
- Prefer the current popup/menu APIs over drawing a second `Area` and manually implementing outside-click/focus/escape behavior.
- A modal should have one stable ID, a clear default/cancel path, and app-owned draft state. Do not create a new random modal ID each pass.
- Later-painted overlapping content wins input priority. Place overlays after underlying content and keep their layer/clip rect deliberate.

## Multi-pass sizing

Immediate layout cannot always know future content dimensions. Before creating an app-specific measurement cache or running the same full subtree invisibly:

- see whether `Frame`, `Grid`, a justified layout, `allocate_ui`, text galley measurement, or the built-in sizing-pass support solves it;
- key cached measurements by all inputs that affect them (available width, style/font/zoom, content revision);
- ensure sizing code has no externally visible side effects;
- use `ui.is_sizing_pass()` and `ctx.will_discard()` to skip expensive painting/resources;
- invalidate caches explicitly when fonts/style/data change;
- request a discard only on a transition, never every pass.

## Layout review checklist

- Are top-level panels siblings and is central last?
- Does every resizable container consume the available dimension?
- Does every manually painted region also allocate/advance the same region?
- Does every `new_child` have a bounded rect, correct ID scope, clip behavior, and parent reconciliation?
- Are long collections virtualized rather than merely clipped?
- Are text overflow and narrow-window behavior explicit?
- Are sizes expressed in points/style metrics rather than assumed physical pixels?
- Could a same-direction nested scroll area trap wheel/touch input?
- Is multipass measurement keyed and side-effect free?

## Evidence map

- Layout and `Ui`: [`crates/egui/src/layout.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/layout.rs), [`crates/egui/src/ui.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/ui.rs)
- Panel ordering: [`crates/egui/src/containers/panel.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/containers/panel.rs)
- Windows/areas/frames: [`crates/egui/src/containers/`](https://github.com/emilk/egui/tree/b865da194/crates/egui/src/containers/)
- Scrolling and virtualization: [`crates/egui/src/containers/scroll_area.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/containers/scroll_area.rs), [`crates/egui_demo_lib/src/demo/scrolling.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui_demo_lib/src/demo/scrolling.rs)
- Tables: [`crates/egui_extras/src/table.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui_extras/src/table.rs)
