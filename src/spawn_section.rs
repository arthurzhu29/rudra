//! The three-column layout shell: Rom | Ram | Rim, separated by thin dividers.
//!
//! This module is purely layout scaffolding — it knows nothing about the
//! document. `ui::build_view` passes one closure per region to fill each
//! column with content.

use bevy::prelude::*;

const DIVIDER_WIDTH: f32 = 2.0;
const DIVIDER_COLOR: Color = Color::srgb(0.22, 0.22, 0.26);
const SECTION_BG: Color = Color::srgb(0.12, 0.12, 0.15);

/// Spawn three side-by-side columns of **equal width** (Rom | Ram | Rim), each
/// filled by the given closure, with thin dividers between them.
///
/// The columns stay exactly equal width regardless of their content:
/// `flex_basis: 0` plus equal `flex_grow` splits the row evenly, `min_width: 0`
/// stops wide content from forcing a column wider, and `overflow: clip` keeps
/// content from spilling past the column boundary.
pub fn spawn_columns<Rom, Ram, Rim>(parent: &mut ChildSpawnerCommands, rom: Rom, ram: Ram, rim: Rim)
where
    Rom: FnOnce(&mut ChildSpawnerCommands),
    Ram: FnOnce(&mut ChildSpawnerCommands),
    Rim: FnOnce(&mut ChildSpawnerCommands),
{
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0, // fill the height left over by the toolbar + status
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|row| {
            spawn_section(row, rom);
            spawn_divider(row);
            spawn_section(row, ram);
            spawn_divider(row);
            spawn_section(row, rim);
        });
}

fn spawn_section<Fill>(parent: &mut ChildSpawnerCommands, fill: Fill)
where
    Fill: FnOnce(&mut ChildSpawnerCommands),
{
    parent
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_basis: Val::Px(0.0),   // ignore content size when sharing width
                flex_grow: 1.0,             // ...so the three columns split evenly
                min_width: Val::Px(0.0),    // allow shrinking below content width
                overflow: Overflow::clip(), // keep content inside the column
                ..default()
            },
            BackgroundColor(SECTION_BG),
        ))
        .with_children(|cs| fill(cs));
}

fn spawn_divider(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node {
            width: Val::Px(DIVIDER_WIDTH),
            height: Val::Percent(100.0),
            flex_shrink: 0.0, // a divider is always exactly DIVIDER_WIDTH wide
            ..default()
        },
        BackgroundColor(DIVIDER_COLOR),
    ));
}
