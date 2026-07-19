# Performance and Background Work

## Optimize frame work before low-level drawing

The dominant application mistakes are usually architectural:

1. continuous repaint when nothing changes;
2. blocking UI-thread work or lock contention;
3. laying out huge offscreen collections;
4. repeating decode/texture/font/resource initialization;
5. recurrent multipass layout;
6. expensive derived computation without caching;
7. repaint-coupled multiple viewports.

Profile before micro-optimizing widget calls. Enable the repository's `profiling` instrumentation and measure release-like builds. Track UI/pass time, repaint causes, worker queue latency, cache/texture memory, list sizes, and number of multipass frames.

## Keep the UI thread nonblocking

Never perform these synchronously in ordinary UI code when latency is material:

- filesystem traversal or metadata over many paths;
- network requests;
- database queries that can block;
- full-size image decode/resample/histograms;
- project serialization or export;
- blocking channel `recv`, thread `join`, or lock acquisition with unbounded hold time;
- CPU-bound async tasks on an executor lane intended for I/O.

File dialogs and tiny reads may be acceptable product tradeoffs, but recognize that the window freezes until they return. Examples using synchronous dialogs or blocking parallel UI are demonstrations, not blanket production endorsements.

## Background result pattern

```rust
struct Jobs {
    next_generation: u64,
    active: Option<u64>,
    receiver: Receiver<JobResult>,
}

fn start(&mut self, ctx: egui::Context, request: Request) {
    let generation = self.next_generation;
    self.next_generation += 1;
    self.active = Some(generation);
    worker_pool.spawn(move || {
        let result = run(request);
        let _ = sender.send(JobResult { generation, result });
        ctx.request_repaint();
    });
}
```

Drain with `try_recv`, not `recv`. Tag requests so an old result cannot overwrite newer state. Bound queues or coalesce progress events. Use cancellation for work with meaningful cost. On shutdown, signal workers and avoid an indefinite UI-thread join.

With Tokio, use `spawn_blocking` for CPU/blocking work. Do not `.await` a long operation in the synchronous UI projection. A future/promise can be stored and polled, but the completion must wake egui.

## Locks and shared state

Short locks around a small snapshot or result swap are acceptable. Better patterns include channels, immutable `Arc` snapshots, atomics for scalar progress, and double-buffered resources.

Avoid:

- holding a global dependency lock while running a large widget subtree;
- holding one lock while acquiring another without a documented order;
- doing decode/I/O while a shared lock is held;
- cloning a full project/image merely to escape a borrow each pass;
- poisoning/deadlock errors silently converted into stale UI.

`Context` is itself synchronized. Keep its accessor closures short and never call back into the same context from inside one.

## Repaint budget

Use reactive rendering by default:

- request immediate repaint for a state change that should appear now;
- request a timed repaint at the next meaningful visual boundary;
- keep requesting while an animation remains active;
- do nothing while static;
- have background completions request repaint;
- diagnose hot loops with `repaint_causes`.

Do not implement polling by calling `request_repaint` forever. If a non-notifying external source truly requires polling, use `request_repaint_after` at the product's necessary rate and stop when inactive/hidden.

`App::logic` can advance cheap background state while UI is hidden, but it still runs on the application thread. Moving code from `ui` to `logic` does not make slow code asynchronous.

## Virtualization and culling

Clipping only reduces pixels. Virtualization reduces layout and widget work.

- fixed-height list: `ScrollArea::show_rows`;
- variable-height list: measured index plus viewport range;
- gallery/spatial editor: viewport intersection/spatial index;
- huge text/log: line index, bounded history, or a specialized editor;
- custom painting: check `is_rect_visible` and batch shapes.

Preserve stable IDs when only a visible subset is instantiated. Do not render ten thousand invisible buttons and call it optimized because the painter clips them.

## Caching

Cache work with a clear cost and invalidation key:

- syntax highlighting/layout jobs keyed by text revision + style;
- filtered/sorted indices keyed by query + source revision;
- thumbnails keyed by path/content revision + target size;
- histograms/meshes keyed by source revision + parameters;
- text measurement keyed by content/font/wrap width/zoom.

`Memory::caches`/`FrameCache` is useful for derived values that are cheap to key and should evict when unused. Application caches are better for domain data, async resources, budgets, explicit invalidation, and cross-view reuse.

Do not cache widget builders or responses. Do not create an unbounded cache whose key includes a continuously changing float or screen rect. Quantize target sizes where appropriate and track memory budgets.

The repository describes roughly 1–100 ms synchronous computations as cache candidates and >100 ms work as async candidates. Treat that as scale guidance, not a frame budget: at 60 Hz, even a recurring 2–5 ms app computation can be too expensive.

## Multipass cost

`request_discard` can roughly double a frame's UI work. Keep the default cap, request only on a sizing transition, and skip expensive paint/resource work when `will_discard` is true. A custom measurement system that renders whole complex subtrees invisibly may be even more costly and can duplicate side effects; measure it against built-in alternatives.

## Texture and media cost

- Install loaders once.
- Retain `TextureHandle`s; do not call `load_texture` every pass.
- Decode/generate thumbnails off-thread.
- Upload only useful resolution and avoid full-resolution photo grids.
- Cull offscreen galleries before expensive per-item work.
- Release old revisions and set a cache budget.
- Consider `reduce_texture_memory` for image-heavy GPU-only views after verifying reload/export needs.

## Multiple windows and parallelism

Immediate viewports tie parent and child repaints together; N windows can cause roughly N times the UI work. Deferred viewports repaint independently and are preferable when windows update at different rates.

egui can be invoked from multiple threads in controlled integrations, but parallel UI building is not the default solution for a slow application. Synchronization and ordering costs are substantial. First move domain work off-thread, virtualize, cache, and reduce repaints. Renderer/tessellation parallel features should be enabled only after profiling shows they help the workload.

## Performance review checklist

- What causes every repaint, and can the app idle at zero UI CPU?
- What is the worst UI-pass time in a release-like build?
- Are any I/O, decode, joins, blocking receives, or long locks on the UI thread?
- Do worker completions wake egui and reject stale results?
- Are large collections virtualized at the UI level?
- Are textures/loaders/fonts initialized once?
- Are caches bounded and keyed by all invalidating inputs?
- Does any transition request discard repeatedly?
- Are invisible/discarded custom paints skipped and shapes batched?
- Are immediate viewports multiplying work?

## Evidence map

- CPU/scroll considerations: [`README.md`](https://github.com/emilk/egui/blob/b865da194/README.md), [`crates/egui/src/memory/mod.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/memory/mod.rs)
- Repaint API: [`crates/egui/src/context.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/context.rs)
- Virtualization: [`crates/egui/src/containers/scroll_area.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui/src/containers/scroll_area.rs)
- Cache: [`crates/egui/src/cache/`](https://github.com/emilk/egui/tree/b865da194/crates/egui/src/cache/)
- Async patterns: [`crates/egui_demo_app/src/apps/http_app.rs`](https://github.com/emilk/egui/blob/b865da194/crates/egui_demo_app/src/apps/http_app.rs), [`examples/external_eventloop_async/`](https://github.com/emilk/egui/tree/b865da194/examples/external_eventloop_async/)
- Profiling: [`examples/puffin_profiler/`](https://github.com/emilk/egui/tree/b865da194/examples/puffin_profiler/)
