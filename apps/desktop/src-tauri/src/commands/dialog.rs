use serde::Deserialize;
use tauri::AppHandle;

#[derive(Deserialize)]
pub struct DialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[tauri::command]
pub async fn dialog_open_files(
    app: AppHandle,
    title: Option<String>,
    filters: Option<Vec<DialogFilter>>,
) -> Vec<String> {
    use tauri_plugin_dialog::{DialogExt, FilePath};

    let mut builder = app.dialog().file();
    if let Some(t) = title {
        builder = builder.set_title(&t);
    }
    if let Some(fs) = filters {
        for f in fs {
            builder = builder.add_filter(
                &f.name,
                &f.extensions.iter().map(String::as_str).collect::<Vec<_>>(),
            );
        }
    }

    builder
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .map(|p| match p {
            FilePath::Path(pb) => pb.to_string_lossy().into_owned(),
            FilePath::Url(u) => u.to_string(),
        })
        .collect()
}
