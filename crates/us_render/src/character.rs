//! 3D Low-Poly Character Model and Articulated Animation System (§7.1, §10, RND-009).
//! Implements visible 3rd-person miner player model with animated legs, arms, and equipped pickaxe.

use bevy::prelude::*;
use us_game::{PlayerInputState, PlayerPhysicalState};

use crate::viewmodel::{CameraPerspective, FirstPersonCamera};

/// Marker component for the root entity of the character model.
#[derive(Component)]
pub struct CharacterVisualRoot;

/// References to child limb entities for articulated walking and tool swinging.
#[derive(Component)]
pub struct CharacterLimbs {
    pub left_leg: Entity,
    pub right_leg: Entity,
    pub left_arm: Entity,
    pub right_arm: Entity,
    pub walk_phase: f32,
    pub swing_progress: f32,
    pub is_swinging: bool,
}

/// Helper function to build a faceted box with vertex colors and outward face normals.
fn add_box_geometry(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    min: Vec3,
    max: Vec3,
    col: [f32; 4],
) {
    let mut add_quad = |p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3| {
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

    // Front (+Z)
    add_quad(
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    );
    // Back (-Z)
    add_quad(
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
    );
    // Top (+Y)
    add_quad(
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
    );
    // Bottom (-Y)
    add_quad(
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, min.y, max.z),
    );
    // Right (+X)
    add_quad(
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(max.x, max.y, max.z),
    );
    // Left (-X)
    add_quad(
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(min.x, max.y, min.z),
    );
}

/// Builds the low-poly torso, head, hard-hat, and glowing headlamp mesh.
pub fn build_torso_head_mesh() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();

    // 1. Leather Belt with Brass Buckle
    let belt_col = [0.22, 0.14, 0.08, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.21, 0.82, -0.13),
        Vec3::new(0.21, 0.90, 0.13),
        belt_col,
    );
    let buckle_col = [0.85, 0.72, 0.22, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.05, 0.83, 0.12),
        Vec3::new(0.05, 0.89, 0.14),
        buckle_col,
    );

    // 2. Terracotta Canvas Miner Jacket
    let jacket_col = [0.65, 0.34, 0.18, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.22, 0.90, -0.13),
        Vec3::new(0.22, 1.44, 0.13),
        jacket_col,
    );

    // 2b. Miner Backpack / Gear Pack (adds depth and silhouette from behind)
    let pack_col = [0.26, 0.29, 0.24, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.16, 0.96, -0.23),
        Vec3::new(0.16, 1.38, -0.13),
        pack_col,
    );
    let pack_trim_col = [0.20, 0.14, 0.08, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.14, 1.38, -0.21),
        Vec3::new(0.14, 1.42, -0.14),
        pack_trim_col,
    );

    // 3. Neck (Skin tone)
    let skin_col = [0.82, 0.65, 0.52, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.07, 1.44, -0.07),
        Vec3::new(0.07, 1.50, 0.07),
        skin_col,
    );

    // 4. Head / Face (Skin tone)
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.13, 1.50, -0.13),
        Vec3::new(0.13, 1.70, 0.13),
        skin_col,
    );

    // 5. Visor / Miner Goggles
    let visor_col = [0.15, 0.15, 0.18, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.11, 1.58, 0.11),
        Vec3::new(0.11, 1.66, 0.14),
        visor_col,
    );

    // 6. Miner Hard-Hat Helmet (Safety Yellow) with Brim and Crown Ridge
    let helmet_col = [0.95, 0.76, 0.10, 1.0];
    // Main cap dome
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.16, 1.66, -0.17),
        Vec3::new(0.16, 1.80, 0.17),
        helmet_col,
    );
    // Front and side brim
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.17, 1.66, -0.18),
        Vec3::new(0.17, 1.71, 0.22),
        helmet_col,
    );
    // Center reinforcing ridge
    let ridge_col = [0.88, 0.70, 0.08, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.04, 1.78, -0.16),
        Vec3::new(0.04, 1.83, 0.17),
        ridge_col,
    );

    // 7. Headlamp Bezel and Glowing Lens
    let bezel_col = [0.15, 0.16, 0.18, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.06, 1.68, 0.21),
        Vec3::new(0.06, 1.78, 0.23),
        bezel_col,
    );
    let headlamp_col = [0.90, 1.0, 1.0, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.04, 1.70, 0.23),
        Vec3::new(0.04, 1.76, 0.25),
        headlamp_col,
    );

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

