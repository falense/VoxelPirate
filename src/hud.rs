use bevy::prelude::*;

use crate::combat::{Broadsides, GameStats, Sinking};
use crate::ship::{PlayerShip, ShipVoxels};

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct CenterText;

pub fn setup_hud(mut commands: Commands) {
    commands.spawn((
        Text::new("WASD sail  ·  Q / E fire port / starboard broadside"),
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
    stats: Res<GameStats>,
    players: Query<(&ShipVoxels, &Broadsides), (With<PlayerShip>, Without<Sinking>)>,
    mut status: Query<&mut Text, (With<StatusText>, Without<CenterText>)>,
    mut center: Query<&mut Text, (With<CenterText>, Without<StatusText>)>,
) {
    if let Ok(mut text) = status.single_mut() {
        if let Ok((voxels, guns)) = players.single() {
            let hull = voxels.blocks.len() as f32 / voxels.initial_count as f32 * 100.0;
            text.0 = format!(
                "Hull {hull:.0}%   Port {}   Starboard {}   Ships sunk: {}",
                reload_label(guns.reload_port),
                reload_label(guns.reload_starboard),
                stats.kills,
            );
        } else {
            text.0 = format!("Ships sunk: {}", stats.kills);
        }
    }
    if let Ok(mut text) = center.single_mut() {
        text.0 = if stats.player_sunk {
            "Your ship went down!  Press R to set sail again".into()
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
