#[cfg(feature = "sqlite-memory")]
use rusqlite::{params, Connection};

#[cfg(feature = "sqlite-memory")]
pub struct SqliteStore { path: String }

#[cfg(feature = "sqlite-memory")]
impl SqliteStore {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS memory_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS blackboard_current (
                task_id INTEGER PRIMARY KEY,
                ts TEXT NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS blackboard_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER,
                ts TEXT NOT NULL,
                data TEXT NOT NULL
            );
            "#,
        )?;
        Ok(Self { path: path.to_string() })
    }

    pub fn upsert_memory(&self, snapshot: &crate::memory::MemorySnapshot) -> anyhow::Result<()> {
        let ts = chrono::Utc::now().to_rfc3339();
        let conn = Connection::open(&self.path)?;
        let s = serde_json::to_string(snapshot).unwrap_or_else(|_| "{}".into());
        conn.execute(
            "INSERT INTO memory_snapshots (ts, data) VALUES (?1, ?2)",
            params![ts, s],
        )?;
        Ok(())
    }

    pub fn latest_memory(&self) -> anyhow::Result<Option<crate::memory::MemorySnapshot>> {
        let conn = Connection::open(&self.path)?;
        let mut stmt = conn.prepare("SELECT data FROM memory_snapshots ORDER BY id DESC LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            Ok(serde_json::from_str::<crate::memory::MemorySnapshot>(&s).ok())
        } else { Ok(None) }
    }

    pub fn upsert_blackboard(&self, task_id: usize, data: &serde_json::Value) -> anyhow::Result<()> {
        let ts = chrono::Utc::now().to_rfc3339();
        let s = serde_json::to_string(data)?;
        let conn = Connection::open(&self.path)?;
        conn.execute(
            "INSERT INTO blackboard_current (task_id, ts, data) VALUES (?1, ?2, ?3)
             ON CONFLICT(task_id) DO UPDATE SET ts=excluded.ts, data=excluded.data",
            params![task_id as i64, &ts, &s],
        )?;
        conn.execute(
            "INSERT INTO blackboard_log (task_id, ts, data) VALUES (?1, ?2, ?3)",
            params![task_id as i64, ts, s],
        )?;
        Ok(())
    }

    pub fn all_blackboard(&self) -> anyhow::Result<std::collections::HashMap<usize, serde_json::Value>> {
        let mut out = std::collections::HashMap::new();
        let conn = Connection::open(&self.path)?;
        let mut stmt = conn.prepare("SELECT task_id, data FROM blackboard_current")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let tid: i64 = row.get(0)?;
            let s: String = row.get(1)?;
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) { out.insert(tid as usize, v); }
        }
        Ok(out)
    }
}

