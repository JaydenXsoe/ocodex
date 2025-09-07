use crate::memory::{MemorySnapshot, TodoItem};
use std::path::Path;

pub trait WorkspaceManager: Send + Sync {
    fn ensure_project(&self, _project_name: &str, _root: &Path) -> std::io::Result<()> { Ok(()) }
    fn write_agents_md(&self, _root: &Path, _content: &str) -> std::io::Result<()> { Ok(()) }
    fn write_todo_md(&self, _root: &Path, _todo: &[TodoItem]) -> std::io::Result<()> { Ok(()) }
    fn snapshot(&self, _root: &Path) -> std::io::Result<MemorySnapshot> { Ok(MemorySnapshot::default()) }
}

#[derive(Default)]
pub struct NoopWorkspaceManager;

impl WorkspaceManager for NoopWorkspaceManager {}

