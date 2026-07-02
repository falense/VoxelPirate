//! The heads-up display: a status panel with hull and reload gauges
//! top-left, the lookout's intel line beneath it, announcements in the
//! center, a contextual key hint bottom-right, and a textured block
//! hotbar along the bottom whenever build mode is live (Spec 007).

use std::f32::consts::PI;

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::blocks;
use crate::combat::{Broadsides, GameStats, Sinking};
use crate::dock::{GamePhase, WaveDirector};
use crate::enemy::EnemyAi;
use crate::salvage::{Derelict, Flotsam};
use crate::ship::{PLAYER_CLASSES, PlayerShip, ShipVoxels, UPGRADE_COSTS};

/// Text elements the HUD updater writes each frame, found by role.
#[derive(Component, PartialEq, Eq)]
pub enum HudText {
    /// Ship class and wave, top of the status panel.
    Title,
    /// Salvage / kills line under the title.
    Status,
    /// Big center announcements (wave banners, pause, victory).
    Center,
    /// Contextual key hints, bottom-right.
    Hint,
    /// Selected block name and cost above the hotbar.
    HotbarLabel,
}

/// Gauge fill bars, found by role; the updater drives width and color.
#[derive(Component)]
pub enum HudBar {
    Hull,
    Port,
    Starboard,
}

#[derive(Component)]
pub struct IntelText;

/// Root node of the block hotbar; hidden outside build mode.
#[derive(Component)]
pub struct HotbarRoot;

/// One hotbar slot, indexed into [`blocks::ALL`].
#[derive(Component)]
pub struct HotbarSlot(pub usize);

const PANEL_BG: Color = Color::srgba(0.02, 0.05, 0.09, 0.62);
const TEXT_DIM: Color = Color::srgba(1.0, 1.0, 1.0, 0.66);
const GOLD: Color = Color::srgb(1.0, 0.85, 0.4);
const TRACK_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.12);

pub fn setup_hud(mut commands: Commands, assets: Res<GameAssets>) {
    // Status panel: title, status line, and the three gauges.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|panel| {
            panel.spawn((
                HudText::Title,
                Text::new(""),
                TextFont {
                    font_size: 17.0,
                    ..default()
                },
                TextColor(GOLD),
            ));
            panel.spawn((
                HudText::Status,
                Text::new(""),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            for (label, role) in [
                ("HULL", HudBar::Hull),
                ("PORT", HudBar::Port),
                ("STBD", HudBar::Starboard),
            ] {
                panel
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(TEXT_DIM),
                            Node {
                                width: Val::Px(34.0),
                                ..default()
                            },
                        ));
                        row.spawn((
                            Node {
                                width: Val::Px(150.0),
                                height: Val::Px(9.0),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(TRACK_BG),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                role,
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    border_radius: BorderRadius::all(Val::Px(3.0)),
                                    ..default()
                                },
                                BackgroundColor(GOLD),
                            ));
                        });
                    });
            }
        });

    // Lookout's intel line, under the panel.
    commands.spawn((
        IntelText,
        Text::new(""),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(TEXT_DIM),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            top: Val::Px(136.0),
            ..default()
        },
    ));

    // Center announcements.
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
                HudText::Center,
                Text::new(""),
                TextFont {
                    font_size: 30.0,
                    ..default()
                },
                TextColor(GOLD),
            ));
        });

    // Contextual key hints, bottom-right.
    commands.spawn((
        HudText::Hint,
        Text::new(""),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(TEXT_DIM),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(12.0),
            bottom: Val::Px(10.0),
            ..default()
        },
    ));

    // Block hotbar, bottom-center: a slot per selectable block (digits 1-0)
    // with the block's actual texture tile as its icon.
    commands
        .spawn((
            HotbarRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                bottom: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(5.0),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                HudText::HotbarLabel,
                Text::new(""),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(5.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ))
            .with_children(|bar| {
                for (index, id) in blocks::ALL.into_iter().take(10).enumerate() {
                    bar.spawn((
                        HotbarSlot(index),
                        Node {
                            width: Val::Px(42.0),
                            height: Val::Px(42.0),
                            border: UiRect::all(Val::Px(2.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
                        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.15)),
                    ))
                    .with_children(|slot| {
                        slot.spawn((
                            ImageNode::new(assets.block_tiles[&id].clone()),
                            Node {
                                width: Val::Px(30.0),
                                height: Val::Px(30.0),
                                ..default()
                            },
                        ));
                        slot.spawn((
                            Text::new(format!("{}", (index + 1) % 10)),
                            TextFont {
                                font_size: 10.0,
                                ..default()
                            },
                            TextColor(TEXT_DIM),
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(1.0),
                                left: Val::Px(4.0),
                                ..default()
                            },
                        ));
                    });
                }
            });
        });
}

