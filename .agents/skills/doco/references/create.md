<!-- doco:managed template=v1 -->
# Create

1. Decide whether a tracked change is actually requested or useful. Creating a
   doco change is not required for routine behavior fixes or implementation
   details, and doco is not an implementation-history mechanism. Create one when
   the user requests tracking or the goal benefits from durable design,
   coordination, handoff or lifecycle review. Run `doco list`; reuse an active ID
   for the same goal instead of fragmenting follow-up work into new changes.
2. Read current architecture, relevant specs, effective ADRs and actual source
   and tests. Exclude unrelated active changes, completed snapshots, archived
   history and tmp unless explicitly needed. CLI context is a path aid, not a
   semantic relevance engine.
3. For a new goal, agree on a unique lowercase-hyphen ID. Use `doco new <id>
   --proposal-only` only when intended behavior and acceptance fit completely in
   proposal.md, no unresolved design choice or dependent task breakdown is needed,
   and one implementer can safely execute and verify it directly. Otherwise use
   `doco new <id>` for the default full package. Templates under `../templates/`
   are skeletons, not finished designs; replace every TODO placeholder. The mode
   is explicit: never infer it from line count or silently omit files from a full
   package.
4. Write proposal: problem, motivation, scope, non-goals, acceptance criteria and
   intended contract changes; set Result to Pending until delivered. It must
   remain understandable after all of work/ is deleted.
5. For a full package, decide whether separate target specs help clarify the
   change. Multiple interfaces, complex state, compatibility or cross-module
   behavior may benefit from work/specs/<capability>.md, using
   `../templates/spec.md`. Both the directory and files are optional: create them
   only when needed, now or later; `doco new` does not generate them. Do not create
   empty directories or placeholder specs for every change. Use lowercase `.md`
   files; subdirectories are supported. A proposal-only package cannot contain
   work specs; convert to a full package first if separate requirements are needed.
   Describe observable requirements, boundaries and acceptance scenarios, linking
   current contracts and identifying intended changes without copying whole current
   specs. Work specs are target contracts, not current facts. Resolve conflicts
   before execution; do not silently let planned behavior override current specs.
6. For a full package, write work/implement.md: actual baseline and affected source entry points;
   distinguish existing from planned APIs. Cover module boundaries, call/data
   flow, API signatures, error semantics, state ownership, lifetime, persistence,
   concurrency and compatibility where relevant. Specify algorithm steps,
   invariants, edge cases, failure handling and resource/performance constraints.
   A Git commit is only a locator; also account for uncommitted relevant changes.
   Link relevant work specs for requirements; keep implementation choices here
   rather than maintaining duplicate contract text.
7. Close correctness/architecture/API/core-algorithm decisions before handing off.
   State fixed choices versus local discretion, blocking open questions,
   verification and current-document impact (explicitly none when appropriate).
8. For a full package, write work/tasks.md: `- [ ] 1.1 action`, stable unique two-level positive-integer
   IDs (`1.1`, `1.2`, `2.1`; no zero or leading zero), Acceptance lines,
   Dependencies lines where needed, and a final verification task. Dependencies
   must exist and be acyclic. Include document updates only if actually affected.
   Use relative package links; cross-change references use `doco:<stable-id>`.
   Link applicable spec acceptance scenarios from tasks. Put detailed work-spec
   links in implement/tasks, not permanent dependencies in proposal; retained
   documents must stand alone after work/ is deleted.
9. Run `doco check <id>` and resolve mechanical errors. Review semantic design
   completeness yourself: another implementer must not have to invent key choices.

Generate optional visual HTML only on request in `doco/tmp/<id>/`; it is a
throwaway explanation, never requirements or execution state.
