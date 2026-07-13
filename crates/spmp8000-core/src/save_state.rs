// Save state management for SPMP8000 emulator
//
// Handles serialization and deserialization of emulator state

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Save state header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveStateHeader {
    /// Magic bytes
    pub magic: [u8; 4],
    /// Version
    pub version: u32,
    /// Game name
    pub game_name: String,
    /// Timestamp
    pub timestamp: u64,
    /// CRC32 of the game file
    pub game_crc32: u32,
}

/// Save state data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveState {
    /// Header
    pub header: SaveStateHeader,
    /// Register state (R0-R15, CPSR)
    pub registers: [u32; 17],
    /// Memory snapshot (key regions only)
    pub memory: Vec<MemoryRegionSnapshot>,
    /// API state
    pub api_state: ApiStateSnapshot,
}

/// Memory region snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRegionSnapshot {
    /// Base address
    pub base: u32,
    /// Region data
    pub data: Vec<u8>,
}

/// API state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiStateSnapshot {
    /// Key state
    pub key_state: u32,
    /// Tick count
    pub tick_count: u64,
    /// Framebuffer address
    pub framebuffer_addr: Option<u32>,
}

/// Save state manager
pub struct SaveStateManager {
    /// Save directory
    save_dir: PathBuf,
}

impl SaveStateManager {
    /// Create a new save state manager
    pub fn new(save_dir: PathBuf) -> Self {
        Self { save_dir }
    }

    /// Save state to file
    pub fn save(&self, state: &SaveState, slot: u32) -> Result<(), String> {
        let filename = format!("save_{}.json", slot);
        let path = self.save_dir.join(filename);

        let json = serde_json::to_string_pretty(state)
            .map_err(|e| format!("Failed to serialize save state: {}", e))?;

        std::fs::write(&path, json)
            .map_err(|e| format!("Failed to write save state: {}", e))?;

        log::info!("Saved state to {}", path.display());
        Ok(())
    }

    /// Load state from file
    pub fn load(&self, slot: u32) -> Result<SaveState, String> {
        let filename = format!("save_{}.json", slot);
        let path = self.save_dir.join(filename);

        if !path.exists() {
            return Err(format!("Save file not found: {}", path.display()));
        }

        let json = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read save state: {}", e))?;

        let state: SaveState = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse save state: {}", e))?;

        log::info!("Loaded state from {}", path.display());
        Ok(state)
    }

    /// Check if a save exists for a slot
    pub fn save_exists(&self, slot: u32) -> bool {
        let filename = format!("save_{}.json", slot);
        self.save_dir.join(filename).exists()
    }

    /// Delete a save
    pub fn delete_save(&self, slot: u32) -> Result<(), String> {
        let filename = format!("save_{}.json", slot);
        let path = self.save_dir.join(filename);

        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete save state: {}", e))?;
        }

        Ok(())
    }
}

/// Create a save state header
pub fn create_header(game_name: &str, game_crc32: u32) -> SaveStateHeader {
    SaveStateHeader {
        magic: *b"SPM8",
        version: 1,
        game_name: game_name.to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        game_crc32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_state_roundtrip() {
        let dir = tempdir().unwrap();
        let manager = SaveStateManager::new(dir.path().to_path_buf());

        let state = SaveState {
            header: create_header("TestGame", 0x12345678),
            registers: [0; 17],
            memory: vec![],
            api_state: ApiStateSnapshot {
                key_state: 0,
                tick_count: 0,
                framebuffer_addr: None,
            },
        };

        manager.save(&state, 0).unwrap();
        assert!(manager.save_exists(0));

        let loaded = manager.load(0).unwrap();
        assert_eq!(loaded.header.game_name, "TestGame");
    }

    #[test]
    fn test_save_not_found() {
        let dir = tempdir().unwrap();
        let manager = SaveStateManager::new(dir.path().to_path_buf());

        assert!(!manager.save_exists(0));
        assert!(manager.load(0).is_err());
    }
}
