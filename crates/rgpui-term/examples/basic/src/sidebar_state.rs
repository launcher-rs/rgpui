//! Sidebar state data structures for Agent Term.
//!
//! This module defines the persistent and transient state used by the sidebar,
//! including favorites, recent directories, workspaces, and section visibility.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A favorite directory entry that can be quickly accessed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FavoriteEntry {
    /// The filesystem path to the favorite directory.
    pub path: PathBuf,
    /// Optional custom label for display (uses directory name if None).
    pub label: Option<String>,
}

impl FavoriteEntry {
    pub fn new(path: PathBuf) -> Self {
        Self { path, label: None }
    }

    pub fn with_label(path: PathBuf, label: impl Into<String>) -> Self {
        Self {
            path,
            label: Some(label.into()),
        }
    }

    /// Returns the display name (label or directory name).
    pub fn display_name(&self) -> String {
        if let Some(label) = &self.label {
            label.clone()
        } else {
            self.path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
        }
    }
}

/// A recently accessed directory entry with timestamp.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecentEntry {
    /// The filesystem path to the directory.
    pub path: PathBuf,
    /// Unix timestamp of last access.
    pub last_opened: u64,
}

impl RecentEntry {
    pub fn new(path: PathBuf) -> Self {
        let last_opened = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self { path, last_opened }
    }

    /// Returns the display name (directory name).
    pub fn display_name(&self) -> String {
        self.path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }
}

/// A saved tab configuration for workspace restoration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedTab {
    /// Tab title.
    pub title: String,
    /// Working directory when tab was saved.
    pub working_dir: Option<PathBuf>,
}

/// A workspace layout that can be saved and restored.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    /// Display name for this workspace.
    pub name: String,
    /// Saved tabs in this workspace.
    pub tabs: Vec<SavedTab>,
    /// Index of the active tab when saved.
    pub active_tab_index: usize,
    /// Sidebar width when saved.
    pub sidebar_width: f32,
}

impl WorkspaceLayout {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tabs: Vec::new(),
            active_tab_index: 0,
            sidebar_width: 250.0,
        }
    }
}

/// Collapsible sections in the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SidebarSection {
    Favorites,
    Projects,
    RecentDirs,
    Workspaces,
}

impl SidebarSection {
    /// Returns all sidebar sections in display order.
    pub fn all() -> &'static [SidebarSection] {
        &[
            SidebarSection::Favorites,
            SidebarSection::Projects,
            SidebarSection::RecentDirs,
            SidebarSection::Workspaces,
        ]
    }

    /// Returns the display title for this section.
    pub fn title(&self) -> &'static str {
        match self {
            SidebarSection::Favorites => "Favorites",
            SidebarSection::Projects => "Projects",
            SidebarSection::RecentDirs => "Recent",
            SidebarSection::Workspaces => "Workspaces",
        }
    }
}

/// Container for favorites data with loading/persistence support.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FavoritesData {
    pub entries: Vec<FavoriteEntry>,
}

impl FavoritesData {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, entry: FavoriteEntry) {
        // Avoid duplicates
        if !self.entries.iter().any(|e| e.path == entry.path) {
            self.entries.push(entry);
        }
    }

    pub fn remove(&mut self, path: &PathBuf) {
        self.entries.retain(|e| &e.path != path);
    }
}

/// Container for recent directories with auto-pruning.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecentData {
    pub entries: Vec<RecentEntry>,
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

fn default_max_entries() -> usize {
    20
}

impl RecentData {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 20,
        }
    }

    /// Add or update a recent entry, moving it to the front.
    pub fn touch(&mut self, path: PathBuf) {
        // Remove existing entry with same path
        self.entries.retain(|e| e.path != path);

        // Add new entry at the beginning
        self.entries.insert(0, RecentEntry::new(path));

        // Prune old entries
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }

    /// Returns entries sorted by last_opened (most recent first).
    pub fn sorted(&self) -> Vec<&RecentEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));
        sorted
    }
}
