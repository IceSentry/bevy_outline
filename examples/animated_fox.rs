//! Plays animations from a skinned glTF.

use std::f32::consts::PI;

use bevy::prelude::*;
use bevy_outline::{Outline, OutlinePlugin, OutlineSettings, OutlineType};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(OutlinePlugin)
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1.0,
        })
        .add_startup_system(setup)
        .add_system(setup_scene_once_loaded)
        .run();
}

#[derive(Resource)]
struct Animations(Vec<Handle<AnimationClip>>);

#[derive(Component)]
struct Ground;

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Insert a resource with the current scene information
    commands.insert_resource(Animations(vec![
        asset_server.load("Fox.glb#Animation0"),
        asset_server.load("Fox.glb#Animation1"),
        asset_server.load("Fox.glb#Animation2"),
    ]));

    // Camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(100.0, 100.0, 150.0)
                .looking_at(Vec3::new(0.0, 20.0, 0.0), Vec3::Y),
            ..default()
        },
        OutlineSettings {
            size: 32.0,
            intensity: 1.5,
            outline_type: OutlineType::BoxBlur,
        },
    ));

    // Plane
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Plane {
                size: 500000.0,
                subdivisions: 1,
            })),
            material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
            ..default()
        },
        Ground,
    ));

    // Light
    commands.spawn(DirectionalLightBundle {
        transform: Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 1.0, -PI / 4.)),
        directional_light: DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        ..default()
    });

    // Fox
    commands.spawn(SceneBundle {
        scene: asset_server.load("Fox.glb#Scene0"),
        ..default()
    });
}

// Once the scene is loaded, start the animation
fn setup_scene_once_loaded(
    mut commands: Commands,
    animations: Res<Animations>,
    mut player: Query<&mut AnimationPlayer>,
    mut done: Local<bool>,
    mesh: Query<Entity, (With<Handle<Mesh>>, Without<Ground>)>,
) {
    if *done {
        return;
    }

    let Ok(mut player) = player.get_single_mut() else {
        return;
    };

    for e in &mesh {
        commands.entity(e).insert(Outline { color: Color::RED });
    }

    player.play(animations.0[0].clone_weak()).repeat();

    *done = true;
}
