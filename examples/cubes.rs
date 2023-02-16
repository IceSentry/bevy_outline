use bevy::prelude::{shape::Cube, *};
use bevy_outline::{Outline, OutlinePlugin, OutlineSettings};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(OutlinePlugin)
        .add_startup_system(setup)
        .add_system(rotate)
        .add_system(update_outline)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
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
            size: 1.0,
            intensity: 1.0,
        },
    ));

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cube { size: 1.0 }.into()),
            material: materials.add(Color::RED.into()),
            transform: Transform::from_xyz(-1.25, 0.0, 0.5),
            ..Default::default()
        },
        RotationAxis(Vec3::Y),
        Outline { color: Color::BLUE },
    ));

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cube { size: 1.0 }.into()),
            material: materials.add(Color::RED.into()),
            ..Default::default()
        },
        RotationAxis(Vec3::X),
        Outline {
            color: Color::GREEN,
        },
    ));

    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cube { size: 1.0 }.into()),
            material: materials.add(Color::RED.into()),
            transform: Transform::from_xyz(1.5, 0.0, 0.0),
            ..Default::default()
        },
        RotationAxis(Vec3::Z),
        Outline { color: Color::RED },
    ));
}

#[derive(Clone, Debug, Component)]
struct RotationAxis(Vec3);

fn rotate(time: Res<Time>, mut query: Query<(&mut Transform, &RotationAxis)>) {
    let delta = time.delta_seconds();
    for (mut transform, rot) in query.iter_mut() {
        transform.rotate(Quat::from_axis_angle(rot.0, delta));
    }
}

fn update_outline(mut q: Query<&mut OutlineSettings>, time: Res<Time>) {
    for mut settings in &mut q {
        settings.size = (time.elapsed_seconds_wrapped().sin() * 0.5 + 0.5) * 32.0;
    }
}
