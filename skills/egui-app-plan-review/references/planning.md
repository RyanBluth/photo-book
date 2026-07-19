# Planning Workflow

## Table of contents

- [Goal](#goal)
- [Discovery](#discovery)
- [Architecture questions](#architecture-questions)
- [Plan construction](#plan-construction)
- [Output template](#output-template)
- [Plan quality gate](#plan-quality-gate)

## Goal

Create an implementation-ready plan grounded in the current codebase and exact egui version. A good plan tells an implementer where state lives, how events move, what identities remain stable, how background work wakes the UI, and how correctness will be tested.

## Discovery

Inspect before proposing:

1. Repository instructions and existing convention documents.
2. `Cargo.toml`/`Cargo.lock` for egui, eframe, egui_extras, egui_tiles, renderer, persistence, accessibility, and test features.
3. Application entry point and `eframe::App` lifecycle methods.
4. The target screen/widget plus callers and state owners.
5. Models, IDs, caches, workers, persistence, and rendering resources touched by the feature.
6. Existing analogous components and tests.
7. Relevant matching-version egui APIs/source.

Map the feature flow:

```text
user/external input
  → Response or consumed command
  → app action/state transition
  → worker/resource operation if needed
  → result publication + request_repaint
  → next UI projection
  → persistence/export if applicable
```

## Architecture questions

Answer all relevant questions before finalizing the plan.

### State and identity

- What is durable domain state, app-owned view state, draft state, superficial egui state, and derived cache?
- What stable domain key identifies each repeated/stateful widget?
- Can content reorder, filter, virtualize, move between parents, or change its visible label?
- How are selection, focus, drag payloads, and modal state invalidated?

### Layout and interaction

- Which built-in panel/container/widget owns layout, clipping, scroll, popup, or accessibility behavior?
- What rectangle is allocated, painted, and returned in the response?
- Who owns keyboard focus and which events are consumed?
- Is the narrowest correct `Sense` used without breaking touch scrolling?
- How do narrow windows, long/localized text, high zoom, and empty/large states behave?

### Async, rendering, and performance

- What work can block or exceed the frame budget?
- What immutable request enters the worker, and how are stale results rejected or canceled?
- What exact completion path calls `request_repaint`?
- Are polling, cache, retry, timeout, and shutdown bounded?
- Which resources persist across passes, and what invalidates/releases them?
- Does virtualization eliminate both UI work and model cloning?
- Can multipass sizing repeat a side effect or expensive operation?

### Product boundaries

- Does the feature affect persistence schema, autosave, preview, export, undo/redo, accessibility, or platform-specific behavior?
- What migration/fallback/error behavior is required?
- Does a viewport need native and embedded behavior?

## Plan construction

Order steps so each can be implemented and verified independently:

1. Introduce or adjust state/model contracts.
2. Define semantic actions and background/resource interfaces.
3. Establish stable IDs and focus/input ownership.
4. Implement layout/widget/rendering behavior using existing primitives.
5. Connect async completion, repaint, cancellation, and errors.
6. Integrate persistence, preview/export, and platform paths.
7. Add unit, semantic UI, layout, async, and visual tests as appropriate.
8. Add profiling/observability for performance-sensitive work.

Name concrete files and symbols. For every step include:

- intended behavior;
- state/API changes;
- egui-specific invariants;
- risks or compatibility constraints;
- verification.

Avoid plans that merely say “update the UI,” “add state,” or “test it.”

## Output template

Use the minimum structure that stays implementation-ready:

```markdown
## Outcome

One paragraph describing final behavior and architectural boundary.

## Current-state evidence

- Existing owners, code paths, analogous patterns, and version constraints.

## Design decisions

- State ownership, IDs, focus/input, async/repaint, layout/rendering, persistence.

## Implementation plan

1. **Step — files/symbols**
   - Changes
   - Invariants and risks
   - Verification

## Verification matrix

- Pure model tests
- Semantic interaction/accessibility tests
- Layout/performance/visual/platform checks

## Open decisions

- Only decisions that materially alter the plan, with a recommended default.
```

For a small change, compress this to a short numbered plan while retaining files, state ownership, and verification.

## Plan quality gate

- The plan matches the checked-out egui API and features.
- Every important state value has one clear owner.
- Stable identity survives reorder/filter/virtualization where required.
- Focus and shortcut ownership are explicit.
- UI-thread work is bounded and completion wakeups are named.
- Allocation, clipping, painting, and response geometry agree.
- Preview/export/persistence behavior is included when the model changes.
- Tests prove user-visible behavior, not implementation details alone.
- Performance-sensitive changes include representative data and measurement.
- No step silently overwrites unrelated worktree changes.
