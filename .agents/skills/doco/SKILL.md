---
name: doco
description: Manage project documentation and doco changes. Use when migrating existing project documentation into doco; creating, executing, reviewing, completing, or archiving a doco change; or maintaining the project's current technical documents.
---
<!-- doco:managed template=v1 -->
<!-- doco:skill version=v5 -->

# Doco

Select the doco library nearest to the working directory or explicitly use
`doco --root <library-root> ...`. Ordinary commands search ancestors, stopping
at the Git worktree root; `init` always targets the given directory instead.
A child library has its own architecture, specs, changes, index and lock, but
no generated skill or Agent entry files. Use the top-level library's installed
skill; existing child instructions remain user-owned. Paths starting with
`doco/` in this workflow refer to the selected library, not the skill directory.
Parent documents describe shared constraints; child documents describe local
facts, without automatic copying or overrides. Read relevant parent constraints
explicitly when needed. Change IDs and `doco:<id>` references are library-local;
never run a lifecycle command in another library just because its ID appears in
`doco list`. The list covers the whole project family, current library first,
then parent-before-child order; other commands operate only on the selected library.

Start from the selected library's `doco/architecture.md` and read only relevant
current specs, effective decisions, source code, and the selected change.
For documentation-only work, follow these current-document rules without
creating a change unless the user requests change tracking.

Change packages preserve goals, decisions and delivery summaries; they are not
implementation logs, because archive retains only proposal.md. Do not create a
change merely to record a routine behavior fix or implementation-detail edit.
Use Git or the project's normal delivery records for implementation history.

Read the reference for the requested action:
- Migrate existing documentation: `references/migrate.md`
- Create: `references/create.md`
- Execute: `references/execute.md`
- Review or complete: `references/complete.md`
- Archive: `references/archive.md`

Do only the requested phase. Do not create a change for discussion alone.
Resolve relative reference paths from this skill directory.

Current architecture describes implemented boundaries, not future plans.
Current specs in doco/specs/ are approved precise contracts, not copies of internal
code. Full changes may optionally use work/specs/**/*.md for target contracts and
acceptance scenarios; these are planned change requirements, not current facts.
Read relevant work specs with the selected change's design and tasks. Synchronize
only delivered, approved contracts into current specs before completion; archive
and cancel delete work specs with the rest of work/.
Investigate code/spec disagreements; never rewrite a spec just to hide an
implementation bug.
Keep only important durable rationale in decisions; label superseded decisions.
Historical proposals and superseded decisions cannot override current contracts.
Keep project document language consistent with the user or project convention.
Installation grants no permission to execute code, complete, archive, or cancel.
