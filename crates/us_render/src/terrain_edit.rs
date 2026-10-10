use bevy::prelude::*;
use bevy::text::FontSize;
use std::collections::HashMap;
use us_core::ChunkPos;
use us_mesh::{mesh_terrain, MAT_AIR, MAT_DIRT, MAT_GRASS, MAT_STONE};
use us_voxel::{
    apply_edit_op, quantize_sdf, raycast_terrain, BrushMode, BrushShape, EditOp, LoggedOp,
    PaddedChunkArray, TerrainChunk,
};

use crate::fly_cam::FlyCamera;
use crate::mesh_conv::terrain_buffers_to_bevy_mesh;

/// Resource storing all active terrain chunks in memory (§5.1).
#[derive(Resource, Default)]
pub struct WorldChunks {
    pub chunks: HashMap<ChunkPos, TerrainChunk>,
}

/// Resource mapping chunk positions to their spawned Bevy entities and mesh handles.
#[derive(Resource, Default)]
pub struct ChunkRenderEntities {
    pub entities: HashMap<ChunkPos, (Entity, Handle<Mesh>)>,
    pub terrain_material: Handle<StandardMaterial>,
}

/// Resource controlling terrain interaction parameters (TER-004, TER-005).
#[derive(Resource)]
pub struct TerrainEditState {
    pub brush_radius: f32,
    pub reach: f32,
    pub last_bite_time: f32,
    pub bite_interval: f32,
    pub op_log: Vec<LoggedOp>,
    pub op_sequence: u64,
}

impl Default for TerrainEditState {
    fn default() -> Self {
        Self {
            brush_radius: 1.5,
            reach: 12.0,
            last_bite_time: -100.0,
            bite_interval: 0.18,
            op_log: Vec::new(),
            op_sequence: 0,
        }
    }
}

/// Resource accumulating fractional volume removed to spawn discrete drops (TER-008, §5.8).
#[derive(Resource, Default)]
pub struct YieldAccumulator {
    pub accumulated_volume: HashMap<u8, f32>,
    pub total_drops: HashMap<u8, u32>,
}

impl YieldAccumulator {
    /// Adds removed volume in m³ and returns number of new discrete items yielded.
    pub fn add_removed_volume(&mut self, material: u8, volume_m3: f32) -> u32 {
        let entry = self.accumulated_volume.entry(material).or_insert(0.0);
        *entry += volume_m3;

        let threshold = 1.0; // 1 m³ per item drop
        if *entry >= threshold {
            let drops = (*entry / threshold).floor() as u32;
            *entry -= drops as f32 * threshold;
            *self.total_drops.entry(material).or_insert(0) += drops;
            drops
        } else {
            0
        }
    }
}

/// Resource storing HUD and debug overlay stats (DEV-002).
#[derive(Resource, Default)]
pub struct DebugOverlayState {
    pub show_f3: bool,
    pub total_triangles: usize,
    pub chunk_count: usize,
    pub last_hit_dist: Option<f32>,
    pub last_hit_material: Option<u8>,
}

/// Component attached to digging debris particles (TER-019).
#[derive(Component)]
pub struct DebrisParticle {
    pub velocity: Vec3,
    pub lifetime: f32,
    pub age: f32,
}

/// Component marking the HUD text entity for real-time updates.
#[derive(Component)]
pub struct HudText;

/// Spawns HUD elements (crosshair and status text).
pub fn setup_hud(mut commands: Commands) {
    // 1. Center Crosshair (+)
    commands.spawn((
        Text::new("+"),
        TextFont {
            font_size: FontSize::Px(26.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            margin: UiRect {
                left: Val::Px(-6.0),
                top: Val::Px(-14.0),
                ..default()
            },
            ..default()
        },
    ));

    // 2. Status HUD Overlay (Top-Left)
    commands.spawn((
        Text::new("Loading Understrata..."),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(16.0),
            ..default()
        },
        HudText,
    ));
}

