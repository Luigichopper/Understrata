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
    }

    Ok(())
}
