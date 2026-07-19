# Review Checklists and Failure Modes

Use this as an application-review entry point. A checked item means it was inspected and is acceptable—not merely that the string was searched.

## Lifecycle and architecture

- [ ] The code targets the checkout's `App::logic`/`App::ui` API, not obsolete `update` guidance.
- [ ] Fonts, style, image loaders, persistence restore, and renderer resources are initialized from `CreationContext` where possible.
- [ ] `ui` is a cheap repeatable projection; it does not assume one call per presented frame.
- [ ] `logic` shows/paints no UI and performs no blocking work.
- [ ] Domain state, view state, services, and egui superficial state have clear owners.
- [ ] Cross-module mutations are returned as actions or applied after active borrows/iteration.
- [ ] No `Ui`, `Painter`, `Response`, or widget builder is retained across passes.
- [ ] Shutdown, cancellation, unsaved work, and persistence errors have defined behavior.

## IDs and state

- [ ] Reorderable/filterable/virtualized items use stable domain keys, not indices.
- [ ] Repeated subtrees are scoped with `push_id`.
- [ ] Sibling `Grid`, table, scroll, collapsing, combo, plot, resize, and text-edit state has unique salts.
- [ ] Dynamic/duplicate window and collapsing titles have explicit stable identity.
- [ ] Random IDs are generated once and stored, never regenerated during UI building.
- [ ] Root area/panel/viewport IDs are globally unique and stable.
- [ ] Debug ID clash warnings remain enabled and clean.
- [ ] Important data is not hidden in `Memory::data`.
- [ ] Draft/commit state is initialized on a transition rather than overwritten every pass.
- [ ] Selection/drag payloads are stable domain IDs and are validated after collection changes.

## Layout and containers

- [ ] Top-level panels are siblings, ordered outer-to-inner, with central last and windows afterward.
- [ ] Root UI has an intentional frame/background/margin.
- [ ] Resizable panels/windows consume their available size or intentionally auto-shrink.
- [ ] Every manually painted large region also allocates/advances that region.
- [ ] Every `new_child` has a bounded rect, ID scope, clip policy, and parent cursor reconciliation.
- [ ] `with_layout` is not accidentally claiming all remaining space.
- [ ] `centered_and_justified` contains exactly one widget.
- [ ] Text has explicit wrap/truncate/scroll behavior in constrained areas.
- [ ] Narrow windows, long strings, non-Latin text, empty data, and high zoom were tested.
- [ ] Long scroll content is virtualized, not only clipped.

## Interaction and widgets

- [ ] Ordinary activation uses `Response::clicked`, preserving keyboard/accessibility behavior.
- [ ] Each control uses the narrowest `Sense`; touch scrolling is not accidentally captured.
- [ ] Custom widgets follow size → allocate → interact/mutate → metadata → visible paint.
- [ ] Custom mutable controls call `mark_changed` only when bound content changes.
- [ ] Standard interaction visuals come from the current style/response.
- [ ] Hit targets follow `interact_size` and are not limited to tiny painted icons.
- [ ] Focus, Enter/Space/Escape, context menu, and drag-stop behavior are defined.
- [ ] Global shortcuts are outside conditionally executed menu/window closures and are consumed once.
- [ ] Specific shortcuts are matched before less-specific variants.
- [ ] Context/response access does not re-enter a context-lock closure.
- [ ] Collection mutations/reorders occur after iteration.

## Painting, text, and media

- [ ] Painting uses allocated absolute-point rectangles and correct clipping/layers.
- [ ] Expensive offscreen/discarded painting is skipped.
- [ ] Large shape batches use `extend` where useful.
- [ ] Domain, screen-point, and texture-pixel coordinate systems are explicit.
- [ ] Global styling/fonts/loaders are not reinstalled each frame.
- [ ] Local style overrides are scoped.
- [ ] Font fallback covers supported scripts and measurement caches invalidate on font/zoom/style change.
- [ ] Direct textures are loaded once per content revision and handles are retained/released deliberately.
- [ ] Photo grids use thumbnails rather than full-resolution uploads.
- [ ] Meaningful images have alt text; loading/error/retry states are visible.
- [ ] Custom GPU callbacks retain resources until deferred execution and schedule repaint only while changing.

