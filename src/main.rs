mod collision;

use crate::collision::*;
use std::f32::consts::FRAC_PI_2;

use bevy::{
    camera::visibility::RenderLayers,
    //Freecam
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::css::GOLD,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::mouse::AccumulatedMouseMotion,
    light::NotShadowCaster,
    prelude::*,
};
use bevy_window::{CursorGrabMode, CursorOptions, Window};

fn hello_world() {
    //println!("hello world!");
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            FreeCameraPlugin,
            FrameTimeDiagnosticsPlugin::default(),
        ))
        .add_systems(
            Startup,
            (spawn_freecam, (setup_scene, spawn_view_model).chain()),
        )
        .add_systems(FixedUpdate, (advance_physics, update_collision).chain())
        .add_systems(
            RunFixedMainLoop,
            (
                (
                    //Move the Camera
                    move_camera,
                )
                    .chain()
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
                (
                    //Move Player
                    move_player,
                    //Move it out of collided objects
                    interpolate_rendered_transform,
                )
                    .chain()
                    .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
            ),
        )
        .add_systems(Update, (update_window, hello_world, update_text))
        .run();
}

#[derive(Debug, Component)]
struct Player;

#[derive(Component)]
struct PlayerInfo {
    pub keybinds: Keybinds,
    pub pitch: f32,
    pub yaw: f32,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub speed_mult: f32,
    pub friction: f32,
    pub on_ground: bool,
    pub win_focused: bool,
    pub in_wall: bool,
}

/// A vector representing the player's velocity in the physics simulation.
#[derive(Debug, Component, Clone, Copy, PartialEq, Default, Deref, DerefMut)]
struct Velocity(Vec3);

#[derive(Debug, Component, Clone, Copy, PartialEq, Default, Deref, DerefMut)]
struct VelocityJump(Vec3);

/// A vector representing the player's velocity in the physics simulation.
#[derive(Debug, Component, Clone, Copy, PartialEq, Default, Deref, DerefMut)]
struct Acceleration(Vec3);

/// The actual position of the player in the physics simulation.
/// This is separate from the `Transform`, which is merely a visual representation.
///
/// If you want to make sure that this component is always initialized
/// with the same value as the `Transform`'s translation, you can
/// use a [component lifecycle hook](https://docs.rs/bevy/latest/bevy/ecs/component/struct.ComponentHooks.html)
#[derive(Debug, Component, Clone, Copy, PartialEq, Default, Deref, DerefMut)]
struct PhysicalTranslation(Vec3);

/// The value [`PhysicalTranslation`] had in the last fixed timestep.
/// Used for interpolation in the `interpolate_rendered_transform` system.
#[derive(Debug, Component, Clone, Copy, PartialEq, Default, Deref, DerefMut)]
struct PreviousPhysicalTranslation(Vec3);


#[derive(Component)]
struct Keybinds {
    /// Multiplier for pitch and yaw rotation speed.
    pub sensitivity: Vec2,
    /// [`KeyCode`] for forward translation.
    pub key_forward: KeyCode,
    /// [`KeyCode`] for backward translation.
    pub key_back: KeyCode,
    /// [`KeyCode`] for left translation.
    pub key_left: KeyCode,
    /// [`KeyCode`] for right translation.
    pub key_right: KeyCode,

    pub key_run: KeyCode,

    pub key_jump: KeyCode,

    pub key_pause: KeyCode,
}

#[derive(Component)]
struct FPSText;

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            sensitivity: Vec2::new(0.003, 0.002),
            key_forward: KeyCode::KeyW,
            key_back: KeyCode::KeyS,
            key_left: KeyCode::KeyA,
            key_right: KeyCode::KeyD,
            key_run: KeyCode::ShiftLeft,
            key_jump: KeyCode::Space,
            key_pause: KeyCode::Escape,
        }
    }
}

impl Default for PlayerInfo {
    fn default() -> Self {
        Self {
            keybinds: Keybinds::default(),
            pitch: 0.0,
            yaw: 0.0,
            walk_speed: 2.0,
            run_speed: 5.0,
            speed_mult: 1.0,
            friction: 20.0,
            on_ground: false,
            win_focused: true,
            in_wall: false,
        }
    }
}

#[derive(Debug, Component)]
struct WorldModelCamera;

const DEFAULT_RENDER_LAYER: usize = 0;

const VIEW_MODEL_RENDER_LAYER: usize = 1;

const JUMP_SPEED: f32 = 2.0;
const GRAVITY: f32 = 5.0;

