@DESIGN.md

# Working conventions for this repo

Organized from things the user has told me while working on PassoFlow. Pending
feature requests (not yet designed/implemented) are tracked in `tasks/todo.md`,
not here — this file is about *how* to work on the project, not the backlog.

## Versioning & docs

- Every release bumps `VERSION`, adds a `CHANGELOG.md` entry (Keep a Changelog
  format), and gets an annotated git tag (`vX.Y.Z`). Both `README.md` and
  `README_ja.md` show the version badge and must stay in sync.
- Action documentation lives in `docs/actions.md` (English, primary) and
  `docs/actions_ja.md` (mirrored, not just translated — keep both accurate).
  Detailed per-action behavior goes in a table, not prose paragraphs.
- Every UI-facing string change touches all three locales in
  `web-ui/src/i18n/translations.ts` (en/ja/zh), never just one.
- New reserved step-level keys (like `note`, `group`, `title`) must be added to
  `_STEP_META_KEYS` in `run_scenario.py` or the validator rejects them as
  "unknown parameter".

## UI/UX preferences

- **Explain non-obvious parameters in the UI itself** (tooltip, placeholder,
  or inline hint), not just in docs — e.g. `set_variable`'s `name`/`value`
  fields confused the user with no in-app guidance.
- **Don't spawn near-duplicate image actions when one parameterized action
  would do.** Prefer the existing image-list and operation parameters, and
  treat any schema change as a migration that needs an explicit compatibility
  plan.
- **Reuse existing interaction patterns for new features when the shape
  matches.** E.g. the user wants a loop/repeat-selection feature built the
  same way group-creation works (select consecutive nodes, one action to
  wrap them), rather than a completely separate UI paradigm.
- **Visual accessibility**: action nodes need a light background tint (not
  plain white/dark-on-dark) so they're legible against the canvas in both
  light and dark mode.

## Process

- Live-verify web UI changes in the browser (not just `tsc`/lint) before
  calling a feature done — several real bugs in this project were only
  caught this way (off-screen dropdown, invisible icon, group-label overlap,
  and the group-rename `nopan`/d3-zoom interaction bug).
- `tsc -b` and `npm run lint` (oxlint) must stay clean; the only accepted
  warnings are the 3 pre-existing `react-hooks/exhaustive-deps`-adjacent
  fast-refresh warnings in context files, unrelated to feature work.
- When the user queues multiple features and says to just proceed through
  them, do the small/low-risk ones first and pause to design bigger,
  schema-breaking changes with them before touching YAML compatibility.