/// Handles user input for digging, filling, changing brush radius, and in-place remeshing.
#[allow(clippy::too_many_arguments)]
pub fn terrain_interaction_system(
    mut commands: Commands,
    time: Res<Time>,
    mouse_button: Option<Res<ButtonInput<MouseButton>>>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mut edit_state: ResMut<TerrainEditState>,
    mut world_chunks: ResMut<WorldChunks>,
    mut render_entities: ResMut<ChunkRenderEntities>,
    mut yield_accum: ResMut<YieldAccumulator>,
    mut debug_overlay: ResMut<DebugOverlayState>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    cam_query: Query<(&Transform, &FlyCamera)>,
) {
    let (Some(keyboard), Some(mouse_button), Some(mut meshes), Some(mut materials)) =
        (keyboard, mouse_button, meshes, materials)
    else {
        return;
    };

    let Ok((cam_trans, _)) = cam_query.single() else {
        return;
    };

    // 1. Brush radius selection (Keys 1, 2, 3)
    if keyboard.just_pressed(KeyCode::Digit1) {
        edit_state.brush_radius = 1.0;
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        edit_state.brush_radius = 1.8;
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        edit_state.brush_radius = 3.0;
    }

    // 2. F3 Debug overlay toggle (DEV-002)
    if keyboard.just_pressed(KeyCode::F3) {
        debug_overlay.show_f3 = !debug_overlay.show_f3;
    }

    // 3. Continuous Raycast from Camera Forward
    let cam_pos = cam_trans.translation;
    let cam_forward = *cam_trans.forward();
    let hit = raycast_terrain(&world_chunks.chunks, cam_pos, cam_forward, edit_state.reach);

    if let Some(h) = hit {
        debug_overlay.last_hit_dist = Some(h.distance);
        debug_overlay.last_hit_material = Some(h.material);
    } else {
        debug_overlay.last_hit_dist = None;
        debug_overlay.last_hit_material = None;
    }

    // 4. Check Dig / Fill Trigger
    let is_dig = mouse_button.pressed(MouseButton::Left);
    let is_fill = keyboard.pressed(KeyCode::KeyF);

    if !is_dig && !is_fill {
        return;
    }

    let Some(hit_info) = hit else {
        return;
    };

    let now = time.elapsed_secs();
    if now - edit_state.last_bite_time < edit_state.bite_interval {
        return;
    }
    edit_state.last_bite_time = now;

    let radius = edit_state.brush_radius;
    let radius_q = (radius * 16.0).round().clamp(1.0, 65535.0) as u16;

    let (brush_center, brush_mode, target_material) = if is_dig {
        // TER-004: Brush center pushed inward along -normal by 0.4 * radius
        let center = hit_info.position - hit_info.normal * (0.4 * radius);
        (center, BrushMode::Subtract, MAT_AIR)
    } else {
        // TER-005: Brush center placed on surface / along +normal
        let center = hit_info.position + hit_info.normal * (0.4 * radius);
        (center, BrushMode::Add, MAT_DIRT)
    };

    let center_q = IVec3::new(
        (brush_center.x * 16.0).round() as i32,
        (brush_center.y * 16.0).round() as i32,
        (brush_center.z * 16.0).round() as i32,
    );

    let op = EditOp::TerrainBrush {
        center_q,
        radius_q,
        shape: BrushShape::Sphere,
        mode: brush_mode,
        material: target_material,
        strength: 255,
    };

    // 5. Authoritative Edit Op Application (pure function §5.6)
    let applied = apply_edit_op(&mut world_chunks.chunks, &op);

    // Record op into sequential replay log (TER-009)
    edit_state.op_sequence += 1;
    let seq = edit_state.op_sequence;
    edit_state.op_log.push(LoggedOp {
        seq,
        tick: (now * 60.0) as u64,
        actor: 0,
        op,
    });

    // 6. Yield Accumulation & Drops (TER-008)
    for (&mat, &vol) in &applied.removed_volume_per_mat {
        yield_accum.add_removed_volume(mat, vol);
    }

    // 7. Juice: Spawn Debris Particles (TER-019)
    let particle_mat = match hit_info.material {
        MAT_GRASS => materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.70, 0.25),
            perceptual_roughness: 0.9,
            ..default()
        }),
        MAT_DIRT => materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.35, 0.22),
            perceptual_roughness: 0.9,
            ..default()
        }),
        MAT_STONE => materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.55, 0.55),
            perceptual_roughness: 0.9,
            ..default()
        }),
        _ => materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.6, 0.6),
            ..default()
        }),
    };

    let particle_mesh = meshes.add(Cuboid::new(0.12, 0.12, 0.12));

    // Spawn 8 randomized particle cubes
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::PI * 0.25;
        let spread_x = angle.cos() * 2.2;
        let spread_z = angle.sin() * 2.2;
        let velocity = hit_info.normal * 3.5 + Vec3::new(spread_x, 2.0, spread_z);

        commands.spawn((
            Mesh3d(particle_mesh.clone()),
            MeshMaterial3d(particle_mat.clone()),
            Transform::from_translation(hit_info.position),
            DebrisParticle {
                velocity,
                lifetime: 0.45,
                age: 0.0,
            },
        ));
    }

    // 8. In-Place Dynamic Remeshing (TER-007): Zero-flicker swap on dirty chunks
    for &cpos in &applied.dirty_chunks {
        let Some(chunk) = world_chunks.chunks.get(&cpos) else {
            continue;
        };

        let padded = PaddedChunkArray::from_chunk_and_neighbors(chunk, |target, nx, ny, nz| {
            if let Some(neighbor) = world_chunks.chunks.get(&target) {
                (neighbor.sdf.get(nx, ny, nz), neighbor.mat.get(nx, ny, nz))
            } else {
                (quantize_sdf(10.0), MAT_AIR)
            }
        });

        let mesh_data = mesh_terrain(&padded);
        let new_bevy_mesh = terrain_buffers_to_bevy_mesh(&mesh_data);

        if let Some((_, handle)) = render_entities.entities.get(&cpos) {
            // Update existing mesh asset in-place
            if let Some(mut mesh_asset) = meshes.get_mut(handle) {
                *mesh_asset = new_bevy_mesh;
            }
        } else if !mesh_data.is_empty() {
            // Newly meshed chunk that previously had no triangles
            let handle = meshes.add(new_bevy_mesh);
            let min_pos = cpos.min_world().0;
            let entity = commands
                .spawn((
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(render_entities.terrain_material.clone()),
                    Transform::from_translation(min_pos),
                ))
                .id();
            render_entities.entities.insert(cpos, (entity, handle));
        }
    }
}

