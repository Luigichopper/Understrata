use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

#[derive(Component)]
pub struct FlyCamera {
    pub speed: f32,
    pub sensitivity: f32,
    pub pitch: f32,
    pub yaw: f32,
}

impl Default for FlyCamera {
    fn default() -> Self {
        Self {
            speed: 25.0,
            sensitivity: 0.003,
            pitch: -0.2,
            yaw: 0.0,
        }
    }
}

pub fn fly_camera_system(
    time: Res<Time>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mouse_button: Option<Res<ButtonInput<MouseButton>>>,
    accumulated_mouse: Option<Res<AccumulatedMouseMotion>>,
    mut query: Query<(&mut Transform, &mut FlyCamera)>,
    input_state: Option<Res<us_game::PlayerInputState>>,
) {
    if let Some(state) = &input_state {
        if !state.fly_mode {
            return;
        }
    }

    let (Some(keyboard), Some(mouse_button)) = (keyboard, mouse_button) else {
        return;
    };
    for (mut transform, mut cam) in query.iter_mut() {
        // Rotate camera if right mouse button is held, or if mouse moved
        if mouse_button.pressed(MouseButton::Right) {
            if let Some(mouse_motion) = &accumulated_mouse {
                let delta = mouse_motion.delta;
                if delta.length_squared() > 0.0 {
                    cam.yaw -= delta.x * cam.sensitivity;
                    cam.pitch -= delta.y * cam.sensitivity;
                    cam.pitch = cam.pitch.clamp(-1.54, 1.54);

                    transform.rotation =
                        Quat::from_rotation_y(cam.yaw) * Quat::from_rotation_x(cam.pitch);
                }
            }
        }

        // Movement relative to camera orientation
        let mut move_dir = Vec3::ZERO;
        let forward = transform.forward();
        let right = transform.right();
        let up = Vec3::Y;

        if keyboard.pressed(KeyCode::KeyW) {
            move_dir += *forward;
        }
        if keyboard.pressed(KeyCode::KeyS) {
            move_dir -= *forward;
        }
        if keyboard.pressed(KeyCode::KeyA) {
            move_dir -= *right;
        }
        if keyboard.pressed(KeyCode::KeyD) {
            move_dir += *right;
        }
        if keyboard.pressed(KeyCode::Space) {
            move_dir += up;
        }
        if keyboard.pressed(KeyCode::ShiftLeft) {
            move_dir -= up;
        }

        if move_dir.length_squared() > 0.0 {
            let speed = if keyboard.pressed(KeyCode::ControlLeft) {
                cam.speed * 2.5
            } else {
                cam.speed
            };
            transform.translation += move_dir.normalize() * speed * time.delta_secs();
        }
    }
}
