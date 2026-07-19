# Painting, Style, Text, and Media

## Allocate first, paint later

Manual painting does not participate in layout or interaction by itself. Obtain a rectangle and response through allocation, then paint within that rectangle. `Painter` uses absolute logical-point coordinates, a layer, and a clip rectangle, and is only valid for the current pass.

```rust
let (response, painter) = ui.allocate_painter(size, egui::Sense::drag());
if painter.is_visible() {
    painter.rect_filled(response.rect, 0.0, background);
    painter.extend(build_shapes(response.rect));
}
```

Prefer `Painter::extend` for batches. Use a clipped painter for content that may overflow. Shrinking a clip rect is predictable; expanding beyond the parent's clip can violate container expectations.

Painting order is shape insertion order within layers. Background must be inserted before foreground; `Frame` uses a placeholder because its child size is unknown until later. Use explicit layers only for real stacking needs such as overlays/drag previews, and keep paint order consistent with input order.

## Model-to-screen transforms

Spatial applications should have explicit transforms:

```text
domain/document coordinates ↔ canvas coordinates ↔ egui screen points
```

Keep zoom/pan in view state, document geometry in document units, and texture resolution in texels. Convert pointer positions through the inverse transform before editing the model. This makes resizing, DPI changes, export, snapping, and tests independent of the current window.

Clip at viewport boundaries and validate rectangles/floats. Avoid NaN/Inf in layout and GPU paths. If rotating/scaling, distinguish axis-aligned interaction bounds from the painted shape and define the selection policy explicitly.

## Style from semantics

Configure global style once during construction:

- `style_mut_of(Theme::Light/Dark, ...)` for theme-specific palettes;
- `all_styles_mut` for shared typography/spacing;
- `set_global_style` when replacing a full style.

Use local `ui.scope` for temporary overrides. A direct `ui.style_mut()` change applies to later children in that UI and can leak farther than intended.

For reusable custom widgets, derive visuals from `ui.style().interact(...)` or `interact_selectable(...)`. Application-specific surfaces may use named design tokens, but hover, active, selected, disabled, focus, strokes, and expansion should remain consistent with the theme. Avoid scattered `Color32::from_*` literals for repeated semantic roles.

Test both light and dark themes, disabled state, focus, selection, hover, and high-contrast product palettes. Color alone should not be the only indicator of critical state.

## Fonts and typography

Install fonts once in the `CreationContext`. `add_font` extends existing fallback families; `set_fonts` replaces definitions. Fallback order matters: place a preferred font high and broad Unicode/emoji fallbacks later as intended.

Use text styles for repeated hierarchy rather than arbitrary `FontId` sizes at every call site. Keep a small typography scale (heading/body/button/small/monospace plus justified product variants). Global zoom should handle accessibility/DPI scaling; viewport-width-derived font size is usually unstable.

Text-specific rules:

- bound layout width before creating a `Galley`;
- choose wrap/truncate explicitly;
- use `RichText` for local emphasis, not a second theming system;
- connect separate visible labels to their input with `labelled_by`;
- provide fonts for all user-supported scripts;
- expect font/style/zoom changes to invalidate text measurement caches.

If custom painting needs text size, lay out the galley first, allocate using its size, then paint it. Do not retain a `Painter`; retaining a cacheable `Arc<Galley>` is a different, content/style-keyed optimization.

## Standard image loading

For normal images, use egui's loader chain and `egui_extras`:

1. enable only the loaders/formats needed;
2. call `egui_extras::install_image_loaders(&cc.egui_ctx)` once;
3. use `include_image!`, a URI, bytes, or a texture source with `Image`;
4. set size/fit/corner/tint/alt text through the builder.

The chain is bytes → decoded color image → GPU texture. Loaders are expected to cache by URI and texture options and call `request_repaint` when asynchronous work becomes ready.

URI caveats:

- native `file://` support is feature/platform-specific;
- relative file paths resolve from the process current working directory, not the Rust source file;
- web HTTP/CORS and asset hosting apply;
- enable raster formats explicitly rather than assuming the `image` crate decodes all formats.

Give images meaningful `alt_text`, unless purely decorative.

## Manual texture ownership

If creating textures directly with `Context::load_texture`, do it when image content changes and keep the returned `TextureHandle` in app/resource state. Calling it every UI pass repeatedly allocates textures and is explicitly incompatible with the immediate-mode loop.

For many photographs:

- cache decoded/thumbnail content by stable asset key and revision;
- generate thumbnails off the UI thread;
- choose upload resolution based on displayed size/zoom and GPU limits;
- release handles/cache entries when assets are no longer needed;
- avoid cloning full image buffers per pass—use `Arc`/handles;
- use `Options::reduce_texture_memory` only after understanding that decoded CPU copies may be discarded;
- distinguish preview color pipeline from high-quality export.

Animated images should schedule only the next frame deadline. Remote/disk image errors need visible placeholder/retry behavior rather than an endless spinner.

## Custom GPU rendering

eframe paints egui after `App::ui`; direct GL/wgpu commands inside `ui` do not automatically interleave with collected shapes. Use a backend-specific `PaintCallback`, registered native texture, or render-to-texture workflow.

Resource rules:

- create render resources through `CreationContext`/render state where possible;
- keep resources alive in an `Arc`/app resource manager until the deferred callback executes;
- clip/scissor to the callback rectangle;
- restore or respect renderer state as required by the backend;
- match logical points to framebuffer pixels using pixels-per-point;
- request repaint only while the custom scene changes;
- account for device loss/surface errors and test backend feature availability.

For a custom integration, process `TexturesDelta::set` before painting and `free` after painting, honor each viewport's output, execute platform commands, and tessellate with the returned pixels-per-point. egui expects premultiplied alpha blending and its documented gamma-space behavior.

## Renderer troubleshooting

- Blurry text: verify `pixels_per_point` and sampler alignment.
- Jagged/vanishing shapes: disable backface culling for egui geometry.
- Wrong transparency/darkness: use `(ONE, ONE_MINUS_SRC_ALPHA)`, clamp sampling, and follow egui's gamma-space rules.
- Content escaping panels: use the UI/clipped painter rather than a global painter.
- Hover visual behind content: insert background before foreground or use correct layer/order.
- Excess GPU memory: find repeated texture creation, oversized uploads, and handles retained indefinitely.

## Evidence map

- Painter: [`crates/egui/src/painter.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/painter.rs)
- Rendering/integration requirements: [`crates/egui/src/lib.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/lib.rs), [`crates/eframe/src/epi.rs`](https://github.com/emilk/egui/blob/b865da194/crates/eframe/src/epi.rs)
- Image loader model: [`crates/egui/src/load.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/load.rs), [`crates/egui/src/widgets/image.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/widgets/image.rs), [`crates/egui_extras/src/loaders.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui_extras/src/loaders.rs)
- Font/style examples: [`examples/custom_font/src/main.rs`](https://github.com/emilk/egui/blob/b865da194/examples/custom_font/src/main.rs), [`examples/custom_style/src/main.rs`](https://github.com/emilk/egui/blob/b865da194/examples/custom_style/src/main.rs), [`examples/custom_font_style/src/main.rs`](https://github.com/emilk/egui/blob/b865da194/examples/custom_font_style/src/main.rs)
- Custom renderer examples: [`examples/custom_3d_glow/`](https://github.com/emilk/egui/tree/b865da194/examples/custom_3d_glow/)
