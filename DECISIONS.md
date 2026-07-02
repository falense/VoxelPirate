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

## Remaining graphics upgrades — parked 2026-07-02
**Blocked work:** none (independent polish items).
**Context:** Sondre asked for richer graphics; of the four candidates, the
living ocean, physical sky/bloom, and per-block PBR materials shipped in
spec 004. The rest were parked when the direction question went unanswered:
**Options:**
- A: Wakes & combat feedback — foam wake + bow spray, cannonball smoke
  trails, camera shake on hits. Pure feel; cheap particles.
- B: Procedural block textures — code-generated wood grain / plank seams /
  canvas weave, pushing toward the Minecraft look (colors alone read flat
  up close).
- C: Stop here and return to gameplay (structural connectivity, real
  buoyancy).
**Recommendation:** A — motion feedback compounds with the new swell, and
the sea still looks empty behind a moving ship.

## Resolved

- **Scavenging mechanic shape** — Option A (sunk ships scatter floating
  flotsam you sail over to collect), chosen by Sondre 2026-06-13. Option B
  (boarding/stripping disabled ships) remains a possible later evolution now
  that block placement exists.

- **Engine/stack** — Bevy 0.18 (Rust), chosen 2026-06-12 over Godot+GDScript
  and TypeScript+Babylon.js; no-C# constraint, agent-friendly compiler loop.
- **Perspective (3D vs top-down)** — implicitly 3D free camera for now; revisit
  if first-person ship-walking turns out to be wanted early.
