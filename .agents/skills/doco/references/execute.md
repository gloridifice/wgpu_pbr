<!-- doco:managed template=v1 -->
# Execute

Use an explicitly selected active change. Read `doco context <id>`, its proposal
and, for a full package, implement, tasks and relevant optional work/specs/**/*.md,
plus relevant current docs, actual source and tests. Work specs define this change's
target contracts and acceptance scenarios, not already implemented current facts.
For proposal-only changes, execute directly against the stated scope and acceptance
criteria. Check the baseline against real interfaces and
uncommitted changes. Do not redesign merely because a new unrelated commit exists.
Check overlapping changes before execution and again before completion.

If proposal-only execution needs separate work specs or reveals unresolved design
choices, dependent steps or materially expanded risk, stop and convert it to a full
package before continuing.
Do not use proposal-only mode to bypass design or verification.

Implement in dependency order. Local naming, helper extraction and equivalent
mechanical fixes are discretionary. Do not silently change public API semantics,
module responsibilities, state/persistence format, concurrency assumptions, core
algorithms, important dependencies or acceptance standards.

On a core mismatch, stop affected tasks and record in work/: original assumption,
observed facts, affected scope, recommendation and pending decision. Mark the task
with `Blocked: <reason>`. Independently completed tasks need not be undone. Obtain
design-owner approval and update affected work specs and implement/tasks (and
proposal for scope changes) before resuming. Keep requirements in specs and refer
to them from design/tasks rather than duplicating them. Remove resolved blockers or
write `Blocked: none`.

Run actual verification before changing `[ ]` to `[x]`. Only these two checkbox
states are valid; in-progress tasks stay unchecked. Record concise `Verification:`
commands and results (per task or in a final section); never call unrun tests passed.
Keep detailed logs in work/ if needed. Remove cancelled tasks from the executable
list only after approved scope adjustment; record the adjustment, do not check them.

Verify applicable work-spec acceptance scenarios and record evidence in tasks.
Synchronize implemented, approved architectural/contract changes into current
documents; do not blindly copy work specs over current specs. Keep unimplemented
plans in work/. Update implement only for approved substantive design changes, not
every local refactor. No document change is needed for purely
internal edits that do not affect documented facts or contracts.

Do not complete or archive automatically. To fix a completed delivery, obtain
permission to `doco reopen <id>`, then add/reset relevant tasks and reverify. For an
archived change, create a new change referencing its stable ID; do not resurrect
and directly execute the discarded old design.
