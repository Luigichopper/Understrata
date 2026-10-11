pub mod character;
pub mod fly_cam;
pub mod mesh_conv;
pub mod terrain_edit;
pub mod viewmodel;

use bevy::prelude::*;
use character::{character_animation_system, spawn_character_model};
use fly_cam::{fly_camera_system, FlyCamera};
use mesh_conv::{split_terrain_buffers_by_material, terrain_buffers_to_bevy_mesh};
use terrain_edit::{
    hud_update_system, particle_update_system, setup_hud, terrain_interaction_system,
    ChunkRenderEntities, DebugOverlayState, TerrainEditState, WorldChunks, YieldAccumulator,
};
use us_core::ChunkPos;
use us_mesh::mesh_terrain_with_offset;
use us_voxel::{raycast_terrain, PaddedChunkArray};
use us_worldgen::gen_test_chunk;
use viewmodel::{
    build_viewmodel_tool_mesh, first_person_camera_system, viewmodel_update_system,
    FirstPersonCamera, ViewModelTool,
};

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
                    first_person_camera_system,
                    viewmodel_update_system,
                    character_animation_system,
                    fly_camera_system,
                    terrain_interaction_system,
                    particle_update_system,
                    hud_update_system,
                )
                    .after(us_game::player_reconciliation_system),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn setup_world_renderer(
    mut commands: Commands,
    mut world_chunks: ResMut<WorldChunks>,
    mut render_entities: ResMut<ChunkRenderEntities>,
    mut debug_overlay: ResMut<DebugOverlayState>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut server: Option<ResMut<us_game::LocalServer>>,
    mut physical_state: Option<ResMut<us_game::PlayerPhysicalState>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        info!("Assets<Mesh>/Assets<StandardMaterial> not available (headless mode). Skipping presentation mesh spawn.");
        return;
    };
    info!("Setting up Understrata terrain renderer (M1/M2)...");

    // 1. Directional Sun Light (golden-hour sunlight matching mockups)
    commands.spawn((
        DirectionalLight {
            illuminance: 14_000.0,
            shadow_maps_enabled: true,
            color: Color::srgb(1.0, 0.96, 0.88),
            ..default()
        },
        Transform::from_xyz(80.0, 120.0, 60.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // 2. Shared Low-Poly Terrain Materials (dirt, stone, and grass with Repeat + Nearest)
    static DIRT_PNG_BYTES: &[u8] = include_bytes!("../../../assets/textures/terrain/dirt.png");
    static STONE_PNG_BYTES: &[u8] = include_bytes!("../../../assets/textures/terrain/stone.png");
    static GRASS_PNG_BYTES: &[u8] = include_bytes!("../../../assets/textures/terrain/grass.png");

    let mut load_tex = |bytes: &'static [u8]| -> Option<Handle<Image>> {
        images.as_mut().and_then(|imgs| {
            Image::from_buffer(
                bytes,
                bevy::image::ImageType::Extension("png"),
                bevy::image::CompressedImageFormats::empty(),
                true,
                bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
                    address_mode_u: bevy::image::ImageAddressMode::Repeat,
                    address_mode_v: bevy::image::ImageAddressMode::Repeat,
                    address_mode_w: bevy::image::ImageAddressMode::Repeat,
                    mag_filter: bevy::image::ImageFilterMode::Nearest,
                    min_filter: bevy::image::ImageFilterMode::Nearest,
                    mipmap_filter: bevy::image::ImageFilterMode::Nearest,
                    ..default()
                }),
                bevy::asset::RenderAssetUsages::default(),
            )
            .ok()
            .map(|img| imgs.add(img))
        })
    };

    let dirt_texture = load_tex(DIRT_PNG_BYTES);
    let stone_texture = load_tex(STONE_PNG_BYTES);
    let grass_texture = load_tex(GRASS_PNG_BYTES);

    let dirt_material = materials.add(StandardMaterial {
        base_color_texture: dirt_texture,
        perceptual_roughness: 0.92,
        reflectance: 0.08,
        cull_mode: None,
        ..default()
    });

    let stone_material = materials.add(StandardMaterial {
        base_color_texture: stone_texture,
        perceptual_roughness: 0.90,
        reflectance: 0.12,
        cull_mode: None,
        ..default()
    });

    let grass_material = materials.add(StandardMaterial {
        base_color_texture: grass_texture,
        perceptual_roughness: 0.85,
        reflectance: 0.06,
        cull_mode: None,
        ..default()
    });

    render_entities.dirt_material = dirt_material;
    render_entities.stone_material = stone_material;
    render_entities.grass_material = grass_material;

    // 3. Pre-generate chunks with a 1-chunk border for seamless neighbor sampling
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

    // 4. Mesh and spawn active chunks
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

                let min_pos = pos.min_world().0;
                let mesh_data = mesh_terrain_with_offset(&padded, min_pos);
                if !mesh_data.is_empty() {
                    total_triangles += mesh_data.triangle_count();
                    chunk_count += 1;

                    let sub_buffers = split_terrain_buffers_by_material(&mesh_data);
                    let mut mat_map = std::collections::HashMap::new();

                    for (mat_id, buf) in sub_buffers {
                        if buf.is_empty() {
                            continue;
                        }
                        let bevy_mesh = terrain_buffers_to_bevy_mesh(&buf);
                        let mesh_handle = meshes.add(bevy_mesh);
                        let mat_handle = render_entities.material_for(mat_id);

                        let entity = commands
                            .spawn((
                                Mesh3d(mesh_handle.clone()),
                                MeshMaterial3d(mat_handle),
                                Transform::from_translation(min_pos),
                            ))
                            .id();

                        mat_map.insert(mat_id, (entity, mesh_handle));
                    }

                    render_entities.entities.insert(pos, mat_map);
                }
            }
        }
    }

    world_chunks.chunks = chunks.clone();
    debug_overlay.total_triangles = total_triangles;
    debug_overlay.chunk_count = chunk_count;

    // 5. Calculate player spawn point on terrain surface
    let spawn_ground_pos = if let Some(hit) =
        raycast_terrain(&chunks, Vec3::new(0.0, 50.0, 30.0), Vec3::NEG_Y, 60.0)
    {
        hit.position + Vec3::new(0.0, 0.05, 0.0)
    } else {
        Vec3::new(0.0, 16.0, 30.0)
    };

    let eye_pos = spawn_ground_pos + Vec3::new(0.0, 1.65, 0.0);
    let target = spawn_ground_pos + Vec3::new(0.0, 1.5, -20.0);
    let dir = (target - eye_pos).normalize();
    let pitch = dir.y.asin();
    let yaw = dir.x.atan2(-dir.z);

    // 6. Camera entity (First-person eye level, near plane 0.05m to prevent tool/ground clipping)
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            near: 0.05,
            ..default()
        }),
        AmbientLight {
            color: Color::srgb(0.65, 0.76, 0.90),
            brightness: 2000.0,
            ..default()
        },
        Transform::from_translation(eye_pos).looking_at(target, Vec3::Y),
        FlyCamera {
            pitch,
            yaw,
            ..default()
        },
        FirstPersonCamera::default(),
    ));

    // 7. Dedicated LocalPlayer entity (separate from camera to avoid transform stomping)
    commands.spawn((
        Transform::from_translation(spawn_ground_pos),
        us_game::LocalPlayer,
    ));

    // 8. Spawn 3D Pickaxe Viewmodel (RND-009)
    let tool_mesh = meshes.add(build_viewmodel_tool_mesh());
    let tool_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 1.0),
        perceptual_roughness: 0.35,
        reflectance: 0.25,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(tool_mesh),
        MeshMaterial3d(tool_mat),
        Transform::from_xyz(0.22, -0.15, -0.38),
        ViewModelTool::default(),
    ));

    // 9. Spawn articulated 3D Character Model (visible in 3rd person)
    spawn_character_model(&mut commands, &mut meshes, &mut materials, spawn_ground_pos);

    // 10. Synchronize client physical state & authoritative local server
    if let Some(ps) = physical_state.as_mut() {
        ps.position = spawn_ground_pos;
        ps.eye_height = 1.65;
        ps.on_ground = true;
    }

    if let Some(srv) = server.as_mut() {
        srv.chunks = chunks;
        srv.player_state.position = spawn_ground_pos;
        srv.player_state.on_ground = true;
    }

    info!(
        "Spawned {} terrain chunks with a total of {} flat-shaded triangles. Player spawned at {:?}.",
        chunk_count, total_triangles, spawn_ground_pos
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_mesh::mesh_terrain;

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
