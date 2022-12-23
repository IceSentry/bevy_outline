use bevy::prelude::{shape::Cube, *};
use bevy_outline::{BlurredOutlinePlugin, Outline, OutlineSettings};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(BlurredOutlinePlugin)
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
    commands.spawn_bundle(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });
    commands
        .spawn_bundle(Camera3dBundle {
            transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        })
        .insert(OutlineSettings { size: 4.0 });

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
        .insert(RotationAxis(Vec3::X))
        .insert(Outline);

    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Cube { size: 1.0 }.into()),
        material: materials.add(Color::RED.into()),
        transform: Transform::from_xyz(1.15, 0.0, 0.0),
        ..Default::default()
    });
}

#[derive(Clone, Debug, Component)]
struct RotationAxis(Vec3);

fn rotate(time: Res<Time>, mut query: Query<(&mut Transform, &RotationAxis), With<Outline>>) {
    let delta = time.delta_seconds();

    for (mut xform, rot) in query.iter_mut() {
        xform.rotate(Quat::from_axis_angle(rot.0, delta));
    }
}

fn update_outline(mut q: Query<&mut OutlineSettings>, time: Res<Time>) {
    for mut settings in &mut q {
        settings.size = (time.seconds_since_startup().sin() as f32 * 0.5 + 0.5) * 24.0;
    }
}
