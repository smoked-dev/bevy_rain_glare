use bevy::{
    core_pipeline::{bloom::BloomSettings, tonemapping::Tonemapping},
    prelude::*,
};
use rain_glare::{RainGlarePlugin, RainGlareSettings};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RainGlarePlugin)
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (
                spin_highlights,
                tweak_rain_glare_settings,
                update_hud_text.after(tweak_rain_glare_settings),
            ),
        )
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(AmbientLight {
        color: Color::Srgba(Srgba::new(0.02, 0.02, 0.05, 1.0)),
        brightness: 0.35,
    });

    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..default()
            },
            tonemapping: Tonemapping::TonyMcMapface,
            transform: Transform::from_xyz(-6.5, 5.5, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        BloomSettings::NATURAL,
        RainGlareSettings {
            intensity: 0.25,
            threshold: 0.45,
            streak_length_px: 10.0,
            rain_density: 3.6,
            wind: Vec2::new(0., -1.0),
            speed: 19.4,

            pattern_scale: 1.0,        // 3x smaller mask features
            mask_thickness_px: 0.65,   // thinner, sharper
            snap_to_pixel: 1.0,        // crunchy edges
            tail_quant_steps: 8.0,     // stepped intensity (retro banding)
            ..default()
        }
    ));

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 8_000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(3.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::default().mesh().size(30.0, 30.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.04, 0.08),
            perceptual_roughness: 0.8,
            metallic: 0.1,
            ..default()
        }),
        ..default()
    });

    let bright_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.35, 0.1),
        emissive: LinearRgba::rgb(10.0, 6.0, 1.4),
        metallic: 0.7,
        perceptual_roughness: 0.35,
        ..default()
    });
    let bright_cyan = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.5, 0.6),
        emissive: LinearRgba::rgb(3.5, 10.0, 10.0),
        metallic: 0.55,
        perceptual_roughness: 0.35,
        ..default()
    });
    let bright_magenta = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.1, 0.4),
        emissive: LinearRgba::rgb(10.0, 2.0, 8.0),
        metallic: 0.5,
        perceptual_roughness: 0.35,
        ..default()
    });

    let sphere_mesh = meshes.add(Sphere::new(0.8).mesh().ico(5).unwrap());
    let torus_mesh = meshes.add(Torus::new(1.1, 0.25));

    commands.spawn((
        PbrBundle {
            mesh: sphere_mesh.clone(),
            material: bright_gold.clone(),
            transform: Transform::from_xyz(-2.5, 1.2, 0.0),
            ..default()
        },
        HighlightSpin {
            speed: Vec3::new(0.3, 0.7, 0.25),
        },
    ));
    commands.spawn((
        PbrBundle {
            mesh: torus_mesh,
            material: bright_cyan.clone(),
            transform: Transform::from_xyz(2.5, 1.8, 1.5),
            ..default()
        },
        HighlightSpin {
            speed: Vec3::new(0.55, 0.25, 0.35),
        },
    ));
    commands.spawn((
        PbrBundle {
            mesh: sphere_mesh,
            material: bright_magenta.clone(),
            transform: Transform::from_xyz(0.5, 1.4, -2.5),
            ..default()
        },
        HighlightSpin {
            speed: Vec3::new(0.45, 0.4, 0.6),
        },
    ));

    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1800.0,
            range: 25.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_translation(Vec3::new(0.0, 3.5, 0.0)),
        ..default()
    });

    commands.spawn((
        TextBundle::from_section(
            "",
            TextStyle {
                font_size: 16.0,
                color: Color::WHITE,
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        }),
        HudText,
    ));
}

#[derive(Component)]
struct HighlightSpin {
    speed: Vec3,
}

fn spin_highlights(time: Res<Time>, mut query: Query<(&HighlightSpin, &mut Transform)>) {
    for (spin, mut transform) in &mut query {
        let delta = time.delta_seconds();
        transform.rotate_x(spin.speed.x * delta);
        transform.rotate_y(spin.speed.y * delta);
        transform.rotate_z(spin.speed.z * delta);
    }
}

#[derive(Component)]
struct HudText;

fn tweak_rain_glare_settings(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: Query<&mut RainGlareSettings>,
) {
    let dt = time.delta_seconds();
    for mut s in &mut settings {
        let mut intensity = s.intensity;
        let mut threshold = s.threshold;
        let mut streak_px = s.streak_length_px;
        let mut density = s.rain_density;
        let mut wind = s.wind;
        let mut speed = s.speed;

        let step_small = 0.6 * dt;
        let step_large = 60.0 * dt;

        if keys.pressed(KeyCode::KeyQ) {
            intensity += step_small;
        }
        if keys.pressed(KeyCode::KeyA) {
            intensity -= step_small;
        }

        if keys.pressed(KeyCode::KeyW) {
            threshold += step_small;
        }
        if keys.pressed(KeyCode::KeyS) {
            threshold -= step_small;
        }

        if keys.pressed(KeyCode::KeyE) {
            streak_px += step_large;
        }
        if keys.pressed(KeyCode::KeyD) {
            streak_px -= step_large;
        }

        if keys.pressed(KeyCode::KeyR) {
            density += step_small;
        }
        if keys.pressed(KeyCode::KeyF) {
            density -= step_small;
        }

        if keys.pressed(KeyCode::KeyT) {
            wind.x += step_small;
        }
        if keys.pressed(KeyCode::KeyG) {
            wind.x -= step_small;
        }
        if keys.pressed(KeyCode::KeyY) {
            wind.y += step_small;
        }
        if keys.pressed(KeyCode::KeyH) {
            wind.y -= step_small;
        }

        if keys.pressed(KeyCode::KeyU) {
            speed += step_small;
        }
        if keys.pressed(KeyCode::KeyJ) {
            speed -= step_small;
        }

        s.intensity = intensity.clamp(0.0, 4.0);
        s.threshold = threshold.clamp(0.0, 4.0);
        s.streak_length_px = streak_px.clamp(1.0, 400.0);
        s.rain_density = density.clamp(0.0, 10.0);
        s.wind = Vec2::new(wind.x.clamp(-3.0, 3.0), wind.y.clamp(-3.0, 3.0));
        s.speed = speed.clamp(0.0, 20.0);
    }
}

fn update_hud_text(settings: Query<&RainGlareSettings>, mut text: Query<&mut Text, With<HudText>>) {
    let Ok(s) = settings.get_single() else {
        return;
    };
    let Ok(mut text) = text.get_single_mut() else {
        return;
    };

    text.sections[0].value = format!(
        "\
Rain glare controls:
Q/A intensity   {:.2}
W/S threshold   {:.2}
E/D streak px   {:.1}
R/F density     {:.2}
T/G wind.x      {:.2}
Y/H wind.y      {:.2}
U/J speed       {:.2}
",
        s.intensity, s.threshold, s.streak_length_px, s.rain_density, s.wind.x, s.wind.y, s.speed
    );
}
