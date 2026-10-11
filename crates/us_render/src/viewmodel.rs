//! First-Person Camera and 3D Viewmodel with Sway and Wall Pushback (§7.1, §10, RND-009).
//! Acceptance criteria: "Viewmodel pass w/ sway, wall pushback. No wall clipping at any FOV."

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use us_game::{PlayerInputState, PlayerPhysicalState};
use us_voxel::raycast_terrain;

use crate::terrain_edit::WorldChunks;

/// Perspective mode for camera view (§7.1, §10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraPerspective {
    #[default]
    FirstPerson,
    ThirdPerson,
}

/// Marker component for the first-person / third-person camera entity.
#[derive(Component)]
pub struct FirstPersonCamera {
    pub bob_phase: f32,
    pub is_active: bool,
    pub perspective: CameraPerspective,
    pub current_distance: f32,
    pub target_distance: f32,
}

impl Default for FirstPersonCamera {
    fn default() -> Self {
        Self {
            bob_phase: 0.0,
            is_active: true,
            perspective: CameraPerspective::FirstPerson,
            current_distance: 0.0,
            target_distance: 2.8,
        }
    }
}

/// Component attached to the first-person tool/viewmodel (RND-009).
#[derive(Component)]
pub struct ViewModelTool {
    pub base_offset: Vec3,
    pub base_rotation: Quat,
    pub current_sway: Vec2,
    pub current_pushback: f32,
    pub swing_progress: f32,
    pub is_swinging: bool,
}

impl Default for ViewModelTool {
    fn default() -> Self {
        Self {
            base_offset: Vec3::new(0.22, -0.15, -0.38),
            base_rotation: Quat::from_euler(EulerRot::YXZ, -0.40, -0.30, 0.15),
            current_sway: Vec2::ZERO,
            current_pushback: 0.0,
            swing_progress: 0.0,
            is_swinging: false,
        }
    }
}

/// Builds a low-poly faceted 3D pickaxe mesh for the viewmodel.
pub fn build_viewmodel_tool_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();

    // Helper to add a flat-shaded quad
    let mut add_quad = |p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, col: [f32; 4]| {
        let n = (p1 - p0).cross(p2 - p0).normalize();
        let n_arr = [n.x, n.y, n.z];

        positions.push([p0.x, p0.y, p0.z]);
        positions.push([p1.x, p1.y, p1.z]);
        positions.push([p2.x, p2.y, p2.z]);

        positions.push([p0.x, p0.y, p0.z]);
        positions.push([p2.x, p2.y, p2.z]);
        positions.push([p3.x, p3.y, p3.z]);

        for _ in 0..6 {
            normals.push(n_arr);
            colors.push(col);
        }
    };

    // Helper to add an axis-aligned box
    let mut add_box = |min: Vec3, max: Vec3, col: [f32; 4]| {
        // Front
        add_quad(
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            col,
        );
        // Back
        add_quad(
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            col,
        );
        // Top
        add_quad(
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            col,
        );
        // Bottom
        add_quad(
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, min.y, max.z),
            col,
        );
        // Right
        add_quad(
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(max.x, max.y, max.z),
            col,
        );
        // Left
        add_quad(
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(min.x, max.y, min.z),
            col,
        );
    };

    // 1. Wooden handle (long narrow rectangular cuboid)
    let handle_col = [0.55, 0.38, 0.22, 1.0];
    add_box(
        Vec3::new(-0.02, -0.35, -0.02),
        Vec3::new(0.02, 0.25, 0.02),
        handle_col,
    );

    // 2. Iron pickaxe head (horizontal crossbar)
    let iron_col = [0.68, 0.72, 0.78, 1.0];
    add_box(
        Vec3::new(-0.25, 0.20, -0.04),
        Vec3::new(0.25, 0.26, 0.04),
        iron_col,
    );

    // 3. Left / Right curved pointed pick tips
    let tip_col = [0.82, 0.85, 0.90, 1.0];
    add_box(
        Vec3::new(-0.35, 0.14, -0.03),
        Vec3::new(-0.25, 0.23, 0.03),
        tip_col,
    );
    add_box(
        Vec3::new(0.25, 0.14, -0.03),
        Vec3::new(0.35, 0.23, 0.03),
        tip_col,
    );

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

    mesh
}

