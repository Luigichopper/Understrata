use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Mesh, PrimitiveTopology};
use us_mesh::{TerrainMeshBuffers, MAT_DIRT, MAT_GRASS, MAT_STONE};

/// Converts engine-agnostic terrain mesh buffers into a Bevy renderable mesh.
pub fn terrain_buffers_to_bevy_mesh(buffers: &TerrainMeshBuffers) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, buffers.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, buffers.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, buffers.uvs.clone());

    // Map material IDs + AO to vertex colors (RGBA) for low-poly shading
    let colors: Vec<[f32; 4]> = buffers
        .materials
        .iter()
        .zip(&buffers.ao)
        .map(|(&mat, &ao)| {
            let base = match mat {
                MAT_GRASS => [0.32, 0.65, 0.22, 1.0], // Vibrant grass green
                MAT_DIRT => [0.45, 0.32, 0.20, 1.0],  // Earthy soil brown
                MAT_STONE => [0.55, 0.54, 0.52, 1.0], // Low-poly cliff stone
                _ => [0.60, 0.60, 0.60, 1.0],
            };
            [base[0] * ao, base[1] * ao, base[2] * ao, base[3]]
        })
        .collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

    mesh
}
