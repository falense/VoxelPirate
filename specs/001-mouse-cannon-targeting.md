# Spec 001 — Mouse cannon targeting

**Status:** accepted (2026-06-14)
**Roadmap:** refines item 4 (cannons / combat feel)

## Summary

Let the player aim cannons by clicking on the sea. A shot is fired **toward
the clicked point** rather than as a fixed perpendicular broadside. The mouse
button selects which side fires, and a gun only fires if it can physically
aim at the click point.

## Player-facing behaviour

- **Left mouse button** fires the **port (left)** broadside.
- **Right mouse button** fires the **starboard (right)** broadside.
- The shot is aimed at the point where the cursor ray meets the sea
  (world `y = 0`). Balls converge on that point and arc to land on it.
- **Fire-only-if-it-bears:** each gun on the selected side fires only when the
  click point lies within that gun's traverse arc. If you left-click but the
  target is off the starboard bow, the port guns can't bear and nothing fires
  (the click is a no-op — no wasted reload).
- No auto-lead and no target lock: the ball goes to *where you clicked*, so the
  player leads moving targets manually. Clicking a moving enemy hits where it
  was, not where it's going.
- Only active in `PlayMode::Sail` (build mode keeps its left/right-click block
  place/remove). Q/E keep firing fixed perpendicular broadsides for keyboard
  play and remain unchanged.

## Design decisions (confirmed with Sondre, 2026-06-14)

- Target model: **shoot where you click** — free aim at the sea point. Not a
  persistent vessel lock, not snap-to-hull.
- Gunnery: **converge on the click point, only fire guns that can bear**;
  left mouse = port, right mouse = starboard.

## Mechanics / ballistics

A broadside gun points perpendicular to the hull. We give it a limited
**traverse arc** so "aim where you click" stays believable instead of a turret
spin:

- `GUN_TRAVERSE` = max yaw (radians) a gun can swing off its perpendicular,
  proposed `60°` (`PI/3`). Horizontal bearing to the click point, measured
  from the gun's side normal, must satisfy `|bearing| <= GUN_TRAVERSE`,
  otherwise that gun does not fire.
- The selected side is determined by the mouse button (port = left,
  starboard = right), **not** by which side the point is on. A gun on the
  selected side still only fires if the point is within its arc — so clicking
  the wrong side's button simply yields no shot.

**Elevation (so the ball lands on the point):** keep launch speed fixed at
`CANNONBALL_SPEED`. Aim the horizontal velocity component along the (clamped)
bearing to the target and solve the projectile equation for the launch
elevation that lands the ball at the target's horizontal distance `d`, given
gravity `GRAVITY` and the muzzle height. Use the low (flat) solution. If `d`
exceeds max ballistic range, clamp to the 45°/max-range shot toward the target
(still fires; just falls short) — this replaces the current fixed
`CANNONBALL_LOFT` heuristic for aimed shots.

## Code changes

- **`src/combat.rs` — `Broadsides`:** carry an optional aim point per side.
  Replace/augment the `fire_port: bool` / `fire_starboard: bool` intents with
  `aim_port: Option<Vec3>` / `aim_starboard: Option<Vec3>` (the world point to
  shoot at). `None` = no fire this frame.
  - To preserve AI/keyboard behaviour without a ballistic target, keep a way
    to request a plain perpendicular volley (e.g. a sentinel, or keep the
    bools alongside the aim points). Enemy AI (`enemy.rs`) and `player_helm`'s
    Q/E path use the perpendicular volley; the mouse path sets the aim point.
- **`src/combat.rs` — `fire_cannons`:** when a side has an aim point, compute
  per-gun bearing, skip guns outside `GUN_TRAVERSE`, and launch toward the
  clamped bearing with the solved elevation. Without an aim point, keep
  today's perpendicular + `CANNONBALL_LOFT` behaviour.
- **`src/ship.rs` — `player_fire_mouse`:** project the cursor to the sea point
  (already done via `cursor_ray`), then on left click set `aim_port` and on
  right click set `aim_starboard` to that point. Remove the old "pick the side
  the point is on" logic — the button now picks the side.
- **`src/enemy.rs`:** update the AI's `fire_port = true` / `fire_starboard =
  true` calls to the perpendicular-volley request shape.

## Out of scope (possible follow-ups)

- Aim reticle / predicted landing marker.
- Target lock onto a specific vessel.
- Auto-lead for moving targets.
- Per-gun reload (reload is still per side).

## Test / verification

- `cargo check` / `cargo clippy` clean.
- Extend `--selftest` (`src/selftest.rs`, uses `AimOverride`) to drive a click
  at an enemy and assert a cannonball spawns heading toward it; assert a click
  on the far side with the wrong button spawns none.
- Manual: balls visibly converge on the clicked point and arc onto a target a
  few blocks away; clicking behind the beam fires nothing.