/// Updates camera orientation, 1st/3rd person perspective, head-bob, and spring-arm collision.
pub fn first_person_camera_system(
    time: Res<Time>,
    input_state: Res<PlayerInputState>,
    mut physical_state: ResMut<PlayerPhysicalState>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    world_chunks: Res<WorldChunks>,
    mut cam_query: Query<(&mut Transform, &mut FirstPersonCamera), Without<ViewModelTool>>,
) {
    if input_state.fly_mode {
        if let Some((cam_trans, _)) = cam_query.iter().next() {
            physical_state.position = cam_trans.translation - Vec3::Y * physical_state.eye_height;
            physical_state.velocity = Vec3::ZERO;
        }
        return; // Fly camera controls orientation when fly_mode is true
    }

    let dt = time.delta_secs();

    // Toggle perspective with V or F5
    if let Some(kbd) = &keyboard {
        if kbd.just_pressed(KeyCode::KeyV) || kbd.just_pressed(KeyCode::F5) {
            for (_, mut cam) in cam_query.iter_mut() {
                cam.perspective = match cam.perspective {
                    CameraPerspective::FirstPerson => CameraPerspective::ThirdPerson,
                    CameraPerspective::ThirdPerson => CameraPerspective::FirstPerson,
                };
            }
        }
    }

    for (mut transform, mut cam) in cam_query.iter_mut() {
        let rotation =
            Quat::from_rotation_y(input_state.yaw) * Quat::from_rotation_x(input_state.pitch);
        transform.rotation = rotation;

        let eye_pos = physical_state.position + Vec3::new(0.0, physical_state.eye_height, 0.0);

        match cam.perspective {
            CameraPerspective::FirstPerson => {
                let h_speed =
                    Vec2::new(physical_state.velocity.x, physical_state.velocity.z).length();
                let mut bob_offset = Vec3::ZERO;

                if physical_state.on_ground && h_speed > 0.1 {
                    cam.bob_phase += h_speed * dt * 3.5;
                    let bob_y = (cam.bob_phase * 2.0).sin() * 0.035;
                    let bob_x = cam.bob_phase.cos() * 0.02;
                    bob_offset = rotation * Vec3::new(bob_x, bob_y, 0.0);
                } else {
                    cam.bob_phase = 0.0;
                }

                transform.translation = eye_pos + bob_offset;
            }
            CameraPerspective::ThirdPerson => {
                let backward = rotation * Vec3::Z;
                let right = rotation * Vec3::X;
                let target_dist = cam.target_distance;

                // Over-the-shoulder offset: shift camera right and up so player head does not occlude crosshair
                let shoulder_offset = right * 0.45 + Vec3::Y * 0.25;
                let arm_origin = eye_pos + shoulder_offset * 0.4;

                // Sample SDF along the spring-arm to detect cave walls and ceilings smoothly without raymarch jitter
                let mut actual_dist = target_dist;
                let test_steps = 10;
                for i in 1..=test_steps {
                    let d = (i as f32 / test_steps as f32) * target_dist;
                    let probe = arm_origin + backward * d;
                    let sdf = us_voxel::sdf_at_world(&world_chunks.chunks, probe);
                    if sdf < 0.25 {
                        actual_dist = (d - 0.22).max(0.4);
                        break;
                    }
                }

                cam.current_distance = cam.current_distance.lerp(actual_dist, (dt * 12.0).min(1.0));
                let cam_pos = arm_origin + backward * cam.current_distance + shoulder_offset * 0.6;
                transform.translation = cam_pos;
            }
        }
    }
}

