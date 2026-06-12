use std::f32::consts::PI;

use bevy::prelude::*;

use crate::combat::{Broadsides, GameStats, Sinking};
use crate::enemy::EnemyAi;
use crate::salvage::{Derelict, Flotsam};
use crate::ship::{PLAYER_CLASSES, PlayerShip, ShipVoxels, UPGRADE_COSTS};

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct IntelText;

#[derive(Component)]
pub struct CenterText;

pub fn setup_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("WASD sail  ·  click to fire the broadside facing the cursor  ·  scroll to zoom"),
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
            top: Val::Px(34.0),
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
            top: Val::Px(58.0),
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
    mut stats: ResMut<GameStats>,
    players: Query<(&ShipVoxels, &Broadsides), (With<PlayerShip>, Without<Sinking>)>,
    mut status: Query<&mut Text, (With<StatusText>, Without<CenterText>)>,
    mut center: Query<&mut Text, (With<CenterText>, Without<StatusText>)>,
) {
    stats.announce_ttl = (stats.announce_ttl - time.delta_secs()).max(0.0);

    if let Ok(mut text) = status.single_mut() {
        let salvage = if stats.tier < UPGRADE_COSTS.len() {
            format!(
                "Salvage {}/{} ({} next)",
                stats.salvage,
                UPGRADE_COSTS[stats.tier],
                PLAYER_CLASSES[stats.tier + 1].name,
            )
        } else {
            format!("Salvage {}", stats.salvage)
        };
        if let Ok((voxels, guns)) = players.single() {
            let hull = voxels.blocks.len() as f32 / voxels.initial_count as f32 * 100.0;
            text.0 = format!(
                "{}   Hull {hull:.0}%   Port {}   Starboard {}   {salvage}   Ships sunk: {}",
                PLAYER_CLASSES[stats.tier].name,
                reload_label(guns.reload_port),
                reload_label(guns.reload_starboard),
                stats.kills,
            );
        } else {
            text.0 = format!("{salvage}   Ships sunk: {}", stats.kills);
        }
    }
    if let Ok(mut text) = center.single_mut() {
        text.0 = if stats.player_sunk {
            "Your ship went down!  Press R to set sail again".into()
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
    let mut parts = Vec::new();
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
    let dir = to_target.normalize_or_zero();
    let forward = (player.rotation * Vec3::X).with_y(0.0).normalize_or_zero();
    let starboard = (player.rotation * Vec3::Z).with_y(0.0).normalize_or_zero();
    // Clock bearing measured clockwise from the bow: starboard beam = 3.
    let theta = dir.dot(starboard).atan2(dir.dot(forward));
    let mut hours = (theta / (PI / 6.0)).round() as i32;
    hours = hours.rem_euclid(12);
    if hours == 0 {
        hours = 12;
    }
    Some(format!("{label} {hours} o'clock, {distance:.0}m"))
}
