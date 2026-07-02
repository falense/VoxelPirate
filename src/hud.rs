use std::f32::consts::PI;

use bevy::prelude::*;

use crate::combat::{Broadsides, GameStats, Sinking};
use crate::enemy::EnemyAi;
use crate::salvage::{Derelict, Flotsam};
use crate::ship::{PLAYER_CLASSES, PlayerShip, ShipVoxels, UPGRADE_COSTS};

#[derive(Component)]
pub struct ControlsText;

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct IntelText;

#[derive(Component)]
pub struct CenterText;

pub fn setup_hud(mut commands: Commands) {
    commands.spawn((
        ControlsText,
        Text::new(""),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(10.0),
            ..default()
        },
    ));
    commands.spawn((
        StatusText,
        Text::new(""),
        TextFont {
            font_size: 17.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(56.0),
            ..default()
        },
    ));
    commands.spawn((
        IntelText,
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(82.0),
            ..default()
        },
    ));
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            top: Val::Percent(38.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                CenterText,
                Text::new(""),
                TextFont {
                    font_size: 30.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.85, 0.4)),
            ));
        });
}

pub fn update_hud(
    time: Res<Time>,
    virtual_time: Res<Time<Virtual>>,
    phase: Res<State<crate::dock::GamePhase>>,
    waves: Res<crate::dock::WaveDirector>,
    mode: Res<crate::build::PlayMode>,
    build_state: Res<crate::build::BuildState>,
    mut stats: ResMut<GameStats>,
    players: Query<(&ShipVoxels, &Broadsides), (With<PlayerShip>, Without<Sinking>)>,
    mut controls: Query<&mut Text, (With<ControlsText>, Without<StatusText>, Without<CenterText>)>,
    mut status: Query<&mut Text, (With<StatusText>, Without<CenterText>, Without<ControlsText>)>,
    mut center: Query<&mut Text, (With<CenterText>, Without<StatusText>, Without<ControlsText>)>,
) {
    stats.announce_ttl = (stats.announce_ttl - time.delta_secs()).max(0.0);

    if let Ok(mut text) = controls.single_mut() {
        let at_dock = *phase.get() == crate::dock::GamePhase::Dock;
        text.0 = if at_dock {
            let def = crate::blocks::def(build_state.selected);
            let upgrade = if stats.tier + 1 < PLAYER_CLASSES.len() {
                format!(
                    "  ·  U: buy {} ({})",
                    PLAYER_CLASSES[stats.tier + 1].name,
                    UPGRADE_COSTS[stats.tier]
                )
            } else {
                String::new()
            };
            format!(
                "AT THE DOCK — 1-0 select block  ·  click place ({} costs {})  ·  right-click remove{upgrade}  ·  R: repair  ·  WASD orbit + scroll  ·  ENTER: set sail",
                def.name, def.cost,
            )
        } else {
            match *mode {
                crate::build::PlayMode::Sail => {
                    "WASD sail  ·  click fires the broadside facing the cursor  ·  scroll zoom  ·  Tab: build mode".into()
                }
                crate::build::PlayMode::Build => {
                    let def = crate::blocks::def(build_state.selected);
                    format!(
                        "BUILD MODE — 1-0 select block  ·  click place ({} costs {})  ·  right-click remove (refund)  ·  Tab: sail",
                        def.name, def.cost,
                    )
                }
            }
        };
    }

    if let Ok(mut text) = status.single_mut() {
        let salvage = format!("Salvage {}", stats.salvage);
        let crown = if stats.victory {
            "   ☠ Dreadnought defeated"
        } else {
            ""
        };
        let wave = match *phase.get() {
            crate::dock::GamePhase::Dock => format!("Wave {} next", waves.wave),
            crate::dock::GamePhase::Battle => format!("Wave {}", waves.wave),
        };
        if let Ok((voxels, guns)) = players.single() {
            let hull = (1.0 - voxels.damage_fraction()) * 100.0;
            text.0 = format!(
                "{}   {wave}   Hull {hull:.0}%   Port {}   Starboard {}   {salvage}   Ships sunk: {}{crown}",
                PLAYER_CLASSES[stats.tier].name,
                reload_label(guns.reload_port),
                reload_label(guns.reload_starboard),
                stats.kills,
            );
        } else {
            text.0 = format!("{wave}   {salvage}   Ships sunk: {}{crown}", stats.kills);
        }
    }
    if let Ok(mut text) = center.single_mut() {
        text.0 = if virtual_time.is_paused() {
            "PAUSED — P to resume".into()
        } else if stats.player_sunk {
            "Your ship is going down!".into()
        } else if stats.announce_ttl > 0.0 {
            stats.announcement.clone()
        } else {
            String::new()
        };
    }
}

fn reload_label(remaining: f32) -> String {
    if remaining <= 0.0 {
        "READY".into()
    } else {
        format!("{remaining:.1}s")
    }
}

/// Lookout's report: clock bearing and range to the nearest foe, derelict,
/// and flotsam, so there's always a heading worth sailing.
pub fn update_intel(
    wind: Res<crate::ocean::Wind>,
    players: Query<&Transform, (With<PlayerShip>, Without<Sinking>)>,
    enemies: Query<&Transform, (With<EnemyAi>, Without<Sinking>)>,
    derelicts: Query<&Transform, (With<Derelict>, Without<Sinking>)>,
    flotsam: Query<&Transform, With<Flotsam>>,
    mut intel: Query<&mut Text, With<IntelText>>,
) {
    let Ok(mut text) = intel.single_mut() else {
        return;
    };
    let Ok(player) = players.single() else {
        text.0 = String::new();
        return;
    };
    let mut parts = vec![format!(
        "Wind blowing toward {} o'clock",
        clock_hours(player, wind.dir())
    )];
    if let Some(report) = nearest_report("Foe", player, enemies.iter()) {
        parts.push(report);
    }
    if let Some(report) = nearest_report("Derelict", player, derelicts.iter()) {
        parts.push(report);
    }
    if let Some(report) = nearest_report("Flotsam", player, flotsam.iter()) {
        parts.push(report);
    }
    text.0 = parts.join("   ·   ");
}

fn nearest_report<'a>(
    label: &str,
    player: &Transform,
    targets: impl Iterator<Item = &'a Transform>,
) -> Option<String> {
    let nearest = targets.min_by(|a, b| {
        let da = a.translation.distance_squared(player.translation);
        let db = b.translation.distance_squared(player.translation);
        da.total_cmp(&db)
    })?;
    let mut to_target = nearest.translation - player.translation;
    to_target.y = 0.0;
    let distance = to_target.length();
    let hours = clock_hours(player, to_target.normalize_or_zero());
    Some(format!("{label} {hours} o'clock, {distance:.0}m"))
}

/// Clock bearing of a direction measured clockwise from the bow:
/// dead ahead = 12, starboard beam = 3.
fn clock_hours(player: &Transform, dir: Vec3) -> i32 {
    let forward = (player.rotation * Vec3::X).with_y(0.0).normalize_or_zero();
    let starboard = (player.rotation * Vec3::Z).with_y(0.0).normalize_or_zero();
    let theta = dir.dot(starboard).atan2(dir.dot(forward));
    let mut hours = (theta / (PI / 6.0)).round() as i32;
    hours = hours.rem_euclid(12);
    if hours == 0 {
        hours = 12;
    }
    hours
}