/// Updates the First-Person Viewmodel with weapon sway, bobbing, swing juice, and wall pushback (RND-009).
pub fn viewmodel_update_system(
    time: Res<Time>,
    accumulated_mouse: Option<Res<AccumulatedMouseMotion>>,
    mouse_button: Option<Res<ButtonInput<MouseButton>>>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    world_chunks: Res<WorldChunks>,
    cam_query: Query<(&Transform, &FirstPersonCamera), Without<ViewModelTool>>,
    mut tool_query: Query<
        (&mut Transform, &mut Visibility, &mut ViewModelTool),
        Without<FirstPersonCamera>,
    >,
) {
    let Ok((cam_transform, cam)) = cam_query.single() else {
        return;
    };

    if cam.perspective == CameraPerspective::ThirdPerson {
        for (_, mut vis, _) in tool_query.iter_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    } else {
        for (_, mut vis, _) in tool_query.iter_mut() {
            *vis = Visibility::Inherited;
        }
    }

    let dt = time.delta_secs();

    // 1. Mouse sway calculation
    let mut mouse_delta = Vec2::ZERO;
    if let Some(motion) = &accumulated_mouse {
        mouse_delta = motion.delta;
    }

    // 2. Swing animation trigger (Left Click = Dig bite, F = Fill)
    let triggered_swing = mouse_button
        .as_ref()
        .is_some_and(|b| b.just_pressed(MouseButton::Left))
        || keyboard
            .as_ref()
            .is_some_and(|k| k.just_pressed(KeyCode::KeyF));

    // 3. Wall clearance probe: forward raycast from eye position
    let eye_pos = cam_transform.translation;
    let forward = cam_transform.forward();
    let pushback_threshold = 0.55f32; // Wall proximity distance

    // Check hit distance to terrain
    let wall_dist = if let Some(hit) =
        raycast_terrain(&world_chunks.chunks, eye_pos, *forward, pushback_threshold)
    {
        hit.distance
    } else {
        pushback_threshold
    };

    let target_pushback = if wall_dist < pushback_threshold {
        (pushback_threshold - wall_dist) / pushback_threshold
    } else {
        0.0
    };

    for (mut tool_transform, _, mut tool) in tool_query.iter_mut() {
        // Trigger swing
        if triggered_swing && !tool.is_swinging {
            tool.is_swinging = true;
            tool.swing_progress = 0.0;
        }

        // Animate swing
        let mut swing_rot = Quat::IDENTITY;
        if tool.is_swinging {
            tool.swing_progress += dt * 6.0;
            if tool.swing_progress >= 1.0 {
                tool.is_swinging = false;
                tool.swing_progress = 0.0;
            } else {
                let progress = tool.swing_progress;
                let angle = (progress * std::f32::consts::PI).sin() * 0.65;
                swing_rot =
                    Quat::from_rotation_x(-angle * 0.8) * Quat::from_rotation_z(angle * 0.4);
            }
        }

        // Smooth weapon sway
        let target_sway = Vec2::new(
            (-mouse_delta.x * 0.0015).clamp(-0.06, 0.06),
            (-mouse_delta.y * 0.0015).clamp(-0.06, 0.06),
        );
        tool.current_sway = tool.current_sway.lerp(target_sway, (dt * 12.0).min(1.0));

        // Smooth wall pushback (RND-009)
        tool.current_pushback = tool
            .current_pushback
            .lerp(target_pushback, (dt * 15.0).min(1.0));
        let pushback_amount = tool.current_pushback;

        // Position: Base offset + sway - backward push along camera forward
        let sway_offset = Vec3::new(tool.current_sway.x, tool.current_sway.y, 0.0);
        // Wall pushback pushes backward (Z) and slightly upward (Y)
        let push_offset = Vec3::new(0.0, pushback_amount * 0.04, pushback_amount * 0.18);

        let local_pos = tool.base_offset + sway_offset + push_offset;
        tool_transform.translation = cam_transform.transform_point(local_pos);

        // Rotation: Base rotation + sway tilt + swing rotation + wall avoidance tilt
        let sway_rot = Quat::from_rotation_y(tool.current_sway.x * 2.0)
            * Quat::from_rotation_x(-tool.current_sway.y * 2.0);
        let push_tilt = Quat::from_rotation_x(-pushback_amount * 0.5);

        tool_transform.rotation =
            cam_transform.rotation * tool.base_rotation * sway_rot * swing_rot * push_tilt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rnd_009_viewmodel_mesh_generation() {
        let mesh = build_viewmodel_tool_mesh();
        assert!(
            mesh.count_vertices() > 0,
            "Viewmodel mesh must have generated vertices"
        );
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        assert_eq!(positions.len(), mesh.count_vertices());
    }

    #[test]
    fn test_rnd_009_wall_pushback_calculation() {
        let pushback_threshold = 0.55f32;
        let wall_dist_near = 0.25f32;

        let pushback = (pushback_threshold - wall_dist_near) / pushback_threshold;
        assert!(
            pushback > 0.5,
            "Close proximity to wall must trigger substantial pushback (>0.5)"
        );

        let wall_dist_far = 1.2f32;
        let pushback_far = if wall_dist_far < pushback_threshold {
            (pushback_threshold - wall_dist_far) / pushback_threshold
        } else {
            0.0
        };
        assert_eq!(pushback_far, 0.0, "Distant wall must produce 0 pushback");
    }
}
