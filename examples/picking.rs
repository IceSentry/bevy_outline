use bevy::prelude::{shape::Cube, *};
use bevy_mod_picking::{
    HoverEvent, InteractablePickingPlugin, PickableBundle, PickingCameraBundle, PickingEvent,
    PickingPlugin,
};
use bevy_outline::{Outline, OutlinePlugin, OutlineSettings};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(PickingPlugin)
        .add_plugin(InteractablePickingPlugin)
        .add_plugin(OutlinePlugin)
        .add_startup_system(setup)
        .add_system(handle_picking)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cube { size: 1.0 }.into()),
            material: materials.add(Color::RED.into()),
            transform: Transform::from_xyz(-1.25, 0.0, 0.5),
            ..Default::default()
        },
        PickableBundle::default(),
    ));

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cube { size: 1.0 }.into()),
            material: materials.add(Color::RED.into()),
            ..Default::default()
        },
        PickableBundle::default(),
    ));

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cube { size: 1.0 }.into()),
            material: materials.add(Color::RED.into()),
            transform: Transform::from_xyz(1.5, 0.0, 0.0),
            ..Default::default()
        },
        PickableBundle::default(),
    ));

    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });

    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        OutlineSettings {
            size: 16.0,
            intensity: 1.5,
        },
        PickingCameraBundle::default(),
    ));
}

pub fn handle_picking(mut commands: Commands, mut events: EventReader<PickingEvent>) {
    for event in events.iter() {
        if let PickingEvent::Hover(e) = event {
            match e {
                HoverEvent::JustEntered(e) => {
                    commands.entity(*e).insert(Outline {
                        color: Color::GREEN,
                    });
                }
                HoverEvent::JustLeft(e) => {
                    commands.entity(*e).remove::<Outline>();
                }
            }
        }
    }
}