/// Builds a single leg mesh with dark work denim and leather boots (pivot at hip y = 0.0).
pub fn build_leg_mesh() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();

    // Denim work pants (hip y=0.0 down to y=-0.62)
    let denim_col = [0.20, 0.24, 0.35, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.08, -0.62, -0.09),
        Vec3::new(0.08, 0.0, 0.09),
        denim_col,
    );

    // Leather work boot (y=-0.85 to y=-0.62, with toe box extending forward)
    let boot_col = [0.28, 0.18, 0.11, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.085, -0.85, -0.09),
        Vec3::new(0.085, -0.62, 0.13),
        boot_col,
    );

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

/// Builds an arm mesh (pivot at shoulder y = 0.0) optionally equipped with a held pickaxe.
pub fn build_arm_mesh(with_pickaxe: bool) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();

    // Terracotta sleeve (y=0.0 down to y=-0.38)
    let sleeve_col = [0.65, 0.34, 0.18, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.06, -0.38, -0.07),
        Vec3::new(0.06, 0.0, 0.07),
        sleeve_col,
    );

    // Leather work glove (y=-0.52 to y=-0.38)
    let glove_col = [0.26, 0.16, 0.10, 1.0];
    add_box_geometry(
        &mut positions,
        &mut normals,
        &mut colors,
        Vec3::new(-0.06, -0.52, -0.07),
        Vec3::new(0.06, -0.38, 0.07),
        glove_col,
    );

    if with_pickaxe {
        // Wooden shaft held in hand
        let handle_col = [0.55, 0.38, 0.22, 1.0];
        add_box_geometry(
            &mut positions,
            &mut normals,
            &mut colors,
            Vec3::new(-0.02, -0.76, -0.02),
            Vec3::new(0.02, -0.15, 0.02),
            handle_col,
        );

        // Iron pickaxe head
        let iron_col = [0.68, 0.72, 0.78, 1.0];
        add_box_geometry(
            &mut positions,
            &mut normals,
            &mut colors,
            Vec3::new(-0.03, -0.22, -0.20),
            Vec3::new(0.03, -0.16, 0.20),
            iron_col,
        );

        // Pointed pick tips
        let tip_col = [0.82, 0.85, 0.90, 1.0];
        add_box_geometry(
            &mut positions,
            &mut normals,
            &mut colors,
            Vec3::new(-0.025, -0.28, -0.28),
            Vec3::new(0.025, -0.19, -0.20),
            tip_col,
        );
        add_box_geometry(
            &mut positions,
            &mut normals,
            &mut colors,
            Vec3::new(-0.025, -0.28, 0.20),
            Vec3::new(0.025, -0.19, 0.28),
            tip_col,
        );
    }

    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

/// Spawns the articulated 3D character hierarchy in the world.
pub fn spawn_character_model(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    spawn_pos: Vec3,
) -> Entity {
    let char_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 1.0),
        perceptual_roughness: 0.45,
        reflectance: 0.20,
        cull_mode: None,
        ..default()
    });

    let torso_mesh = meshes.add(build_torso_head_mesh());
    let leg_mesh = meshes.add(build_leg_mesh());
    let left_arm_mesh = meshes.add(build_arm_mesh(false));
    let right_arm_mesh = meshes.add(build_arm_mesh(true));

    // Spawn child limbs with pivots
    let left_leg = commands
        .spawn((
            Mesh3d(leg_mesh.clone()),
            MeshMaterial3d(char_mat.clone()),
            Transform::from_xyz(-0.11, 0.85, 0.0),
        ))
        .id();

    let right_leg = commands
        .spawn((
            Mesh3d(leg_mesh),
            MeshMaterial3d(char_mat.clone()),
            Transform::from_xyz(0.11, 0.85, 0.0),
        ))
        .id();

    let left_arm = commands
        .spawn((
            Mesh3d(left_arm_mesh),
            MeshMaterial3d(char_mat.clone()),
            Transform::from_xyz(-0.28, 1.40, 0.0),
        ))
        .id();

    let right_arm = commands
        .spawn((
            Mesh3d(right_arm_mesh),
            MeshMaterial3d(char_mat.clone()),
            Transform::from_xyz(0.28, 1.40, 0.0),
        ))
        .id();

    // Spawn torso child
    let torso = commands
        .spawn((
            Mesh3d(torso_mesh),
            MeshMaterial3d(char_mat),
            Transform::IDENTITY,
        ))
        .id();

    // Spawn root entity (initially hidden in 1st-person view)
    let root = commands
        .spawn((
            CharacterVisualRoot,
            CharacterLimbs {
                left_leg,
                right_leg,
                left_arm,
                right_arm,
                walk_phase: 0.0,
                swing_progress: 0.0,
                is_swinging: false,
            },
            Transform::from_translation(spawn_pos),
            Visibility::Hidden,
        ))
        .add_child(torso)
        .add_child(left_leg)
        .add_child(right_leg)
        .add_child(left_arm)
        .add_child(right_arm)
        .id();

    root
}

