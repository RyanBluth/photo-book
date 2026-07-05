# Layout And Theme

## egui Layout And Painting

- When a widget owns a full panel or large region, paint the full intended background explicitly using a stable rect, then advance or allocate the same region so later widgets do not overlap it.
- Prefer `egui::Frame` for ordinary padded/fill containers. Use manual `rect_filled` only when painting a region that is not naturally represented by a container, such as a full panel background or a custom widget body.
- Prefer `ui.scope_builder(UiBuilder::new().max_rect(...), ...)` for scoped child regions. Use `ui.new_child(...)` only for deliberate manual layout, and document where cursor advancement/allocation happens.
- When creating child UIs with `UiBuilder`, always provide a stable `id_salt` for persistent child regions and a bounded `max_rect` derived from the parent layout.
- Avoid relying on viewport-scaled type. Pick fixed, local text sizes that match the density of the panel.
- Prefer `horizontal_wrapped` for compact metadata or chip rows where narrow sidebars are expected.
- Cap text width in narrow sidebars and explicitly choose wrapping or truncation. Use egui truncation APIs such as `Label::truncate()` or `TextWrapping::truncate_at_width(...)` instead of unbounded `layout_no_wrap`.

## Theme Colors

- Use tokens from `src/theme.rs` for app surfaces and repeated UI states instead of ad hoc color values.
- Keep panel contrast intentional:
  - `SURFACE_XX_DARK` for the darkest app/background regions.
  - `SURFACE_X_DARK` for bars or chrome that should separate from the darkest background.
  - `SURFACE`, `SURFACE_MUTED`, and `SURFACE_DARK` for cards, controls, and intermediate surfaces.
- If a new repeated visual state needs a color that does not map well to the existing tokens, add a named token to `theme.rs` instead of scattering raw `Color32::from_gray(...)` values.

