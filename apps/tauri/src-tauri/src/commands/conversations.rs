use serde_json::Value;
use tauri::AppHandle;

#[tauri::command]
pub fn conversations_get_or_create(app: AppHandle) -> Value {
    crate::conversations::get_or_create(&app)
}

#[tauri::command]
pub fn conversations_load_messages(app: AppHandle, conversation_id: String) -> Value {
    crate::conversations::load_messages(&app, &conversation_id)
}

#[tauri::command]
pub fn conversations_save_message(app: AppHandle, req: Value) -> Value {
    crate::conversations::save_message(&app, &req)
}
