use crate::error::Result;
use crate::model::*;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn memory() -> Result<Db> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Db> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS tasks(id INTEGER PRIMARY KEY AUTOINCREMENT, position INTEGER NOT NULL, json TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS history(
               id INTEGER PRIMARY KEY AUTOINCREMENT, url TEXT NOT NULL, media_id TEXT, title TEXT NOT NULL,
               site TEXT NOT NULL, path TEXT, size INTEGER, thumbnail TEXT, finished_at INTEGER NOT NULL);
             CREATE INDEX IF NOT EXISTS history_url ON history(url);
             CREATE INDEX IF NOT EXISTS history_media ON history(site, media_id);",
        )?;
        Ok(Db { conn: Mutex::new(conn) })
    }

    fn c(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.c().query_row("SELECT value FROM kv WHERE key=?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        self.c().execute("INSERT INTO kv(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [key, value])?;
        Ok(())
    }

    // --- File d'attente ---

    pub fn tasks_load(&self) -> Result<Vec<Task>> {
        let c = self.c();
        let mut st = c.prepare("SELECT id, position, json FROM tasks ORDER BY position, id")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))?;
        let mut v = Vec::new();
        for row in rows {
            let (id, pos, json) = row?;
            if let Ok(mut t) = serde_json::from_str::<Task>(&json) {
                t.id = id;
                t.position = pos;
                v.push(t);
            }
        }
        Ok(v)
    }

    pub fn task_insert(&self, t: &mut Task) -> Result<()> {
        let c = self.c();
        c.execute("INSERT INTO tasks(position, json) VALUES(?1, '{}')", [t.position])?;
        t.id = c.last_insert_rowid();
        c.execute("UPDATE tasks SET json=?1 WHERE id=?2", params![serde_json::to_string(t)?, t.id])?;
        Ok(())
    }

    pub fn task_save(&self, t: &Task) -> Result<()> {
        self.c().execute("UPDATE tasks SET position=?1, json=?2 WHERE id=?3", params![t.position, serde_json::to_string(t)?, t.id])?;
        Ok(())
    }

    pub fn task_delete(&self, id: i64) -> Result<()> {
        self.c().execute("DELETE FROM tasks WHERE id=?1", [id])?;
        Ok(())
    }

    // --- Historique ---

    pub fn history_add(&self, h: &HistoryItem) -> Result<i64> {
        let c = self.c();
        c.execute(
            "INSERT INTO history(url, media_id, title, site, path, size, thumbnail, finished_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![h.url, h.media_id, h.title, h.site, h.path, h.size.map(|s| s as i64), h.thumbnail, h.finished_at],
        )?;
        Ok(c.last_insert_rowid())
    }

    fn row_to_item(r: &rusqlite::Row) -> rusqlite::Result<HistoryItem> {
        let path: Option<String> = r.get(5)?;
        Ok(HistoryItem {
            id: r.get(0)?,
            url: r.get(1)?,
            media_id: r.get(2)?,
            title: r.get(3)?,
            site: r.get(4)?,
            exists: path.as_deref().is_some_and(|p| Path::new(p).exists()),
            path,
            size: r.get::<_, Option<i64>>(6)?.map(|s| s as u64),
            thumbnail: r.get(7)?,
            finished_at: r.get(8)?,
        })
    }

    const COLS: &'static str = "id, url, media_id, title, site, path, size, thumbnail, finished_at";

    /// Recherche dans le titre, le site et l'URL. `sort` : date, title, site, size ; `desc` inverse l'ordre.
    pub fn history_list(&self, query: &str, sort: &str, desc: bool, limit: i64) -> Result<Vec<HistoryItem>> {
        let col = match sort {
            "title" => "title COLLATE NOCASE",
            "site" => "site COLLATE NOCASE",
            "size" => "size",
            _ => "finished_at",
        };
        let sql = format!(
            "SELECT {} FROM history WHERE (?1='' OR title LIKE ?2 ESCAPE '\\' OR site LIKE ?2 ESCAPE '\\' OR url LIKE ?2 ESCAPE '\\') ORDER BY {col} {} LIMIT ?3",
            Self::COLS,
            if desc { "DESC" } else { "ASC" }
        );
        let like = format!("%{}%", query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let c = self.c();
        let mut st = c.prepare(&sql)?;
        let rows = st.query_map(params![query, like, limit], Self::row_to_item)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn history_get(&self, id: i64) -> Result<Option<HistoryItem>> {
        Ok(self.c().query_row(&format!("SELECT {} FROM history WHERE id=?1", Self::COLS), [id], Self::row_to_item).optional()?)
    }

    /// Doublon : même URL, ou même identifiant de média sur le même site.
    pub fn history_find_duplicate(&self, url: &str, site: &str, media_id: Option<&str>) -> Result<Option<HistoryItem>> {
        let c = self.c();
        let sql = format!(
            "SELECT {} FROM history WHERE url=?1 OR (?3 IS NOT NULL AND media_id=?3 AND site=?2) ORDER BY finished_at DESC LIMIT 1",
            Self::COLS
        );
        Ok(c.query_row(&sql, params![url, site, media_id], Self::row_to_item).optional()?)
    }

    pub fn history_delete(&self, ids: &[i64]) -> Result<()> {
        let c = self.c();
        for id in ids {
            c.execute("DELETE FROM history WHERE id=?1", [id])?;
        }
        Ok(())
    }

    pub fn history_clear(&self) -> Result<()> {
        self.c().execute("DELETE FROM history", [])?;
        Ok(())
    }

    /// Efface les entrées plus vieilles que la durée choisie.
    pub fn history_prune(&self, r: HistoryRetention) -> Result<usize> {
        let days = match r {
            HistoryRetention::Week => 7,
            HistoryRetention::Month => 30,
            HistoryRetention::ThreeMonths => 90,
            HistoryRetention::SixMonths => 182,
            HistoryRetention::Year => 365,
            HistoryRetention::Never => return Ok(0),
        };
        let limit = crate::util::now() - days * 86_400;
        Ok(self.c().execute("DELETE FROM history WHERE finished_at < ?1", [limit])?)
    }

    pub fn history_remove_missing(&self) -> Result<usize> {
        let all = self.history_list("", "date", true, 1_000_000)?;
        let gone: Vec<i64> = all.iter().filter(|h| h.path.is_some() && !h.exists).map(|h| h.id).collect();
        self.history_delete(&gone)?;
        Ok(gone.len())
    }
}