/// Updates particles: velocity, gravity, scaling, and despawning.
pub fn particle_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut DebrisParticle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut trans, mut p) in query.iter_mut() {
        p.age += dt;
        if p.age >= p.lifetime {
            commands.entity(entity).despawn();
            continue;
        }

        // Apply gravity
        p.velocity.y -= 14.0 * dt;
        trans.translation += p.velocity * dt;

        // Shrink particle as it ages
        let life_factor = (1.0 - (p.age / p.lifetime)).max(0.0);
        trans.scale = Vec3::splat(life_factor);
    }
}

/// Updates HUD text to reflect player actions, yield counts, and F3 debug statistics.
pub fn hud_update_system(
    edit_state: Res<TerrainEditState>,
    yield_accum: Res<YieldAccumulator>,
    debug_overlay: Res<DebugOverlayState>,
    cam_query: Query<&Transform, With<FlyCamera>>,
    mut text_query: Query<&mut Text, With<HudText>>,
) {
    let Ok(mut text) = text_query.single_mut() else {
        return;
    };

    let cam_pos = if let Ok(cam_t) = cam_query.single() {
        cam_t.translation
    } else {
        Vec3::ZERO
    };

    let dirt_yield = yield_accum.total_drops.get(&MAT_DIRT).copied().unwrap_or(0);
    let stone_yield = yield_accum
        .total_drops
        .get(&MAT_STONE)
        .copied()
        .unwrap_or(0);

    let mut content = format!(
        "UNDERSTRATA — M2 (Dig & Fill)\n\
         [LMB] Dig  [F] Fill  [1/2/3] Radius: {:.1}m\n\
         Yield: Dirt: {} | Stone: {}\n",
        edit_state.brush_radius, dirt_yield, stone_yield
    );

    if debug_overlay.show_f3 {
        let hit_str = match (debug_overlay.last_hit_dist, debug_overlay.last_hit_material) {
            (Some(d), Some(m)) => {
                let mname = match m {
                    MAT_GRASS => "Grass",
                    MAT_DIRT => "Dirt",
                    MAT_STONE => "Stone",
                    _ => "Other",
                };
                format!("{:.1}m ({})", d, mname)
            }
            _ => "None".to_string(),
        };

        content += &format!(
            "\n--- F3 DEBUG ---\n\
             Cam Pos: ({:.1}, {:.1}, {:.1})\n\
             Target: {}\n\
             Ops Applied: {}\n",
            cam_pos.x,
            cam_pos.y,
            cam_pos.z,
            hit_str,
            edit_state.op_log.len()
        );
    } else {
        content += "[F3] Toggle Debug Overlay";
    }

    **text = content;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yield_accumulator_threshold() {
        let mut accum = YieldAccumulator::default();
        assert_eq!(accum.add_removed_volume(MAT_DIRT, 0.4), 0);
        assert_eq!(accum.add_removed_volume(MAT_DIRT, 0.5), 0);
        assert_eq!(accum.add_removed_volume(MAT_DIRT, 0.3), 1); // 1.2 m³ accumulated -> 1 drop
        assert_eq!(accum.total_drops.get(&MAT_DIRT), Some(&1));

        assert_eq!(accum.add_removed_volume(MAT_DIRT, 2.0), 2); // remaining 0.2 + 2.0 = 2.2 -> 2 drops
        assert_eq!(accum.total_drops.get(&MAT_DIRT), Some(&3));
    }
}
