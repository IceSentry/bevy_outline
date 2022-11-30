use bevy::prelude::{shape::Cube, *};
use bevy_outline::plugin::{BlurredOutlinePlugin, Outline};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(BlurredOutlinePlugin)
        .add_startup_system(setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn_bundle(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });
    commands.spawn_bundle(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Cube { size: 1.0 }.into()),
        material: materials.add(Color::RED.into()),
        transform: Transform::from_xyz(-1.15, 0.0, 0.0),
        ..Default::default()
    });

    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Cube { size: 1.0 }.into()),
            material: materials.add(Color::RED.into()),
            ..Default::default()
        })
        .insert(Outline);

    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Cube { size: 1.0 }.into()),
        material: materials.add(Color::RED.into()),
        transform: Transform::from_xyz(1.15, 0.0, 0.0),
        ..Default::default()
    });
}
