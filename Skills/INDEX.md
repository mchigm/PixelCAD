# Skills

AI Skill resources

---

# Directory

```
/Skills
-> INDEX.md
```

Skill implementations live under `.agents/skills/` (Zed's project-local
skill location); this file is the human-readable register of them.

## Registered skills

| Skill | Location | Purpose |
|-------|----------|---------|
| `next-phase` | `.agents/skills/next-phase/SKILL.md` | Closes out a completed development phase (archives `PLAN.md`, updates `ROADMAP.md`'s status table) and invokes the Planner profile for the next `PENDING` phase. |

> This directory is **versioned in git** as of ROADMAP Phase 1 (decision
> "I2"). See `PHASE2_RESULT.md` Section 6 for the reasoning: `ROADMAP.md`'s
> Session Protocol references `Profiles/` by path, so a clean clone must
> contain these files for the documented process to be reproducible.