## Performance and concurrency

- [ ] A static idle app consumes no continuous UI CPU.
- [ ] Repaint causes are known; clocks use timed deadlines and workers wake on completion.
- [ ] No filesystem/network/decode/export/database/blocking receive/join runs on the UI thread.
- [ ] UI-thread locks are short and no slow work occurs while holding shared locks.
- [ ] Async results are generation-tagged/cancelable so stale work cannot win.
- [ ] Result queues are bounded/coalesced and drained without monopolizing a pass.
- [ ] Caches have complete invalidation keys and memory/eviction policy.
- [ ] `request_discard` occurs only on transitions and does not recur.
- [ ] Immediate viewports were chosen with measured repaint cost.
- [ ] Performance was measured in a release-like build with representative data.

## Accessibility and tests

- [ ] Separate labels are connected with `labelled_by`.
- [ ] Custom widgets expose roles, names, enabled/selected/value state, and keyboard activation.
- [ ] Focus order and focus indication are usable without a pointer.
- [ ] Critical state is not conveyed by color/hover alone.
- [ ] Semantic kittest coverage exists for core actions, state changes, focus, reordering, and errors.
- [ ] Layout tests cover explicit small/large/high-zoom dimensions.
- [ ] Tests inject deterministic time/data and avoid real network.
- [ ] Image snapshots are small, focused, reviewed, and only used for visual contracts.
- [ ] Snapshot tolerance was not broadened without proving a known regression still fails.
- [ ] Platform smoke tests cover supported OS/web/backend features.

## High-signal searches

These searches identify review candidates; each result still needs context:

```sh
rg 'request_repaint\(|request_discard\(' src
rg 'load_texture|install_image_loaders|set_fonts|set_global_style' src
rg 'ScrollArea::.*show\(|TableBuilder|for .* in ' src
rg 'Id::new|id_salt|push_id|make_persistent_id|new_child' src
rg 'Painter|painter\(\)|rect_filled|allocate_|advance_cursor' src
rg '\.lock\(|\.read\(|\.write\(|recv\(|join\(|block_on|spawn' src
rg 'clicked_by|pointer\.|input_mut|consume_shortcut|mark_changed' src
rg 'unwrap\(|expect\(' src
```

## Symptom-to-cause map

| Symptom | First suspects |
| --- | --- |
| Text focus/cursor jumps between rows | unstable/colliding IDs, index identity |
| Scroll areas share position | missing distinct `id_salt` |
| Window forgets position after title change | title used as dynamic ID |
| Panel snaps smaller after resize | auto-shrink with content not consuming space |
| Content overlaps next widget | manual paint/child UI did not advance parent |
| Touch cannot scroll | child uses drag or click-and-drag sense too broadly |
| Button works with mouse only | raw pointer/`clicked_by` instead of semantic activation |
| UI stays stale until mouse moves | background completion omitted `request_repaint` |
| Laptop fan stays active while idle | unconditional repaint, recurrent animation/discard/polling |
| First view freezes | setup or I/O/decode performed in first UI pass |
| Memory/GPU use grows | texture recreated per pass, unbounded cache, retained old handles |
| Large list is slow despite scroll clipping | all rows still laid out; missing virtualization |
| Deadlock inside input callback | context re-entered while locked |
| Action happens twice occasionally | non-idempotent side effect in multipass/sizing closure |
| Snapshot flaky across machines | nondeterminism or backend GPU variation/tolerance misuse |
| Child window multiplies CPU | immediate viewport repaint coupling |

Follow each suspected cause into the corresponding guide chapter before changing code.
