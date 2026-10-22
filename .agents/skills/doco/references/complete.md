<!-- doco:managed template=v1 -->
# Review or complete

Review the selected active change against its goal, approved design and actual
source/contracts. Verify all in-scope tasks really completed, agreed acceptance
has actual evidence, no unresolved blockers remain, and affected architecture,
specs and durable decisions have been synchronized with delivered facts.

For full changes with work/specs/, review each relevant requirement and acceptance
scenario against actual implementation and task evidence. Resolve explicit blockers
and contract disagreements. Merge delivered, approved long-term contracts into
current doco/specs/ before completion, without copying unfinished plans or replacing
unrelated current facts. Work specs remain change snapshots and will be deleted on
archive; retained documents must not depend on them. Mechanical spec checks only
cover nonempty bodies, placeholders, references and explicit blockers, not semantic
completeness or actual acceptance.

For proposal-only changes, review each proposal acceptance criterion directly.
Before completion, Result must describe the delivered outcome and proposal.md must
contain concise actual verification evidence. No task checkbox is implied or
required.

If an environment cannot run required acceptance, report the limitation. Do not
count missing evidence as success unless the user explicitly accepts the stated
limitation or approves revised acceptance. Record that decision and the actual
verification performed; a checkbox/confirmation flag does not prove correctness.

Update proposal Result with delivered scope, material adjustments and verification
conclusion. Keep it concise and independently readable without work/. Do not copy
implementation detail or task logs into the historical summary.

Run `doco check <id>`. Only when semantic review is complete and completion was
requested, run `doco complete <id>`. CLI success means mechanical checks and the
whole-directory move succeeded; it does not certify semantic acceptance. completed/
retains the complete work package as a delivery snapshot, not current truth. After
`doco complete`, no need to re-list or read completed/ to confirm the move.
Completion does not commit, merge or publish and does not authorize archiving.

If corrections are needed later, use `doco reopen <id>` with approval, then reset
or add tasks and verify again. Keep previously independent valid evidence.
