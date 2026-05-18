use bevy::prelude::*;

const DIVIDER_WIDTH: f32 = 2.0;

pub fn my_setup<Rom, Ram, Rim>(
    parent: &mut ChildSpawnerCommands,
    rom: Rom,
    ram: Ram,
    rim: Rim,
    colours: (Color, Color, Color),
)
where
    Rom: FnOnce(&mut ChildSpawnerCommands),
    Ram: FnOnce(&mut ChildSpawnerCommands),
    Rim: FnOnce(&mut ChildSpawnerCommands),
{

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.08)),
        ))
        .with_children(|parent| {
            // Left section
            spawn_section(parent, colours.0, rom);

            // Divider
            spawn_divider(parent);

            // Middle section
            spawn_section(parent, colours.1, ram);

            // Divider
            spawn_divider(parent);

            // Right section
            spawn_section(parent, colours.2, rim);
        });
}

fn spawn_section<And>(parent: &mut ChildSpawnerCommands, color: Color, and: And)
where
    And: FnOnce(&mut ChildSpawnerCommands),
{
    parent.spawn((
        Node {
            height: Val::Percent(100.0),
            flex_grow: 1.0,
            ..default()
        },
        BackgroundColor(color),
    ))
        .with_children(|cs| and(cs));
}

fn spawn_divider(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node {
            width: Val::Px(DIVIDER_WIDTH),
            height: Val::Percent(100.0),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(Color::WHITE),
    ));
}
