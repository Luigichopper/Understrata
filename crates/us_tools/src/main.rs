use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use us_content::ContentRegistry;

#[derive(Parser, Debug)]
#[command(name = "us_tools", about = "Understrata developer and build tools")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Validates all content data files in assets/data
    ValidateContent {
        #[arg(short, long, default_value = "assets/data")]
        data_dir: PathBuf,
    },
    /// Generates and previews a test chunk with worldgen statistics
    WorldgenPreview {
        #[arg(short, long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value = "0,0,0")]
        chunk: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ValidateContent { data_dir } => {
            println!("Validating content in {}...", data_dir.display());
            let mut registry = ContentRegistry::new();
            registry.load_from_dir(&data_dir)?;
            registry.validate()?;
            println!(
                "Content validation successful! Loaded {} materials, {} blocks, {} items.",
                registry.materials.len(),
                registry.blocks.len(),
                registry.items.len()
            );
        }
        Commands::WorldgenPreview { seed, chunk } => {
            let coords: Vec<i32> = chunk
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            let pos = if coords.len() == 3 {
                us_core::ChunkPos::new(coords[0], coords[1], coords[2])
            } else {
                us_core::ChunkPos::new(0, 0, 0)
            };

            println!(
                "Generating worldgen preview for chunk ({}, {}, {}) with seed {}...",
                pos.0.x, pos.0.y, pos.0.z, seed
            );

            let terrain_chunk = us_worldgen::gen_test_chunk(seed, pos);
            let padded = us_voxel::PaddedChunkArray::from_chunk_and_neighbors(
                &terrain_chunk,
                |target_chunk, _x, _y, _z| {
                    let neighbor = us_worldgen::gen_test_chunk(seed, target_chunk);
                    (neighbor.sdf.get(0, 0, 0), neighbor.mat.get(0, 0, 0))
                },
            );

            let mesh = us_mesh::mesh_terrain(&padded);
            println!(
                "Worldgen preview generated successfully! Mesh triangles: {}, vertices: {}, uniform sdf: {}",
                mesh.triangle_count(),
                mesh.vertex_count(),
                terrain_chunk.sdf.is_uniform()
            );
        }
    }

    Ok(())
}
