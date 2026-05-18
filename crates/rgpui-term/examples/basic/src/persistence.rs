//! Persistence module for sidebar state.
//!
//! Handles loading and saving of favorites, recent directories, and workspace
//! layouts to disk using JSON format under `~/.config/gpui-term/`.

use crate::sidebar_state::{FavoritesData, RecentData, WorkspaceLayout};
use rgpui::private::serde_json;
use std::fs;
use std::path::PathBuf;

/// Returns the base config directory for gpui-term.
fn config_dir() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("gpui-term");
    path
}

/// Ensures the config directory and subdirectories exist.
fn ensure_dirs() {
    let base = config_dir();
    let _ = fs::create_dir_all(&base);
    let _ = fs::create_dir_all(base.join("workspaces"));
}

// ---------------------------------------------------------------------------
// Favorites
// ---------------------------------------------------------------------------

fn favorites_path() -> PathBuf {
    config_dir().join("favorites.json")
}

pub fn load_favorites() -> FavoritesData {
    let path = favorites_path();
    if !path.exists() {
        return FavoritesData::default();
    }
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(e) => {
            log::warn!("failed to read favorites: {e}");
            FavoritesData::default()
        }
    }
}

pub fn save_favorites(data: &FavoritesData) {
    ensure_dirs();
    let path = favorites_path();
    match serde_json::to_string_pretty(data) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                log::warn!("failed to write favorites: {e}");
            }
        }
        Err(e) => log::warn!("failed to serialize favorites: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Recent directories
// ---------------------------------------------------------------------------

fn recent_path() -> PathBuf {
    config_dir().join("recent.json")
}

pub fn load_recent() -> RecentData {
    let path = recent_path();
    if !path.exists() {
        return RecentData::default();
    }
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(e) => {
            log::warn!("failed to read recent dirs: {e}");
            RecentData::default()
        }
    }
}

pub fn save_recent(data: &RecentData) {
    ensure_dirs();
    let path = recent_path();
    match serde_json::to_string_pretty(data) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                log::warn!("failed to write recent dirs: {e}");
            }
        }
        Err(e) => log::warn!("failed to serialize recent dirs: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Workspaces
// ---------------------------------------------------------------------------

fn workspaces_dir() -> PathBuf {
    config_dir().join("workspaces")
}

/// Sanitize a workspace name for use as a filename.
fn workspace_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.json", sanitized)
}

pub fn load_workspaces() -> Vec<WorkspaceLayout> {
    let dir = workspaces_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("failed to read workspaces dir: {e}");
            return Vec::new();
        }
    };

    let mut workspaces = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(data) = fs::read_to_string(&path) {
                if let Ok(ws) = serde_json::from_str::<WorkspaceLayout>(&data) {
                    workspaces.push(ws);
                }
            }
        }
    }
    workspaces.sort_by(|a, b| a.name.cmp(&b.name));
    workspaces
}

pub fn save_workspace(workspace: &WorkspaceLayout) {
    ensure_dirs();
    let dir = workspaces_dir();
    let filename = workspace_filename(&workspace.name);
    let path = dir.join(filename);
    match serde_json::to_string_pretty(workspace) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                log::warn!("failed to write workspace: {e}");
            }
        }
        Err(e) => log::warn!("failed to serialize workspace: {e}"),
    }
}

pub fn delete_workspace(name: &str) {
    let dir = workspaces_dir();
    let filename = workspace_filename(name);
    let path = dir.join(filename);
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
}