/// Updates character model position, orientation, limb walk-cycles, and tool swings (RND-009).
#[allow(clippy::too_many_arguments)]
pub fn character_animation_system(
    time: Res<Time>,
    physical_state: Res<PlayerPhysicalState>,
    input_state: Res<PlayerInputState>,
    mouse_button: Option<Res<ButtonInput<MouseButton>>>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    cam_query: Query<&FirstPersonCamera>,
    mut root_query: Query<
        (&mut Transform, &mut Visibility, &mut CharacterLimbs),
        With<CharacterVisualRoot>,
    >,
    mut limb_query: Query<
        &mut Transform,
        (Without<CharacterVisualRoot>, Without<FirstPersonCamera>),
    >,
) {
    let Ok(cam) = cam_query.single() else {
        return;
    };
    let dt = time.delta_secs();
    let is_3rd_person = cam.perspective == CameraPerspective::ThirdPerson;

    for (mut root_trans, mut vis, mut limbs) in root_query.iter_mut() {
        *vis = if is_3rd_person {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };

        if !is_3rd_person {
            continue;
        }

        // Position root at player ground position and smoothly align yaw
        root_trans.translation = physical_state.position;
        let target_rot = Quat::from_rotation_y(input_state.yaw);
        root_trans.rotation = root_trans.rotation.slerp(target_rot, (dt * 22.0).min(1.0));

        // Smooth crouch height scaling
        let target_scale = if physical_state.is_crouching {
            Vec3::new(1.0, 1.2 / 1.8, 1.0)
        } else {
            Vec3::ONE
        };
        root_trans.scale = root_trans.scale.lerp(target_scale, (dt * 15.0).min(1.0));

        // Dig / Fill swing animation trigger
        let triggered_swing = mouse_button
            .as_ref()
            .is_some_and(|b| b.just_pressed(MouseButton::Left))
            || keyboard
                .as_ref()
                .is_some_and(|k| k.just_pressed(KeyCode::KeyF));

        if triggered_swing && !limbs.is_swinging {
            limbs.is_swinging = true;
            limbs.swing_progress = 0.0;
        }

        if limbs.is_swinging {
            limbs.swing_progress += dt * 6.0;
            if limbs.swing_progress >= 1.0 {
                limbs.is_swinging = false;
                limbs.swing_progress = 0.0;
            }
        }

        // Limb walk cycle based on ground speed
        let h_speed = Vec2::new(physical_state.velocity.x, physical_state.velocity.z).length();
        let is_walking = physical_state.on_ground && h_speed > 0.1;
        if is_walking {
            limbs.walk_phase += h_speed * dt * 6.5;
        }

        let leg_angle = (limbs.walk_phase).sin() * 0.45;
        let arm_angle = (limbs.walk_phase).sin() * 0.40;

        let leg_l_target = if is_walking {
            Quat::from_rotation_x(leg_angle)
        } else {
            Quat::IDENTITY
        };
        let leg_r_target = if is_walking {
            Quat::from_rotation_x(-leg_angle)
        } else {
            Quat::IDENTITY
        };
        let arm_l_target = if is_walking {
            Quat::from_rotation_x(-arm_angle)
        } else {
            Quat::IDENTITY
        };

        let swing_chop = if limbs.is_swinging {
            let p = limbs.swing_progress;
            (p * std::f32::consts::PI).sin() * 1.2
        } else {
            0.0
        };
        let arm_r_base = if is_walking { arm_angle } else { 0.0 };
        let arm_r_target = Quat::from_rotation_x(arm_r_base - swing_chop);

        if let Ok(mut t) = limb_query.get_mut(limbs.left_leg) {
            t.rotation = t.rotation.slerp(leg_l_target, (dt * 15.0).min(1.0));
        }
        if let Ok(mut t) = limb_query.get_mut(limbs.right_leg) {
            t.rotation = t.rotation.slerp(leg_r_target, (dt * 15.0).min(1.0));
        }
        if let Ok(mut t) = limb_query.get_mut(limbs.left_arm) {
            t.rotation = t.rotation.slerp(arm_l_target, (dt * 15.0).min(1.0));
        }
        if let Ok(mut t) = limb_query.get_mut(limbs.right_arm) {
            t.rotation = t.rotation.slerp(arm_r_target, (dt * 20.0).min(1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_character_meshes_generate_valid_triangles() {
        let torso = build_torso_head_mesh();
        assert!(torso.count_vertices() > 0);

        let leg = build_leg_mesh();
        assert!(leg.count_vertices() > 0);

        let arm = build_arm_mesh(true);
        assert!(arm.count_vertices() > 0);
    }
}
