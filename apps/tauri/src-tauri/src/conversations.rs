use rusqlite::{Connection, Result, params};
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

pub struct ConversationDb(pub Mutex<Connection>);

impl ConversationDb {
    pub fn open(app: &AppHandle) -> Result<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .expect("no app data dir");
        std::fs::create_dir_all(&data_dir).ok();
        let conn = Connection::open(data_dir.join("conversations.db"))?;
        conn.execute_batch("
            PRAGMA journal_mode=WAL;
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
            CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id, created_at);
        ")?;
        Ok(Self(Mutex::new(conn)))
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn get_or_create(app: &AppHandle) -> Value {
    let db = app.state::<ConversationDb>();
    let conn = db.0.lock().unwrap();
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
    let conn = db.0.lock().unwrap();
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
    let conn = db.0.lock().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let ts = now_ms();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, conversation_id, role, content, ts],
    ).ok();

    json!({ "id": id, "conversationId": conversation_id, "role": role, "content": content, "createdAt": ts })
}
