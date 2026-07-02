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

## Wakes & combat feedback — parked 2026-07-02
**Blocked work:** none (independent polish item).
**Context:** the last unbuilt graphics upgrade — foam wake + bow spray,
cannonball smoke trails, camera shake on hits. Pure feel; cheap particles.
Recommended next polish step: motion feedback compounds with the swell,
and the sea still looks empty behind a moving ship.

## Resolved

- **Game-flow shape (dock interlude)** — chosen by Sondre 2026-07-02:
  wave-based matches, hull classes as explicit dock purchases (auto-upgrade
  removed), and a dedicated dock scene (calm cove + pier + orbit camera)
  rather than freezing the battle in place. Spec 006.

- **Graphics upgrade menu** — of the four candidates offered 2026-07-02,
  living ocean + physical sky/bloom + PBR materials shipped in spec 004,
  and procedural block textures shipped in spec 005 (Sondre asked for
  textures explicitly). Only wakes & combat feedback remains (above).

- **Scavenging mechanic shape** — Option A (sunk ships scatter floating
  flotsam you sail over to collect), chosen by Sondre 2026-06-13. Option B
  (boarding/stripping disabled ships) remains a possible later evolution now
  that block placement exists.

- **Engine/stack** — Bevy 0.18 (Rust), chosen 2026-06-12 over Godot+GDScript
  and TypeScript+Babylon.js; no-C# constraint, agent-friendly compiler loop.
- **Perspective (3D vs top-down)** — implicitly 3D free camera for now; revisit
  if first-person ship-walking turns out to be wanted early.