fn spawn_view_model(mut commands: Commands) {
    commands.spawn((
        Player,
        PlayerInfo::default(),
        Transform::from_xyz(0.0, 10.0, 0.0),
        AABB::new(Vec3::new(-0.25, -0.5, -0.25), Vec3::new(0.25, 0.5, 0.25)),
        Visibility::default(),
                Velocity::default(),
                VelocityJump::default(),
                Acceleration::default(),
        PhysicalTranslation::default(),
        PreviousPhysicalTranslation::default(),
        children![
            (
                WorldModelCamera,
                Camera3d::default(),
                Projection::from(PerspectiveProjection {
                    fov: 90.0_f32.to_radians(),
                    ..default()
                }),
            ),
            // Spawn view model camera.
            (
                Camera3d::default(),
                Camera {
                    // Bump the order to render on top of the world model.
                    order: 1,
                    ..default()
                },
                Projection::from(PerspectiveProjection {
                    fov: 70.0_f32.to_radians(),
                    ..default()
                }),
                // Only render objects belonging to the view model.
                RenderLayers::layer(VIEW_MODEL_RENDER_LAYER),
            )
        ],
    ));
}

fn move_player(
    time: Res<Time<Real>>,
    //touch_input: Res<Touches>,
    //mouse_button_input: Res<ButtonInput<MouseButton>>,
    key_input: Res<ButtonInput<KeyCode>>,
    //mut toggle_cursor_grab: Local<bool>,
    //mut mouse_cursor_grab: Local<bool>,
    player: Single<(&mut Transform, &mut PlayerInfo, &mut Velocity, &mut VelocityJump, &mut Acceleration), With<Player>>,
) {
    let dt = time.delta_secs();

    let (mut transform, mut player_info, mut velocity, mut velocity_jump, mut acceleration) = player.into_inner();

    let player_info = &mut *player_info;
    let keybinds = &mut player_info.keybinds;

    let mut axis_input = Vec3::ZERO;
    if key_input.pressed(keybinds.key_forward) {
        axis_input.z += 1.0;
    }
    if key_input.pressed(keybinds.key_back) {
        axis_input.z -= 1.0;
    }
    if key_input.pressed(keybinds.key_right) {
        axis_input.x += 1.0;
    }
    if key_input.pressed(keybinds.key_left) {
        axis_input.x -= 1.0;
    }

    // Update velocity

    let run_speed = player_info.run_speed;
    let walk_speed = player_info.walk_speed;

    let speed_mult = player_info.speed_mult;
    let friction = player_info.friction;

    if axis_input != Vec3::ZERO {
        let max_speed = if key_input.pressed(keybinds.key_run) {
            run_speed * speed_mult
        } else {
            walk_speed * speed_mult
        };
        velocity.0 = axis_input.normalize() * max_speed;
    } else {
        let friction = friction.clamp(0.0, f32::MAX);
        velocity.0.smooth_nudge(&Vec3::ZERO, friction, dt);
        if velocity.0.length_squared() < 1e-6 {
            velocity.0 = Vec3::ZERO;
        }
    }

    if player_info.on_ground && key_input.just_pressed(keybinds.key_jump) {
        velocity_jump.0.y = JUMP_SPEED;
        player_info.on_ground = false;
    } else {
        if !player_info.on_ground {
            acceleration.0.y = -GRAVITY;
        } else {
            acceleration.0.y = 0.0;
        }
    }

    // Apply movement update
    let mut forward = *transform.forward();
    forward.y = 0.0;
    forward = forward.normalize_or_zero();

    let mut right = *transform.right();
    right.y = 0.0;
    right = right.normalize_or_zero();

    // Gravity handles vertical movement.
    let up = Vec3::Y;

    velocity.0 = velocity.0.x * right + velocity.0.y * up + velocity.0.z * forward;
}

fn move_camera(
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    player: Single<(&mut Transform, &mut PlayerInfo), With<Player>>,
) {
    let (mut transform, mut player_info) = player.into_inner();

    let player_info = &mut *player_info;
    let keybinds = &mut player_info.keybinds;

    let delta = accumulated_mouse_motion.delta;

    if delta != Vec2::ZERO && player_info.win_focused {
        let delta_yaw = -delta.x * keybinds.sensitivity.x;
        let delta_pitch = -delta.y * keybinds.sensitivity.y;

        let (yawl, pitchl, rolll) = transform.rotation.to_euler(EulerRot::YXZ);
        let yawl = yawl + delta_yaw;

        const PITCH_LIMIT: f32 = FRAC_PI_2 - 0.01;
        let pitchl = (pitchl + delta_pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);

        player_info.yaw = yawl;
        player_info.pitch = pitchl;

        transform.rotation = Quat::from_euler(EulerRot::YXZ, yawl, pitchl, rolll);
    }
}

fn advance_physics(
    fixed_time: Res<Time<Fixed>>,
    mut query: Query<(
        &mut PhysicalTranslation,
        &mut PreviousPhysicalTranslation,
        &mut Velocity,
        &mut VelocityJump,
        &Acceleration,
    )>,
) {
    for (mut current_physical_translation, mut previous_physical_translation, mut velocity, mut velocity_jump, acceleration) in
        query.iter_mut()
    {
        previous_physical_translation.0 = current_physical_translation.0;
        velocity.0 += acceleration.0 * fixed_time.delta_secs();
        velocity_jump.0 += acceleration.0 * fixed_time.delta_secs();
        current_physical_translation.0 += velocity.0 * fixed_time.delta_secs();
        current_physical_translation.0 += velocity_jump.0 * fixed_time.delta_secs();
    }
}

