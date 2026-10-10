use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContentError {
    #[error("Failed to read content file: {path}: {source}")]
    IoError {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse RON in {path}: {source}")]
    ParseError {
        path: String,
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    #[error("Duplicate ID '{id}' found in category '{category}'")]
    DuplicateId { category: String, id: String },
    #[error("Broken reference: '{from_id}' references unknown {target_category} '{target_id}'")]
    BrokenReference {
        from_id: String,
        target_category: String,
        target_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TexturesTriple {
    pub top: String,
    pub side: String,
    pub bottom: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialDef {
    pub id: String,
    pub name_key: String,
    pub hardness: f32,
    pub min_tool_tier: i32,
    pub yield_item: String,
    pub yield_per_m3: f32,
    pub textures: TexturesTriple,
    pub sound_set: String,
    pub priority: u8,
    pub emission: Option<[f32; 3]>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockDrop {
    pub item: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockDef {
    pub id: String,
    pub name_key: String,
    pub hardness: f32,
    pub min_tool_tier: i32,
    pub opaque: bool,
    pub emits_light: Option<u8>,
    pub light_filter: u8,
    pub textures: TexturesTriple,
    #[serde(default)]
    pub shapes: Vec<String>,
    pub sound_set: String,
    #[serde(default)]
    pub drops: Vec<BlockDrop>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub flammable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemDef {
    pub id: String,
    pub name_key: String,
    #[serde(default = "default_max_stack")]
    pub max_stack: u32,
    pub category: String,
    pub tool_tier: Option<i32>,
    pub tool_power: Option<f32>,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_max_stack() -> u32 {
    99
}

#[derive(Debug, Default, Clone)]
pub struct ContentRegistry {
    pub materials: HashMap<String, MaterialDef>,
    pub blocks: HashMap<String, BlockDef>,
    pub items: HashMap<String, ItemDef>,
}

impl ContentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_material(&mut self, mat: MaterialDef) -> Result<(), ContentError> {
        if self.materials.contains_key(&mat.id) {
            return Err(ContentError::DuplicateId {
                category: "material".to_string(),
                id: mat.id,
            });
        }
        self.materials.insert(mat.id.clone(), mat);
        Ok(())
    }

    pub fn register_block(&mut self, block: BlockDef) -> Result<(), ContentError> {
        if self.blocks.contains_key(&block.id) {
            return Err(ContentError::DuplicateId {
                category: "block".to_string(),
                id: block.id,
            });
        }
        self.blocks.insert(block.id.clone(), block);
        Ok(())
    }

    pub fn register_item(&mut self, item: ItemDef) -> Result<(), ContentError> {
        if self.items.contains_key(&item.id) {
            return Err(ContentError::DuplicateId {
                category: "item".to_string(),
                id: item.id,
            });
        }
        self.items.insert(item.id.clone(), item);
        Ok(())
    }

    /// Validates cross-references across items, blocks, and materials.
    pub fn validate(&self) -> Result<(), ContentError> {
        // 1. Verify material yield_items exist in items
        for mat in self.materials.values() {
            if !mat.yield_item.is_empty() && !self.items.contains_key(&mat.yield_item) {
                return Err(ContentError::BrokenReference {
                    from_id: mat.id.clone(),
                    target_category: "item".to_string(),
                    target_id: mat.yield_item.clone(),
                });
            }
        }

        // 2. Verify block drops exist in items
        for block in self.blocks.values() {
            for drop in &block.drops {
                if !self.items.contains_key(&drop.item) {
                    return Err(ContentError::BrokenReference {
                        from_id: block.id.clone(),
                        target_category: "item".to_string(),
                        target_id: drop.item.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Loads all data from `assets/data/` directory.
    pub fn load_from_dir<P: AsRef<Path>>(&mut self, base_dir: P) -> Result<(), ContentError> {
        let base = base_dir.as_ref();

        let materials_dir = base.join("materials");
        if materials_dir.exists() {
            for entry in std::fs::read_dir(&materials_dir).map_err(|e| ContentError::IoError {
                path: materials_dir.display().to_string(),
                source: e,
            })? {
                let entry = entry.map_err(|e| ContentError::IoError {
                    path: materials_dir.display().to_string(),
                    source: e,
                })?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ron") {
                    let content =
                        std::fs::read_to_string(&path).map_err(|e| ContentError::IoError {
                            path: path.display().to_string(),
                            source: e,
                        })?;
                    let mat: MaterialDef =
                        ron::from_str(&content).map_err(|e| ContentError::ParseError {
                            path: path.display().to_string(),
                            source: Box::new(e),
                        })?;
                    self.register_material(mat)?;
                }
            }
        }

        let blocks_dir = base.join("blocks");
        if blocks_dir.exists() {
            for entry in std::fs::read_dir(&blocks_dir).map_err(|e| ContentError::IoError {
                path: blocks_dir.display().to_string(),
                source: e,
            })? {
                let entry = entry.map_err(|e| ContentError::IoError {
                    path: blocks_dir.display().to_string(),
                    source: e,
                })?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ron") {
                    let content =
                        std::fs::read_to_string(&path).map_err(|e| ContentError::IoError {
                            path: path.display().to_string(),
                            source: e,
                        })?;
                    let block: BlockDef =
                        ron::from_str(&content).map_err(|e| ContentError::ParseError {
                            path: path.display().to_string(),
                            source: Box::new(e),
                        })?;
                    self.register_block(block)?;
                }
            }
        }

        let items_dir = base.join("items");
        if items_dir.exists() {
            for entry in std::fs::read_dir(&items_dir).map_err(|e| ContentError::IoError {
                path: items_dir.display().to_string(),
                source: e,
            })? {
                let entry = entry.map_err(|e| ContentError::IoError {
                    path: items_dir.display().to_string(),
                    source: e,
                })?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ron") {
                    let content =
                        std::fs::read_to_string(&path).map_err(|e| ContentError::IoError {
                            path: path.display().to_string(),
                            source: e,
                        })?;
                    let item: ItemDef =
                        ron::from_str(&content).map_err(|e| ContentError::ParseError {
                            path: path.display().to_string(),
                            source: Box::new(e),
                        })?;
                    self.register_item(item)?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_validation_catches_broken_ref() {
        let mut reg = ContentRegistry::new();
        reg.register_material(MaterialDef {
            id: "us:stone".into(),
            name_key: "mat.stone".into(),
            hardness: 1.0,
            min_tool_tier: 0,
            yield_item: "us:stone_item".into(),
            yield_per_m3: 1.0,
            textures: TexturesTriple {
                top: "stone".into(),
                side: "stone".into(),
                bottom: "stone".into(),
            },
            sound_set: "stone".into(),
            priority: 10,
            emission: None,
            tags: vec![],
        })
        .unwrap();

        assert!(matches!(
            reg.validate(),
            Err(ContentError::BrokenReference { .. })
        ));

        // Register the item and verify validation passes
        reg.register_item(ItemDef {
            id: "us:stone_item".into(),
            name_key: "item.stone".into(),
            max_stack: 99,
            category: "material".into(),
            tool_tier: None,
            tool_power: None,
            tags: vec![],
        })
        .unwrap();

        assert!(reg.validate().is_ok());
    }
}
