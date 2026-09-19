---
name: next-phase
description: Use when a PixelCAD development phase's PLAN.md has passed every acceptance criterion and it is time to close out that phase and start planning the next one. Reads ROADMAP.md, archives the completed PLAN.md, updates the Roadmap status table, and invokes the Planner profile for the next PENDING phase. Trigger phrases include "close out this phase", "start the next phase", "what's next on the roadmap", or "invoke the Planner for the next phase".
---

# Next Phase

Use this skill at the boundary between two PixelCAD development phases: the
current phase's acceptance criteria all PASS, and it's time to hand off to
the Planner for the next one. This is the automation for step 3 ("Close")
plus the start of step 1 ("Plan") of the Session Protocol described in
`ROADMAP.md`.

## Steps

1. **Read `ROADMAP.md` in full.** Identify the current `ACTIVE` phase and the
   next `PENDING` phase in the Status Log table.
2. **Confirm the current phase is actually done.** Re-check that phase's
   `PLAN.md` (or `PLAN-phase<N>.md` if already archived) acceptance criteria
   are every one PASS, with evidence. Do not skip this even if a prior
   session claimed completion — verify against the repository as it exists
   now.
3. **Archive the completed plan.** Move `PLAN.md` to
   `Plans/archive/PLAN-phase<N>.md`, where `<N>` is the completed phase's
   number from the Roadmap heading (e.g. `PLAN-phase0.md` for
   "Phase 0 — Bootstrap & Command Engine").
4. **Update `ROADMAP.md`'s Status Log table:**
   - Flip the completed phase's row from `ACTIVE` to `COMPLETE`, filling in
     the `Completed` column with the ISO 8601 completion date.
   - Leave the next phase's row as `PENDING` — it only becomes `ACTIVE` once
     the Planner has drafted a `PLAN.md` for it and the maintainer has set
     `Status: LOCKED`, per the Session Protocol. Do not jump ahead of that
     gate.
5. **Carry forward open items.** If `BACKLOG.md` exists and has unresolved
   entries, do not drop them silently — surface them to the maintainer or
   fold relevant ones into the scope discussion for the next `PLAN.md`.
6. **Invoke the Planner profile** (`Profiles/Planner/profile.md`) to draft
   `PLAN.md` for the next `PENDING` phase, using the phase description
   already present in `ROADMAP.md`'s `## Phases` section as the starting
   brief.
7. **Stop and wait.** Per the Session Protocol, a freshly drafted plan needs
   the maintainer to review and lock it (`Status: LOCKED`) before Autopilot
   may execute it. Do not self-lock a plan.

## Constraints

- Never modify `Profiles/Autopilot/**` or `Profiles/Planner/**` — these are
  the agent profiles themselves and are out of scope for every development
  phase, including this handoff.
- Never invent a phase number or plan content not already implied by
  `ROADMAP.md`'s `## Phases` section; if the roadmap is ambiguous about
  what's next, ask the maintainer rather than guessing.
