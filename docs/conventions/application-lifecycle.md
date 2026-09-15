# Application Lifecycle And Interaction Ownership

## Runtime ownership

The native entry point is synchronous and owns exactly one multi-thread Tokio runtime. Enter that
runtime for the complete `eframe::run_native` lifetime, leave the enter guard, and only then drop
the runtime. Do not add `#[tokio::main]` around code that constructs another runtime.

CPU-bound work and blocking file/compression work belongs in `spawn_blocking`. Every operation must
have an explicit ready, error, timeout, and (when user initiated) cancellation path.

Native file dialogs use the asynchronous `.spawn()` API and `file_dialog::spawn` for completion
and repaint. Avoid synchronous `.show()` inside UI code: native-dialog's macOS implementation
changes the application's activation policy while an unowned modal is open, which can hide the
main window. Keep pending dialog results in the owning UI state and preserve selection on cancel.

## Keyboard ownership

Canvas and viewer shortcuts are local commands, not global input listeners. Their primary surface
must own focus before they consume a key. A focused `TextEdit`, autocomplete, slider, modal, or
other control owns its keys. Use `input_mut.consume_key`/`consume_shortcut`, match the most-specific
shortcut first, and do not also inspect the consumed event elsewhere.

## Stable identity and previews

Derive editor IDs from stable page/layer/domain IDs. Collection indices are reorder destinations,
not widget or drag-payload identity. A random ID may be generated once for a newly created domain
object, but never while building a frame or preview.

Page, quick-layout, export, and other thumbnails are paint-only. They may call media painting and
text/shape painting helpers, but must not allocate transform bodies, handles, text editors, focus
targets, or invisible click/drag regions. The parent thumbnail owns selection and DnD interaction.

## Repaint and background completion

Static pending/error placeholders request no repaint. The producer publishes its result and wakes
the captured `egui::Context`. A staged GPU upload may need one decode wakeup and one final publish
wakeup; neither stage may poll with an unconditional per-pass repaint. Timed repaint is reserved
for a real deadline or bounded fallback.

## Query and layout caches

Virtualized views consume shared immutable query snapshots. Do not clone all `Photo` values or
rebuild ordered path vectors every UI pass. Cache derived row metadata by query revision and every
layout input that affects it; construct a complete ordered-path vector only when range selection
actually needs one.

Prefer intrinsic egui layout and font galley measurement over a long-lived application sizing
cache. If a new measurement cache is unavoidable, its key must include content revision,
available width, font/style generation, and zoom.

## Snapshots and lock boundaries

Autosave captures an immutable `Project` at the trigger boundary. Serialization, compression, and
disk I/O operate only on that value and never return to live scene locks.

Global dependency locks and scene locks follow these rules:

1. Acquire at most one dependency singleton lock at a time. Copy the narrow value needed and
   release it before calling another `dep!`/`dep_mut!` operation.
2. Do not acquire a dependency singleton while holding a scene write lock if the reverse order is
   possible elsewhere. UI code should avoid introducing any nested dependency/scene lock order.
3. Never perform file I/O, decode, export, compression, or an unbounded wait while holding either
   kind of lock.
4. Scene write guards may cover the immediate `scene.ui` call under the current architecture, but
   new slow derivation and worker orchestration must happen before or after that guard.
5. Use the `debug_dependency_locks` feature and profiling scopes when changing shared ownership.
   Replace the service locator incrementally only where measurements or tests justify the change.

## Local style and custom controls

Temporary visual changes use `ui.scope`/a child UI. Components must not call `global_style_mut` for
a one-shot behavior or mutate the supplied UI style in a way that leaks to later siblings.

Custom controls allocate once per intended interaction region and mark changed only on a real value
transition. Compound controls may have deliberate child regions (for example a chip's remove
target), each with its own derived stable ID and response.
