<!-- doco:managed template=v1 -->
# Archive or cancel

Archive is not an implementation-history store. Full packages lose work/, including
optional work/specs/ target contracts, and proposal-only packages never contain it.
Git or normal delivery records must preserve implementation-level history.

Ordinary archive accepts completed changes only. Confirm the proposal stands on
its own and retained files do not depend on work/, including work-spec links.
Current facts were synchronized at completion; do not overwrite current
architecture/specs from old designs or work specs.

Run `doco archive <id> --dry-run` to display the retained proposal, deleted work
entries and destination. Only after explicit authorization use `doco archive <id>`.
The compatibility `--yes` flag is optional; there is no additional confirmation
prompt. All work/ materials are deleted, including work specs and extra
research/logs. Unknown files outside work/ are a conflict, not silently deleted.
Only proposal.md remains in archived/. No Git history or code is modified, and
unrelated tmp HTML is not implicitly cleaned.
After `doco archive`, no need to re-list or read archived/ to confirm the move and
retained proposal.

A failed deletion may leave a partial work/ in the original lifecycle directory.
Report the actual state; resolve the filesystem problem and retry the same command.
If work/ was removed but the move failed, retry still preserves proposal and can
finish the move. Never claim success after only some cleanup steps succeeded.

Cancellation is separate: an active change can go directly to archived without
pretending to be completed. Get explicit approval and provide both the reason and
how already implemented code is retained, reverted separately, or handed off (or
state that none was implemented). Preview with `doco cancel <id> --reason "..."
--disposition "..." --dry-run`; execute with `--yes` only after authorization.
The CLI records those details in proposal Result before deleting work/. It does
not roll back code. Cancelled tasks must not be marked as delivered.

Archived changes cannot be reopened; create a new change referencing the old
`doco:<id>` proposal when new implementation is required.
