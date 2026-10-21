# Sol toolbox — 21 October 2026

## Tatouages

Mandate: SOL_BRIEF.md, explicitly requested by the user on 21 October 2026. Target: branch
`dev/sol-toolbox`, three ordered tasks with a separate attributed commit and a green
`bash scripts/test.sh lint unit` result for each. Builds only through build-safe.sh; no GUI
layer, push or merge. Reader sources, gui-smoke, parity board and third_party are outside scope.
Removing Recents must preserve Tasks History. Delete the transient brief in the last commit.

## Polaroid

Status: closed, all three requested tasks verified. C-security-002 committed as `842decf`;
C-security-004 committed as `95db196`. C-files-005 and this archive are in the final commit,
`feat(recents): remove and clear files without changing history`. No active dependency remains.
No push or merge was performed. The transient SOL_BRIEF.md was removed.

Task 1: core length/class strength rule with Unicode and boundary cases; a Protect meter and
opt-in password reveal controls (mouse, keyboard, accessibility action, remask on empty).
Verdict: GREEN (layers: lint unit; not a full run) (401s).

Task 2: all five permissions default to allowed; the job runner passes flags to AES-256 encryption.
Existing protect callers retain their defaults. Editing covers content and page assembly, printing
covers both quality levels, and accessibility extraction remains allowed. Owner password falls back
to the open password for None or empty. Tests cover defaults, each restriction separately, all
restrictions together, owner fallback and app-job plumbing, using qpdf's encryption report.
Verdict: GREEN (layers: lint unit; not a full run) (881s).

Task 3: remove on card hover/focus and Clear recents, with keyboard and accessibility actions.
Hidden paths live in hidden-recents.json; deleting cards never rewrites history.json. A new open
or completed job restores its outputs. Exclusions expire with history entries. Failed writes leave
the visible state unchanged and produce an error toast. Four store tests check unchanged history
bytes, duplicates and persistence, all entries beyond the 12-card limit, reopening and new activity,
write errors, corrupt state recovery and expired exclusion pruning.
Verdict: GREEN (layers: lint unit; not a full run) (504s).

Environment: this new worktree lacked ignored vendor assets. It now has an ignored symlink to the
existing sibling vendor directory, verified to contain libpdfium.so. The first suite failed for this
missing dependency (1203s); the same suite passed after fixing it. Task 3's first compile failed
because a folder accessibility label lacked a role (59s); role and default action were added.
No forbidden source was edited. No GUI layer was run. The Impeccable mechanical scan returned [].
No visual or interaction validation is claimed; it remains outside this mandate's test allowance.

Proposed grades, function / aesthetics / performance: task 1 = 8 / 8 / 9; task 2 = 8 / 8 / 9;
task 3 = 8 / 8 / 9. The missing evidence for 9 in function and aesthetics is a permitted GUI pass:
password reveal/remask and strength states, permission selections and resulting output, hover and
keyboard removal, clearing and reopening, and light/dark layout at the minimum window size.
Performance grades are proposals from the small bounded operations and successful existing tests,
not a benchmark claim.

Completion audit: mandate fulfilled; verification receipts recorded; no external dependency remains;
all independent authorized work is complete. The linked vendor assets are ignored and unmodified.
The remaining GUI verification is explicitly excluded by the brief, not a claim of a completed
interaction or visual review. Shared Cargo target pruning was not run while another worktree was
using the cache; the prescribed build-safe wrapper handled every build and check in this mission.
