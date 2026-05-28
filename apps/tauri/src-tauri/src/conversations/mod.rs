use rusqlite::{Connection, Result, params};
use serde_json::{json, Value};
use parking_lot::Mutex;
use tauri::{AppHandle, Manager};

use crate::db::{run_migrations, Migration};

pub struct ConversationDb(pub Mutex<Connection>);

impl ConversationDb {
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
            name: "create_conversations_and_messages",
            up: |conn| {
                conn.execute_batch(
                    "PRAGMA journal_mode=WAL;
                    CREATE TABLE IF NOT EXISTS conversations (
                        id TEXT PRIMARY KEY,
                        title TEXT NOT NULL,
                        created_at INTEGER NOT NULL
                    );
                    CREATE TABLE IF NOT EXISTS messages (
                        id TEXT PRIMARY KEY,
                        conversation_id TEXT NOT NULL REFERENCES conversations(id),
                        role TEXT NOT NULL,
                        content TEXT NOT NULL,
                        created_at INTEGER NOT NULL
                    );
                    CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id, created_at);",
                )
            },
        },
    ];

    pub fn open(app: &AppHandle) -> Result<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .expect("no app data dir");
        std::fs::create_dir_all(&data_dir).ok();
        let conn = Connection::open(data_dir.join("conversations.db"))?;
        run_migrations(&conn, Self::MIGRATIONS)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn clear_all(&self) {
        let conn = self.0.lock();
        conn.execute_batch("DELETE FROM messages; DELETE FROM conversations;").ok();
    }
}

impl crate::data_store::DataStore for ConversationDb {
    fn key(&self) -> &'static str {
        "conversations"
    }

    fn export(&self) -> Value {
        let conn = self.0.lock();
        let conversations = query_rows(
            &conn,
            "SELECT id, title, created_at FROM conversations ORDER BY created_at",
            |row| {
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "createdAt": row.get::<_, i64>(2)?,
                }))
            },
        );
        let messages = query_rows(
            &conn,
            "SELECT id, conversation_id, role, content, created_at FROM messages ORDER BY created_at",
            |row| {
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "conversationId": row.get::<_, String>(1)?,
                    "role": row.get::<_, String>(2)?,
                    "content": row.get::<_, String>(3)?,
                    "createdAt": row.get::<_, i64>(4)?,
                }))
            },
        );
        json!({ "conversations": conversations, "messages": messages })
    }

    fn import(&self, data: &Value) -> Result<usize, String> {
        let conversations = data.get("conversations").and_then(|v| v.as_array());
        let messages = data.get("messages").and_then(|v| v.as_array());
        let conn = self.0.lock();
        conn.execute_batch("DELETE FROM messages; DELETE FROM conversations;")
            .map_err(|e| e.to_string())?;

        let mut count = 0;
        if let Some(rows) = conversations {
            for c in rows {
                conn.execute(
                    "INSERT INTO conversations (id, title, created_at) VALUES (?1, ?2, ?3)",
                    params![
                        str_field(c, "id"),
                        str_field(c, "title"),
                        c.get("createdAt").and_then(|v| v.as_i64()).unwrap_or_default(),
                    ],
                )
                .map_err(|e| e.to_string())?;
                count += 1;
            }
        }
        if let Some(rows) = messages {
            for m in rows {
                conn.execute(
                    "INSERT INTO messages (id, conversation_id, role, content, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        str_field(m, "id"),
                        str_field(m, "conversationId"),
                        str_field(m, "role"),
                        str_field(m, "content"),
                        m.get("createdAt").and_then(|v| v.as_i64()).unwrap_or_default(),
                    ],
                )
                .map_err(|e| e.to_string())?;
                count += 1;
            }
        }
        Ok(count)
    }
}

fn query_rows(
    conn: &Connection,
    sql: &str,
    map: impl Fn(&rusqlite::Row) -> Result<Value>,
) -> Vec<Value> {
    conn.prepare(sql)
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| map(row))
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn get_or_create(app: &AppHandle) -> Value {
    let db = app.state::<ConversationDb>();
    let conn = db.0.lock();
    let id = "default";
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM conversations WHERE id = ?1",
            params![id],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !exists {
        let ts = now_ms();
        conn.execute(
            "INSERT INTO conversations (id, title, created_at) VALUES (?1, ?2, ?3)",
            params![id, "AI Chat", ts],
        )
        .ok();
    }

    let (title, created_at): (String, i64) = conn
        .query_row(
            "SELECT title, created_at FROM conversations WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or_else(|_| ("AI Chat".to_string(), now_ms()));

    json!({ "id": id, "title": title, "createdAt": created_at })
}

pub fn load_messages(app: &AppHandle, conversation_id: &str) -> Value {
    let db = app.state::<ConversationDb>();
    let conn = db.0.lock();
    let mut stmt = match conn.prepare(
        "SELECT id, role, content, created_at FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC"
    ) {
        Ok(s) => s,
        Err(_) => return json!([]),
    };

    let messages: Vec<Value> = match stmt.query_map(params![conversation_id], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "conversationId": conversation_id,
            "role": row.get::<_, String>(1)?,
            "content": row.get::<_, String>(2)?,
            "createdAt": row.get::<_, i64>(3)?,
        }))
    }) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(_) => vec![],
    };
    json!(messages)
}

pub fn save_message(app: &AppHandle, req: &Value) -> Value {
    let conversation_id = req.get("conversationId").and_then(|v| v.as_str()).unwrap_or("default");
    let role = req.get("role").and_then(|v| v.as_str()).unwrap_or("user");
    let content = req.get("content").and_then(|v| v.as_str()).unwrap_or("");

    let db = app.state::<ConversationDb>();
    let conn = db.0.lock();
    let id = uuid::Uuid::new_v4().to_string();
    let ts = now_ms();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, conversation_id, role, content, ts],
    ).ok();

    json!({ "id": id, "conversationId": conversation_id, "role": role, "content": content, "createdAt": ts })
}

#[cfg(test)]
mod test;
