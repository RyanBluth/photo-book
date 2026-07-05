# egui Widget Conventions

These conventions apply when implementing custom widgets or composing low-level egui primitives.

Source references are relative to the egui repository root.

## Reuse First

- Prefer existing local widgets when the interaction model matches.
- A local custom widget is reasonable when the shared widget has materially different density, behavior, or visual semantics.
- Keep custom widgets private to the module until a second use case appears.
- Prefer built-in egui containers and helpers (`Frame`, `ScrollArea`, `Panel`, popup/menu helpers, and egui's compound content layout helpers) when you need margins, clipping, focus, scrolling, accessibility, menus, or persistent state. Manual painting is best for the final visual primitive after egui has already allocated and handled interaction.

## Public API Shape

- Prefer builder-style setters for configurable widgets, matching egui's `Button`, `Slider`, and container APIs.
- Implement `Widget for T` for reusable widgets so callers can use `ui.add(widget)`.
- Keep core implementation in a helper when useful. egui often has a public `Widget::ui` returning `Response`, plus an internal helper that returns richer implementation details.
- Keep custom response types small and domain-specific. Report meaningful actions such as `clicked`, `close_clicked`, or `changed`, not every internal hover/paint detail.

## Sizing And Allocation

- Size custom widgets from content plus `ui.spacing()`/style spacing, not fixed constants alone. Built-in buttons and sliders derive minimum sizes from interaction and style spacing.
- Respect `ui.spacing().interact_size` for interactive controls so hit targets stay consistent with the rest of the UI.
- Allocate exactly once for a simple custom widget. Use `allocate_exact_size`, `allocate_rect`, `allocate_painter`, or a higher-level layout helper depending on the widget shape.
- When a widget has multiple interactive regions, allocate each region deliberately and combine responses with `Response::union(...)` if the regions form one logical widget.
- Set `response.set_intrinsic_size(...)` when a widget lays out text or content whose natural size matters to parent layout, following egui labels.
- For full-panel widgets, the painted region and allocated/advanced region must match. Painting into `available_rect_before_wrap()` without advancing or allocating that rect creates layout bugs.

## Interaction And Response

- Use the returned `Response` as the source of truth for interaction state. For standard visual states, prefer `ui.style().interact(&response)` or `ui.visuals()` over custom hover/active logic.
- Allocate first, mutate state second, paint last. Checkbox and slider both allocate, update state from click/drag/keyboard/accessibility input, mark changed, then paint.
- Call `Response::mark_changed()` only when underlying data changes, not for view-only effects such as hover, cursor changes, scroll position, or repaint.
- Set cursors from hover/drag state only when relevant. Do not set cursors unconditionally during paint.
- For drag controls, handle pointer input, keyboard input, and accessibility actions consistently when the control changes an actual value.
- For reusable custom widgets, add `widget_info(...)` when accessibility or screen-reader behavior matters. egui's checkbox and slider do this after interaction and changed-state handling.

## Painting

- Paint only when visible. Use `ui.is_rect_visible(rect)` before expensive manual painting.
- Allocate and process input before painting. Within the paint phase, paint background/frame before foreground/content.
- Use the widget visuals associated with the current response state. Prefer `ui.style().interact(&response)` for inactive/hovered/active/focused colors and strokes.
- Use clipped painters for content that may overflow. TextEdit paints through a clipped painter for this reason.
- For backgrounds behind unknown-size child content, prefer `Frame`. egui's `Frame` inserts a placeholder shape, lays out children, then replaces the shape once content size is known.
- Use `src/theme.rs` color tokens for app-specific surfaces and repeated custom-widget states. Avoid ad hoc raw color values in custom widget painting.

## Text

- Cap text width and explicitly choose wrap, truncate, or scroll behavior.
- Use egui's text layout controls for overflow:
  - `Label::truncate()` for normal labels that should stay one line.
  - `TextWrapping::truncate_at_width(...)` for hand-laid-out text.
  - wrapping labels or `horizontal_wrapped` when content should naturally span rows.
- Avoid unbounded `layout_no_wrap` in sidebars and panes unless the text is guaranteed short.
- Tooltips for elided text are explicit. If truncated content needs full-value access, add hover text or a details view.
- Single-line controls should generally truncate rather than wrap. Multi-line content should use explicit wrapping.
- Use stable font sizes appropriate to the panel density. Do not scale text with viewport width.

## Compound Widgets

- Prefer existing layout helpers for icon/text/frame/min-size composition instead of hand-placing every subcomponent.
- Keep the primary interaction id stable. When unioning responses, preserve the response that owns focus and keyboard/accessibility interaction as the primary response.
- If a compound widget contains text plus a control, decide whether the full row is clickable or only the control is clickable and make the hit target match that decision.

## Containers And Child UIs

- Containers created with `UiBuilder` should carry an explicit max rect, layout, id salt when persistent, and clipping when content can overflow.
- Container end/finalization should reconcile content size, parent cursor advancement, clipping, state, and painting. `Frame`, `ScrollArea`, and `Panel` all do this explicitly.
- Prefer `ui.scope_builder(UiBuilder::new().max_rect(...), ...)` for normal scoped child regions.
- Use `ui.new_child(...)` only when intentionally doing manual layout. If using `new_child`, make cursor advancement/allocation obvious near the call site.
- Use `Frame`, `ScrollArea`, `Panel`, `Popup`, and menu APIs instead of hand-rolled containers when margins, clipping, state, accessibility, scrolling, focus, or menus are involved.
- For large scroll content, prefer virtualization patterns such as `ScrollArea::show_rows` or viewport-based rendering over rendering every row.

## Useful egui Source References

- Widget sizing/allocation: `crates/egui/src/widgets/button.rs`, `checkbox.rs`, `slider.rs`, `label.rs`.
- Text overflow: `crates/egui/src/widgets/label.rs` and `crates/egui/src/widgets/text_edit/builder.rs`.
- Response semantics: `crates/egui/src/response.rs`.
- Containers and child UIs: `crates/egui/src/containers/frame.rs`, `scroll_area.rs`, `panel.rs`, and `menu.rs`.