fn interpolate_rendered_transform(
    fixed_time: Res<Time<Fixed>>,
    mut query: Query<(
        &mut Transform,
        &PhysicalTranslation,
        &PreviousPhysicalTranslation,
    )>,
) {
    for (mut transform, current_physical_translation, previous_physical_translation) in
        query.iter_mut()
    {
        let previous = previous_physical_translation.0;
        let current = current_physical_translation.0;
        let alpha = fixed_time.overstep_fraction();

        let rendered_translation = previous.lerp(current, alpha);
        transform.translation = rendered_translation;
    }
}


fn update_window(
    mut windows: Query<(&Window, &mut CursorOptions)>,
    key_input: Res<ButtonInput<KeyCode>>,
    player: Single<(&mut Transform, &mut PlayerInfo), With<Player>>,
) {
    let (mut transform, mut player_info) = player.into_inner();

    let player_info = &mut *player_info;
    let keybinds = &mut player_info.keybinds;
    if key_input.just_pressed(keybinds.key_pause) {
        player_info.win_focused = !player_info.win_focused;
    }

    if player_info.win_focused {
        for (window, mut cursor_options) in &mut windows {
            if !window.focused {
                continue;
            }

            cursor_options.grab_mode = CursorGrabMode::Locked;
            cursor_options.visible = false;
        }
    } else {
        for (_, mut cursor_options) in &mut windows {
            cursor_options.grab_mode = CursorGrabMode::None;
            cursor_options.visible = true;
        }
    }
}

pub const enable_freecam: bool = false;

fn spawn_freecam(mut commands: Commands) {
    if enable_freecam {
        commands.spawn((
            Camera3d::default(),
            Transform::from_xyz(0., 1.5, 6.).looking_at(Vec3::ZERO, Vec3::Y),
            FreeCamera::default(),
        ));
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("Circle"),
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d::<StandardMaterial>(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        AABB::new(Vec3::new(-10.0, -1.0, -10.0), Vec3::new(10.0, 0.0, 10.0)),
    ));
    commands.spawn((
        Name::new("Cube"),
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d::<StandardMaterial>(materials.add(Color::srgb_u8(124, 144, 255))),
        Transform::from_xyz(0.0, 0.5, 0.0),
        AABB::new(Vec3::splat(-0.5), Vec3::splat(0.5)),
    ));
    commands.spawn((
        Name::new("Cube"),
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
        MeshMaterial3d::<StandardMaterial>(materials.add(Color::srgb_u8(124, 144, 255))),
        Transform::from_xyz(0.0, 0.5, 1.0),
        AABB::new(Vec3::splat(-0.25), Vec3::splat(0.25)),
    ));
    commands.spawn((
        Name::new("Cube"),
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
        MeshMaterial3d::<StandardMaterial>(materials.add(Color::srgb_u8(124, 144, 255))),
        Transform::from_xyz(0.5, 0.15, 1.0),
        AABB::new(Vec3::splat(-0.25), Vec3::splat(0.25)),
    ));
    commands.spawn((
        Name::new("Cube"),
        Mesh3d(meshes.add(Cuboid::new(0.20, 0.20, 0.20))),
        MeshMaterial3d::<StandardMaterial>(materials.add(Color::srgb_u8(124, 144, 255))),
        Transform::from_xyz(0.0, 1.0, 2.0),
        AABB::new(Vec3::splat(-0.10), Vec3::splat(0.10)),
    ));
    commands.spawn((
        Name::new("Light"),
        PointLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    commands
        .spawn((
            // Create a Text with multiple child spans.
            Text::new("FPS: "),
            TextFont {
                // This font is loaded and will be used instead of the default font.
                font: asset_server.load("fonts/FiraSans-Bold.ttf").into(),
                font_size: FontSize::Px(42.0),
                ..default()
            },
        ))
        .with_child((
            TextSpan::default(),
            (
                TextFont {
                    // If the "default_font" feature is unavailable, load a font to use instead.
                    #[cfg(not(feature = "default_font"))]
                    font: asset_server.load("fonts/FiraSans-Medium.ttf").into(),
                    font_size: FontSize::Px(33.0),
                    ..Default::default()
                },
                TextColor(GOLD.into()),
            ),
            FPSText,
        ));
}

fn update_text(diagnostics: Res<DiagnosticsStore>, mut query: Query<&mut TextSpan, With<FPSText>>) {
    for mut span in &mut query {
        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS)
            && let Some(value) = fps.smoothed()
        {
            // Update the value of the second section
            **span = format!("{value:.2}");
        }
    }
}
