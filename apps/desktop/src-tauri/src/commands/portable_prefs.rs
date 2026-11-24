//! IPC for the portable preferences store (#1433). Thin: the allowlist,
//! validation and merge rules live in `crate::portable_prefs`.

use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager, State};

use crate::db::now_ms;
use crate::error::{AppError, AppResult};
use crate::portable_prefs::PortablePrefsStore;

fn store(app: &AppHandle) -> AppResult<State<'_, PortablePrefsStore>> {
    app.try_state::<PortablePrefsStore>()
        .ok_or_else(|| AppError::Storage("portable preferences store unavailable".into()))
}

fn snapshot(store: &PortablePrefsStore) -> AppResult<Value> {
    let (values, migrated) = store.values()?;
    Ok(json!({ "values": values, "migrated": migrated }))
}

fn object(values: Value) -> AppResult<Map<String, Value>> {
    match values {
        Value::Object(map) => Ok(map),
        _ => Err(AppError::Validation(
            "preferences must be a JSON object".into(),
        )),
    }
}

#[tauri::command]
pub async fn portable_prefs_get(app: AppHandle) -> AppResult<Value> {
    let store = store(&app)?;
    snapshot(&store)
}

#[tauri::command]
pub async fn portable_prefs_set(app: AppHandle, values: Value) -> AppResult<Value> {
    store(&app)?.set(&object(values)?, now_ms())?;
    Ok(json!({ "success": true }))
}

#[tauri::command]
pub async fn portable_prefs_migrate(app: AppHandle, values: Value) -> AppResult<Value> {
    let store = store(&app)?;
    store.migrate(&object(values)?)?;
    snapshot(&store)
}
