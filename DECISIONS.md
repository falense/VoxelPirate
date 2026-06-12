# Decision queue

Parked preference decisions awaiting Sondre's input (see "Decision protocol"
in AGENTS.md). Newest first. When a decision is made, move it to the Resolved
section with the outcome.

Format per entry:

```
## <short title> — parked <date>
**Blocked work:** <what can't proceed>
**Options:**
- A: <option> — <trade-off>
- B: <option> — <trade-off>
**Recommendation:** <which and why>
```

## Open

## Scavenging mechanic shape — parked 2026-06-12
**Blocked work:** Roadmap item 5 (grow your ship by scavenging defeated vessels)
**Options:**
- A: Sunk ships scatter floating loot blocks you sail over to collect —
  simple, keeps the action at sea, works with today's systems
- B: Ships get a "disabled" damage state (guns gone but afloat) and you pull
  alongside to strip them block-by-block — richer and more piratey, but needs
  block placement (roadmap item 3) and a build cursor first
**Recommendation:** A now, evolve toward B once block placement lands.

## Resolved

- **Engine/stack** — Bevy 0.18 (Rust), chosen 2026-06-12 over Godot+GDScript
  and TypeScript+Babylon.js; no-C# constraint, agent-friendly compiler loop.
- **Perspective (3D vs top-down)** — implicitly 3D free camera for now; revisit
  if first-person ship-walking turns out to be wanted early.
