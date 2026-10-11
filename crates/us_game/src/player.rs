//! Local player entity, input gathering, and authoritative server delta reconciliation (§7, §17).

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use us_net::{ClientMessage, PlayerInputIntent, ServerMessage};

use crate::server::LocalServer;

/// Marker component for the local player entity.
#[derive(Component, Default)]
pub struct LocalPlayer;

/// Resource tracking client-side player camera orientation and input sequence.
#[derive(Resource)]
pub struct PlayerInputState {
    pub sequence: u64,
    pub yaw: f32,
    pub pitch: f32,
    pub sensitivity: f32,
    pub fly_mode: bool,
}

impl Default for PlayerInputState {
    fn default() -> Self {
        Self {
            sequence: 0,
            yaw: 0.0,
            pitch: -0.2,
            sensitivity: 0.003,
            fly_mode: false,
        }
    }
}

/// Resource holding the latest authoritative player physical state received from the server.
#[derive(Resource)]
pub struct PlayerPhysicalState {
    pub position: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub is_crouching: bool,
    pub eye_height: f32,
}

impl Default for PlayerPhysicalState {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 24.0, 42.0),
            velocity: Vec3::ZERO,
            on_ground: false,
            is_crouching: false,
            eye_height: 1.65,
        }
    }
}

/// Gathers user input and dispatches intent to the authoritative local server (NET-001).
pub fn player_input_system(
    time: Res<Time>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mouse_button: Option<Res<ButtonInput<MouseButton>>>,
    accumulated_mouse: Option<Res<AccumulatedMouseMotion>>,
    mut input_state: ResMut<PlayerInputState>,
    physical_state: Res<PlayerPhysicalState>,
    mut server: ResMut<LocalServer>,
) {
    let (Some(keyboard), Some(mouse_button)) = (keyboard, mouse_button) else {
        return;
    };

    // Toggle fly mode with P (Creative / Debug MOV-011)
    if keyboard.just_pressed(KeyCode::KeyP) {
        input_state.fly_mode = !input_state.fly_mode;
        info!("Fly mode toggled: {}", input_state.fly_mode);
        if !input_state.fly_mode {
            server.player_state.position = physical_state.position;
            server.player_state.velocity = Vec3::ZERO;
        }
    }

    // Look rotation (active when Right Mouse held, or always when not in menu)
    if mouse_button.pressed(MouseButton::Right) {
        if let Some(mouse_motion) = &accumulated_mouse {
            let delta = mouse_motion.delta;
            if delta.length_squared() > 0.0 {
                input_state.yaw -= delta.x * input_state.sensitivity;
                input_state.pitch -= delta.y * input_state.sensitivity;
                input_state.pitch = input_state.pitch.clamp(-1.54, 1.54);
            }
        }
    }

    if input_state.fly_mode {
        return; // Fly camera system handles motion in fly mode
    }

    // Horizontal WASD input
    let mut move_dir = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        move_dir.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        move_dir.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        move_dir.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        move_dir.x += 1.0;
    }

    let jump = keyboard.pressed(KeyCode::Space);
    let sprint = keyboard.pressed(KeyCode::ShiftLeft);
    let crouch = keyboard.pressed(KeyCode::ControlLeft);

    input_state.sequence += 1;
    let intent = PlayerInputIntent {
        sequence: input_state.sequence,
        move_dir,
        look_yaw: input_state.yaw,
        look_pitch: input_state.pitch,
        jump,
        sprint,
        crouch,
        dt: time.delta_secs(),
    };

    let now = time.elapsed_secs_f64();
    server.send_from_client(ClientMessage::PlayerInput(intent), now);
}

/// Receives authoritative server state deltas and updates client representation.
pub fn player_reconciliation_system(
    time: Res<Time>,
    mut server: ResMut<LocalServer>,
    mut physical_state: ResMut<PlayerPhysicalState>,
    mut query: Query<&mut Transform, (With<LocalPlayer>, Without<Camera3d>)>,
) {
    let now = time.elapsed_secs_f64();
    let messages = server.poll_client_inbox(now);

    for msg in messages {
        if let ServerMessage::PlayerState(delta) = msg {
            physical_state.position = delta.position;
            physical_state.velocity = delta.velocity;
            physical_state.on_ground = delta.on_ground;
            physical_state.is_crouching = delta.is_crouching;
            physical_state.eye_height = if delta.is_crouching { 1.05 } else { 1.65 };

            // Update transform if entity exists
            for mut transform in query.iter_mut() {
                transform.translation = delta.position;
            }
        }
    }
}
