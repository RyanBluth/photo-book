# Review Workflow

## Table of contents

- [Review contract](#review-contract)
- [Inspection sequence](#inspection-sequence)
- [Severity](#severity)
- [Review dimensions](#review-dimensions)
- [Evidence standard](#evidence-standard)
- [Output template](#output-template)
- [No-finding outcome](#no-finding-outcome)

## Review contract

Review for user-visible correctness, data safety, responsiveness, accessibility, portability, and maintainability. Avoid style-only findings unless they create a concrete risk or violate an established repository convention.

Do not implement fixes during a review unless explicitly requested. Read-only diagnostics and targeted tests are allowed when proportionate.

## Inspection sequence

1. Read repository guidance and inspect worktree state.
2. Identify the review baseline: current tree, diff, commit, or PR.
3. Determine dependency versions/features.
4. Read each changed/target file in full enough to understand ownership.
5. Follow callers, response/action handling, model mutations, worker paths, preview/export, persistence, and tests.
6. Compare with matching-version egui contracts/source.
7. Search systematically using relevant patterns:

```sh
rg 'request_repaint\(|request_discard\(' src
rg 'Id::new|id_salt|push_id|make_persistent_id|new_child' src
rg 'load_texture|install_image_loaders|set_fonts|global_style_mut' src
rg 'painter\(\)|allocate_|advance_cursor|interact\(' src
rg 'key_pressed|key_down|consume_key|consume_shortcut|wants_keyboard_input' src
rg '\.lock\(|\.read\(|\.write\(|recv\(|join\(|block_on|spawn' src
rg 'ScrollArea|TableBuilder|show_rows|show_viewport' src
rg 'mark_changed|widget_info|labelled_by|clicked_by' src
```

Search results are leads, not findings.

## Severity

- **Critical**: deterministic crash, data loss/corruption, security issue, or unusable release path.
- **High**: strong user-visible correctness defect, freeze/hot loop, input theft, broken export/persistence, or severe scaling failure.
- **Medium**: localized incorrect behavior, accessibility gap, unstable state, resource leak risk, or maintainability issue with a credible failure path.
- **Low**: limited-impact robustness or convention issue worth fixing.
- **Tradeoff**: measurable architectural cost without enough evidence to call it a defect.

Severity follows consequence and likelihood, not code ugliness.

## Review dimensions

### Lifecycle and multipass

- Does code assume one `ui`/closure call per presented frame?
- Can a sizing/discard pass repeat I/O, telemetry, job creation, or model mutation without an input transition?
- Is setup in `CreationContext` where appropriate?
- Does `logic` avoid UI and blocking work?
- Does runtime/event-loop shutdown follow framework rules?

### State, IDs, and persistence

- Is domain truth app-owned rather than hidden in egui memory?
- Do reordered/filtered/virtualized items use stable domain IDs instead of indices/random per-pass IDs?
- Do repeated stateful containers have distinct salts?
- Do dynamic titles have explicit identity?
- Do model changes reach undo/redo, autosave, persistence, preview, and export consistently?
- Are persisted schemas versioned/defaulted and runtime resources skipped?

### Layout and containers

- Are top-level panels siblings with central last?
- Does each custom-painted or child-UI region allocate/advance its actual visual bounds?
- Does `new_child` have bounded rect, ID scope, clipping, and parent reconciliation?
- Does resizable content consume available space intentionally?
- Are long lists truly virtualized?
- Do text and controls survive narrow/high-zoom/localized states?

### Interaction, focus, and shortcuts

- Does ordinary activation use semantic `clicked()` rather than raw pointer-only tests?
- Is `Sense` narrower than needed or broad enough to steal scrolling?
- Are keyboard shortcuts scoped to a focused owner and consumed once?
- Can text edits/sliders/popups receive keys without canvas/global handlers also acting?
- Are overlapping responses intentional, stable, and correctly layered?
- Does changed state mean bound data actually changed?
- Are collection mutations applied after iteration?

### Painting and resources

- Does painting use allocated absolute-point coordinates and correct clipping?
- Are previews paint-only, or do they accidentally register editor interactions?
- Are textures/loaders/fonts initialized once and resource handles retained/released?
- Do background loaders publish completion and request one repaint?
- Can missing/error states spin forever or remain stale?
- Do custom GPU callbacks retain valid resources through deferred execution?

### Async, locks, and performance

- Can the UI thread block on I/O, decode, database, channel receive, join, or long locks?
- Can nested runtimes or blocking executor misuse panic/starve?
- Are results tagged/cancelable so stale work cannot overwrite current state?
- Are queues, retries, polling, timeouts, and export loops bounded?
- Does a static screen repaint continuously?
- Does virtualization still clone/recompute the full model every pass?
- Are cache invalidation keys complete for content, size, font, style, zoom, and revisions?
- Are UI-held locks and autosave snapshots coherent?

### Accessibility and testing

- Do custom controls expose role, name, enabled/selected/value state, focus, and actions?
- Are separate labels linked and meaningful images named?
- Can keyboard/accessibility activation reach the same behavior as pointer input?
- Do tests cover focus conflicts, ID stability, async wakeup, preview non-interactivity, persistence/export, and normal shutdown?
- Are snapshots focused, deterministic, and feature-gated tests actually run?

## Evidence standard

Each finding must contain:

1. **Location**: exact file and line/symbol.
2. **Mechanism**: what the code does and the triggering path.
3. **Consequence**: specific user/runtime effect.
4. **Confidence**: verified defect, high-confidence risk, or inference/tradeoff.
5. **Direction**: concise correction principle, not an unsolicited full patch.
6. **Verification**: test or reproduction that would close the finding.

Reject a suspected finding if a caller, guard, framework contract, or test disproves the mechanism. If concurrent worktree edits overlap the issue, identify the baseline and re-check the current state before reporting.

## Output template

```markdown
## Findings

### High — Short actionable title

`path/file.rs:line`

Explain mechanism, trigger, and consequence. State whether verified or inferred.

Recommended direction and regression test.

## Positive patterns

- Include only when useful to preserve during remediation.

## Contextual tradeoffs

- Costs that need measurement/product judgment rather than automatic rewriting.

## Verification gaps

- Tests, platforms, features, or runtime conditions not exercised.
```

Keep findings primary. Do not bury them under a long summary.

## No-finding outcome

If no material issues are found, say so directly. Report:

- what code paths and version-specific contracts were checked;
- which tests were run;
- residual risks or untested platform/performance conditions.
