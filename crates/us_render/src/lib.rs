pub mod fly_cam;
pub mod mesh_conv;
pub mod terrain_edit;

use bevy::prelude::*;
use fly_cam::{fly_camera_system, FlyCamera};
use mesh_conv::terrain_buffers_to_bevy_mesh;
use terrain_edit::{
    hud_update_system, particle_update_system, setup_hud, terrain_interaction_system,
    ChunkRenderEntities, DebugOverlayState, TerrainEditState, WorldChunks, YieldAccumulator,
};
use us_core::ChunkPos;
use us_mesh::mesh_terrain;
use us_voxel::PaddedChunkArray;
use us_worldgen::gen_test_chunk;

/// Understrata Rendering Plugin (terrain meshing, dynamic remeshing, low-poly presentation, fly camera, HUD)
pub struct UnderstrataRenderPlugin;

impl Plugin for UnderstrataRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldChunks>()
            .init_resource::<ChunkRenderEntities>()
            .init_resource::<TerrainEditState>()
            .init_resource::<YieldAccumulator>()
            .init_resource::<DebugOverlayState>()
            .add_systems(Startup, (setup_world_renderer, setup_hud))
            .add_systems(
                Update,
                (
                    fly_camera_system,
                    terrain_interaction_system,
                    particle_update_system,
                    hud_update_system,
                ),
            );
    }
}

fn setup_world_renderer(
    mut commands: Commands,
    mut world_chunks: ResMut<WorldChunks>,
    mut render_entities: ResMut<ChunkRenderEntities>,
    mut debug_overlay: ResMut<DebugOverlayState>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        info!("Assets<Mesh>/Assets<StandardMaterial> not available (headless mode). Skipping presentation mesh spawn.");
        return;
    };
    info!("Setting up Understrata terrain renderer (M1/M2)...");

    // 1. Directional Sun Light (golden-hour sunlight matching mockups)
    commands.spawn((
        DirectionalLight {
            illuminance: 15_000.0,
            shadow_maps_enabled: true,
            color: Color::srgb(1.0, 0.95, 0.85),
            ..default()
        },
        Transform::from_xyz(80.0, 120.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // 2. Camera with AmbientLight Component (MOV-011) overlooking the terrain
    commands.spawn((
        Camera3d::default(),
        AmbientLight {
            color: Color::srgb(0.55, 0.70, 0.90),
            brightness: 400.0,
            ..default()
        },
        Transform::from_xyz(0.0, 30.0, 50.0).looking_at(Vec3::new(0.0, 5.0, 0.0), Vec3::Y),
        FlyCamera::default(),
    ));

    // 3. Shared Low-Poly Terrain Material
    let terrain_material = materials.add(StandardMaterial {
        perceptual_roughness: 0.92,
        reflectance: 0.08,
        cull_mode: None,
        ..default()
    });
    render_entities.terrain_material = terrain_material.clone();

    // 4. Pre-generate chunks with a 1-chunk border for seamless neighbor sampling
    let seed = 42;
    let radius = 2; // -2..=1 gives 4x4 chunks in XZ
    let mut chunks = std::collections::HashMap::new();

    for cy in -2..=1 {
        for cz in (-radius - 1)..=(radius + 1) {
            for cx in (-radius - 1)..=(radius + 1) {
                let pos = ChunkPos::new(cx, cy, cz);
                chunks.insert(pos, gen_test_chunk(seed, pos));
            }
        }
    }

    // 5. Mesh and spawn active chunks
    let mut total_triangles = 0;
    let mut chunk_count = 0;

    for cy in -1..=0 {
        for cz in -radius..radius {
            for cx in -radius..radius {
                let pos = ChunkPos::new(cx, cy, cz);
                let Some(chunk) = chunks.get(&pos) else {
                    continue;
                };

                let padded =
                    PaddedChunkArray::from_chunk_and_neighbors(chunk, |target, nx, ny, nz| {
                        if let Some(neighbor) = chunks.get(&target) {
                            (neighbor.sdf.get(nx, ny, nz), neighbor.mat.get(nx, ny, nz))
                        } else {
                            (us_voxel::quantize_sdf(10.0), 0)
                        }
                    });

                let mesh_data = mesh_terrain(&padded);
                if !mesh_data.is_empty() {
                    total_triangles += mesh_data.triangle_count();
                    chunk_count += 1;

                    let bevy_mesh = terrain_buffers_to_bevy_mesh(&mesh_data);
                    let mesh_handle = meshes.add(bevy_mesh);

                    let min_pos = pos.min_world().0;
                    let entity = commands
                        .spawn((
                            Mesh3d(mesh_handle.clone()),
                            MeshMaterial3d(terrain_material.clone()),
                            Transform::from_translation(min_pos),
                        ))
                        .id();

                    render_entities.entities.insert(pos, (entity, mesh_handle));
                }
            }
        }
    }

    world_chunks.chunks = chunks;
    debug_overlay.total_triangles = total_triangles;
    debug_overlay.chunk_count = chunk_count;

    info!(
        "Spawned {} terrain chunks with a total of {} flat-shaded triangles.",
        chunk_count, total_triangles
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_chunks_mesh_correctly() {
        let seed = 42;
        let mut chunks = std::collections::HashMap::new();
        for cy in -2..=1 {
            for cz in -3..=3 {
                for cx in -3..=3 {
                    let pos = ChunkPos::new(cx, cy, cz);
                    chunks.insert(pos, gen_test_chunk(seed, pos));
                }
            }
        }

        let pos = ChunkPos::new(0, 0, 0);
        let chunk = chunks.get(&pos).unwrap();
        let padded = PaddedChunkArray::from_chunk_and_neighbors(chunk, |target, nx, ny, nz| {
            if let Some(neighbor) = chunks.get(&target) {
                (neighbor.sdf.get(nx, ny, nz), neighbor.mat.get(nx, ny, nz))
            } else {
                (us_voxel::quantize_sdf(10.0), 0)
            }
        });

        let mesh_data = mesh_terrain(&padded);
        assert!(
            mesh_data.triangle_count() > 1000,
            "Surface chunk should produce solid terrain mesh"
        );

        let pos_neg = ChunkPos::new(0, -1, 0);
        let chunk_neg = chunks.get(&pos_neg).unwrap();
        let padded_neg =
            PaddedChunkArray::from_chunk_and_neighbors(chunk_neg, |target, nx, ny, nz| {
                if let Some(neighbor) = chunks.get(&target) {
                    (neighbor.sdf.get(nx, ny, nz), neighbor.mat.get(nx, ny, nz))
                } else {
                    (us_voxel::quantize_sdf(10.0), 0)
                }
            });
        let mesh_data_neg = mesh_terrain(&padded_neg);
        assert!(
            mesh_data_neg.triangle_count() > 1000,
            "Subsurface chunk should produce solid terrain mesh"
        );
    }
}