pub fn update_hud(
    time: Res<Time>,
    virtual_time: Res<Time<Virtual>>,
    phase: Res<State<GamePhase>>,
    waves: Res<WaveDirector>,
    mode: Res<crate::build::PlayMode>,
    build_state: Res<crate::build::BuildState>,
    mut stats: ResMut<GameStats>,
    players: Query<(&ShipVoxels, &Broadsides), (With<PlayerShip>, Without<Sinking>)>,
    mut texts: Query<(&mut Text, &HudText)>,
    mut bars: Query<(&mut Node, &mut BackgroundColor, &HudBar)>,
    mut hotbar: Query<&mut Visibility, With<HotbarRoot>>,
    mut slots: Query<(&HotbarSlot, &mut BorderColor)>,
) {
    stats.announce_ttl = (stats.announce_ttl - time.delta_secs()).max(0.0);
    let at_dock = *phase.get() == GamePhase::Dock;
    let building = *mode == crate::build::PlayMode::Build;
    let player = players.single().ok();

    for (mut text, role) in &mut texts {
        text.0 = match role {
            HudText::Title => {
                let class = PLAYER_CLASSES[stats.tier].name;
                if at_dock {
                    format!("{class}  |  Wave {} next", waves.wave)
                } else {
                    format!("{class}  |  Wave {}", waves.wave)
                }
            }
            HudText::Status => {
                let crown = if stats.victory {
                    "   *  the seas are yours"
                } else {
                    ""
                };
                format!(
                    "Salvage {}   |   Ships sunk {}{crown}",
                    stats.salvage, stats.kills
                )
            }
            HudText::Center => {
                if virtual_time.is_paused() {
                    "PAUSED: P to resume".into()
                } else if stats.player_sunk {
                    "Your ship is going down!".into()
                } else if stats.announce_ttl > 0.0 {
                    stats.announcement.clone()
                } else {
                    String::new()
                }
            }
            HudText::Hint => {
                if at_dock {
                    let upgrade = if stats.tier + 1 < PLAYER_CLASSES.len() {
                        format!(
                            "U buy {} ({})   ",
                            PLAYER_CLASSES[stats.tier + 1].name,
                            UPGRADE_COSTS[stats.tier]
                        )
                    } else {
                        String::new()
                    };
                    // Short: the hotbar sits center-bottom beside this hint.
                    format!("R repair   {upgrade}WASD orbit   ENTER set sail")
                } else if building {
                    "click place   right-click remove   TAB back to the helm".into()
                } else {
                    "WASD sail   click fires toward the cursor   scroll zoom   TAB build".into()
                }
            }
            HudText::HotbarLabel => {
                let def = blocks::def(build_state.selected);
                format!("{}  |  {} salvage", def.name, def.cost)
            }
        };
    }

    for (mut node, mut color, role) in &mut bars {
        let (fraction, fill) = match role {
            HudBar::Hull => {
                let hull = player
                    .map(|(voxels, _)| 1.0 - voxels.damage_fraction())
                    .unwrap_or(0.0);
                (
                    hull,
                    Color::srgb(1.0 - hull * 0.75, 0.25 + hull * 0.55, 0.25),
                )
            }
            HudBar::Port => {
                reload_bar(player.map(|(_, guns)| (guns.reload_port, guns.reload_time)))
            }
            HudBar::Starboard => {
                reload_bar(player.map(|(_, guns)| (guns.reload_starboard, guns.reload_time)))
            }
        };
        node.width = Val::Percent(fraction * 100.0);
        color.0 = fill;
    }

    if let Ok(mut visibility) = hotbar.single_mut() {
        *visibility = if building {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    let selected = blocks::ALL
        .iter()
        .position(|id| *id == build_state.selected)
        .unwrap_or(0);
    for (slot, mut border) in &mut slots {
        *border = if slot.0 == selected {
            BorderColor::all(GOLD)
        } else {
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.15))
        };
    }
}

/// Reload gauge: fills as the battery reloads, gold when ready to fire.
fn reload_bar(state: Option<(f32, f32)>) -> (f32, Color) {
    let Some((remaining, reload_time)) = state else {
        return (0.0, TRACK_BG);
    };
    let fraction = 1.0 - (remaining / reload_time.max(0.001)).clamp(0.0, 1.0);
    if fraction >= 1.0 {
        (1.0, GOLD)
    } else {
        (fraction, Color::srgba(0.75, 0.7, 0.55, 0.8))
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
    text.0 = parts.join("   |   ");
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
