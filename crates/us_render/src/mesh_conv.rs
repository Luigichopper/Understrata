use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Mesh, PrimitiveTopology};
use std::collections::HashMap;
use us_mesh::TerrainMeshBuffers;

/// Splits terrain mesh buffers by material ID so each material can be rendered with its own texture/material.
pub fn split_terrain_buffers_by_material(
    buffers: &TerrainMeshBuffers,
) -> HashMap<u8, TerrainMeshBuffers> {
    let mut map: HashMap<u8, TerrainMeshBuffers> = HashMap::new();
    let num_triangles = buffers.triangle_count();

    for tri in 0..num_triangles {
        let i0 = tri * 3;
        let mat = buffers.materials[i0];
        let sub = map.entry(mat).or_default();

        for i in i0..i0 + 3 {
            sub.positions.push(buffers.positions[i]);
            sub.normals.push(buffers.normals[i]);
            sub.uvs.push(buffers.uvs[i]);
            sub.materials.push(buffers.materials[i]);
            sub.ao.push(buffers.ao[i]);
        }
    }

    map
}

/// Converts engine-agnostic terrain mesh buffers into a Bevy renderable mesh.
pub fn terrain_buffers_to_bevy_mesh(buffers: &TerrainMeshBuffers) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, buffers.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, buffers.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, buffers.uvs.clone());

    // Shading: Ambient Occlusion on vertex colors for faceted depth
    let colors: Vec<[f32; 4]> = buffers.ao.iter().map(|&ao| [ao, ao, ao, 1.0]).collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

    mesh
}
