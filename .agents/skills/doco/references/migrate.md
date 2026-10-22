<!-- doco:managed template=v1 -->
# Migrate existing documentation

Migration is current-document maintenance, not a lifecycle state. Follow only the
phase the user requested: inventory, migration design, or approved application.
Do not create a doco change merely to discuss or assess migration; use one only
when the user requests change tracking.

## 1. Establish scope and authorization

Confirm the project root, accessible documentation sources, and requested stopping
point. Inventory and design are read-only. Without explicit application approval,
do not run `doco init`, create changes, rewrite or move documents, update links, or
delete files. If application is approved but doco is not initialized, obtain
separate approval before initialization and report its integration-file effects.

Start from existing current doco documents when present. Discover candidates from
root READMEs and indexes, documentation directories, Agent instructions, repository
links, and project-specific formats rather than a fixed filename list. Record known
external or inaccessible sources such as wikis; do not claim a complete inventory
of material you could not inspect.

Exclude `.git`, build outputs, dependency/vendor trees, generated documentation and
`doco/tmp/` unless explicitly in scope. Treat completed and archived packages as
non-current history and do not read them by default. Existing architecture, specs
and effective decisions are merge targets and current evidence, never blank files
to overwrite.

## 2. Classify content, not paths

Split mixed files into content units and classify each unit as current, planned,
historical or unknown. A file may split across several destinations, and several
files may merge into one. Names such as `architecture`, `spec`, `ADR`, `RFC` or a
source system's status directory are hints only; verify meaning and authority.

Use this mapping after checking the actual source, tests, configuration and approved
contracts:

| Verified content | Doco destination or disposition |
|---|---|
| Implemented system purpose, technology, module boundaries, dependencies, control/data flow, state and runtime constraints | Merge into `doco/architecture.md`. |
| External protocols, public behavior, persisted formats, compatibility rules or behavior that must remain exact | Split by contract into `doco/specs/<contract>.md`. |
| Important long-lived rationale not evident from code | Move or merge into `doco/decisions/NNNN-<decision>.md`; retain superseded status and do not renumber existing ADRs. |
| A confirmed unfinished goal that will continue | With user approval, use the Create workflow to produce a complete active change; do not copy source status or mark tasks complete without verification. |
| Completed, abandoned or uncertain plans/RFCs | Do not fabricate completed/archived packages. Extract verified current facts or durable rationale, then KEEP or DEFER the source unless the user separately requests historical import. |
| README material, tutorials, runbooks, contribution guides, licenses, changelogs, generated or vendor docs | Keep in the location appropriate to their audience; link to doco when useful instead of forcing them into doco. |

Code shows implemented behavior, while specs state approved contracts. If they
disagree, record a conflict and determine whether the implementation is wrong or a
contract change is intentionally requested; never edit a spec merely to hide the
disagreement. Do not let historical plans override current code or contracts.
Preserve uncertain claims and ask for a decision instead of silently choosing one.

## 3. Produce the migration design

Before applying changes, show the proposed doco tree, merge/split boundaries,
naming and link changes, files proposed for retention or removal, conflicts, risks,
open decisions and validation plan. Include one row per inventoried source or
content range:

| Field | Required detail |
|---|---|
| Source | File and, for mixed files, section or content range. |
| Kind/status | Architecture, contract, decision, active plan, history, user/ops or generated; current, planned, historical or unknown. |
| Authority/evidence | Source, test, configuration, current contract or owner confirmation used to validate it. |
| Target/action | Destination plus KEEP, MERGE, SPLIT, MOVE, CONDENSE, DEFER or REMOVE-AFTER-APPROVAL. |
| Conflicts | Contradictions, naming/ID collisions and unresolved authority. |
| Validation/disposition | Coverage, link checks and final treatment of the source. |

Every inventoried source must have an explicit disposition. Flat collections will
usually MOVE or MERGE; nested documentation often keeps audience-facing sections
while extracting current technical facts; monoliths usually SPLIT by section;
existing ADR sets retain numbering and replacement chains; other change systems
must be mapped by verified real status, not directory names. Duplicate or stale
sources may be condensed or removed only after the canonical target is verified.
Stop after presenting this design unless the user explicitly approved application.

## 4. Apply an approved migration

Recheck source files and relevant code before writing so concurrent changes do not
invalidate the plan. Merge with existing doco content; never replace current facts
wholesale. Preserve the project's language and useful provenance, avoid copying
low-level implementation detail, use stable descriptive spec names, and allocate
new ADR numbers without renumbering old decisions.

Write and review destinations first: architecture, then specs and decisions, then
any explicitly approved active changes through the Create workflow. Update indexes,
entry documents and inbound links only after destination content exists. Remove or
replace an old source with a short navigation document only when its exact treatment
was approved and its migrated content or intentional omission has been verified.
Deletion is never implied by general approval to “migrate docs.” Stop affected work
on new semantic conflicts, unsafe paths, unsupported encoding or concurrent edits;
report what was and was not changed.

## 5. Validate and report

Confirm that each inventory row reached its approved disposition; current doco has
no unresolved future/history leakage; architecture, specs and decisions have distinct
responsibilities; and no duplicate authoritative source remains unintentionally.
Search for old paths and repair valid references. Run the project's Markdown/link
checks where available, run `doco check <id>` for each imported active change, and
review the complete Git diff for omissions and unrelated edits. Report checks not
run, inaccessible sources, retained legacy documents and unresolved conflicts; do
not describe a partial or unverified migration as complete.
