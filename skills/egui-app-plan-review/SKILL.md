---
name: egui-app-plan-review
description: Plan, architect, and review production Rust applications built with egui or eframe. Use when Codex needs to create an implementation plan, assess a proposed UI architecture, review a diff or codebase, diagnose egui-specific risks, or audit state, IDs, layout, custom widgets, interaction, focus, painting, images, async work, repaint behavior, performance, accessibility, testing, persistence, viewports, or native/web integration.
---

# egui App Plan and Review

Produce code-backed plans and reviews for egui applications. Treat immediate-mode lifecycle, identity, interaction ownership, and repaint scheduling as correctness concerns rather than cosmetic details.

## Select the workflow

- For a requested implementation plan, read [references/planning.md](references/planning.md), the [guide index](references/egui-app-best-practices/README.md), and the relevant guide chapters below.
- For a code, diff, PR, or architecture review, read [references/review.md](references/review.md), [the review checklist](references/egui-app-best-practices/10-review-checklists.md), and the relevant guide chapters below.
- For a combined plan and review, complete the review first, then make the plan respond directly to verified findings.
- For a narrow question, load only the relevant chapters instead of the full guide.

### Guide chapter routing

- Lifecycle, immediate mode, passes, repaint model: [01-mental-model.md](references/egui-app-best-practices/01-mental-model.md)
- Application structure, eframe lifecycle, workers, persistence, viewports: [02-application-architecture.md](references/egui-app-best-practices/02-application-architecture.md)
- IDs, app/widget state, focus, drafts, selection: [03-state-and-identity.md](references/egui-app-best-practices/03-state-and-identity.md)
- Panels, allocation, child UIs, text, scrolling, multipass sizing: [04-layout-and-containers.md](references/egui-app-best-practices/04-layout-and-containers.md)
- Responses, senses, shortcuts, custom widgets and canvases: [05-widgets-and-interaction.md](references/egui-app-best-practices/05-widgets-and-interaction.md)
- Painting, themes, fonts, images, textures, custom GPU work: [06-painting-style-and-media.md](references/egui-app-best-practices/06-painting-style-and-media.md)
- UI-thread work, repaint, async, locks, virtualization, caching: [07-performance-and-background-work.md](references/egui-app-best-practices/07-performance-and-background-work.md)
- Accessibility, kittest, snapshots, native/web/custom integration: [08-accessibility-testing-and-portability.md](references/egui-app-best-practices/08-accessibility-testing-and-portability.md)

## Respect the product's accessibility scope

Accessibility is an opt-in review and planning dimension unless the repository or user makes it a requirement. For `photo-book`, accessibility is currently out of scope: do not load the accessibility guidance, report missing AccessKit metadata or keyboard-only operation, require focus indicators, add accessibility-specific tests, or plan accessibility work unless the user explicitly requests it. Preserve existing accessibility behavior when practical, but do not treat semantic completeness as a release requirement.

## Establish evidence before conclusions

1. Read repository instructions such as `AGENTS.md` and local conventions.
2. Inspect the worktree before attributing changes. Preserve unrelated user edits.
3. Determine the exact egui/eframe version and enabled features from manifests and lockfiles.
4. Read the target code, its callers, state owners, and relevant tests.
5. Verify unstable or version-specific API claims against the dependency source, local sibling checkout, or official documentation for that version.
6. Prefer evidence in this order:
   - target application's behavior and tests;
   - matching-version egui/eframe contracts and source;
   - official examples and demos;
   - repository conventions;
   - inference, explicitly labeled.

Do not treat every example shortcut as a production recommendation. Examples may intentionally block, force repaint, unwrap, or simplify shutdown to demonstrate one API.

## Apply the immediate-mode invariants

Always check these invariants when relevant:

- UI code is a cheap, repeatable projection and may run multiple passes per visible frame.
- Domain state belongs to the application; egui memory holds superficial widget state.
- Stateful or reorderable content uses stable, scoped domain-derived IDs.
- Top-level panels are siblings, ordered outer-to-inner, with `CentralPanel` last.
- Manual painting and child UIs allocate or advance the same region they visually occupy.
- Custom widgets follow size → allocate → interact/mutate → metadata → visible paint.
- Ordinary activation uses semantic `Response` methods, preserving keyboard and accessibility behavior.
- Shortcut and raw-input handlers have explicit focus ownership and consume owned events.
- Slow work never blocks the UI thread; completion publishes state and wakes egui once.
- Repaint runs only while visible output can change; long collections are virtualized, not merely clipped.
- Textures, fonts, loaders, and renderer resources have explicit lifetime and invalidation rules.
- When accessibility is in scope, custom controls expose roles, names, values, focus, and keyboard/accessibility actions.

## Keep plan and review behavior distinct

For a plan:

- Do not modify code unless the user also asks for implementation.
- Resolve enough codebase detail that the plan names concrete files, owners, state transitions, risks, and verification.
- State open decisions only when they materially change the architecture.
- Prefer staged changes that preserve a working application and make regressions observable.

For a review:

- Lead with findings, ordered by severity.
- Cite exact files and line numbers or symbols.
- Explain the triggering path and user/runtime consequence, not just a stylistic preference.
- Separate verified defects, high-confidence risks, and contextual tradeoffs.
- Do not implement fixes unless asked.
- If no findings remain, say so and list residual testing gaps.

## Check repository-specific guidance

When working in `photo-book`, also read `docs/conventions/` and inspect the matching source rather than assuming the convention is fully enforced. Pay special attention to:

- canvas/viewer shortcut focus ownership;
- paint-only previews and stable transform/layer IDs;
- allocation exactly once for transformable controls;
- worker completion wakeups versus busy repaint;
- export/offscreen loading termination;
- layer visibility/locking across edit, preview, hit-test, persistence, and export;
- full-dataset clone cost despite UI virtualization;
- sizing-cache invalidation across content/font/style/zoom;
- scoped versus global style mutation;
- async runtime shutdown and immutable autosave snapshots.

Use these as audit leads, not permanent claims. Re-verify them against the current worktree.

## Keep bundled evidence version-aware

The bundled guide was researched against egui `0.35.0` at commit `b865da194`. Use its invariants and source map, but verify exact API names and changed contracts whenever the target uses another version.
