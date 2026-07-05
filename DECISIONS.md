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

## Salvage economy rebalance — parked 2026-07-05
**Blocked work:** economy tuning; the tier ramp past the brig is grindy.
**Context:** Sondre reports salvage is too scarce to cover both upkeep and
upgrades. Numbers confirm it: wreck loot is capped at 10 pieces + 1 gold
chest regardless of ship size (`start_sinking`, combat.rs), so income stays
~35–75/wave while upgrades cost 20/60/140 and dock repairs charge full block
cost (an iron refit can eat a whole wave's income). At-sea flotsam pickup
also repairs before it banks, so damage silently consumes loot.
**Options:**
- A: Loot scales with wreck value — extra plunder chests proportional to the
  sunk ship's total block value. Income ramps with wave difficulty; repairs
  stay a meaningful cost to weigh against upgrades.
- B: Cheap upkeep — dock repairs free or half price; loot stays scarce, so
  upgrades remain slow, deliberate purchases.
- C: Both, gently — mild loot scaling plus half-price repairs; most
  forgiving, economy mostly stops being a constraint.
- D: Cheaper upgrades — flatten the curve (e.g. 20/45/90) so each tier lands
  after ~1.5 waves of decent play.
**Recommendation:** A — it fixes the structural flaw (flat income vs rising
costs) while keeping damage economically meaningful, and it rewards hunting
the bigger ships.
(The related flotsam-despawn leak — loot vanishing at 120 s before the
wave-clear sweep could bank it — was a dominated fix and shipped separately.)

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
