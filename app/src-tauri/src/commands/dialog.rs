//! Diálogos nativos del sistema.

use tauri_plugin_dialog::DialogExt;

/// Muestra el selector de archivos PDF y devuelve las rutas elegidas (vacío si se cancela).
/// `title` es el título de la ventana del selector (ya traducido por la interfaz).
#[tauri::command]
pub async fn pick_pdf_files(app: tauri::AppHandle, title: Option<String>) -> Vec<String> {
    let mut dialog = app.dialog().file().add_filter("PDF", &["pdf"]);
    if let Some(t) = title {
        dialog = dialog.set_title(t);
    }
    // `blocking_*` es seguro aquí: los comandos `async` no se ejecutan en el hilo de la interfaz.
    dialog
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Selector de una imagen PNG o JPEG (marca de agua, fondo). `None` si se cancela.
#[tauri::command]
pub async fn pick_image_file(app: tauri::AppHandle, title: Option<String>) -> Option<String> {
    let mut dialog = app.dialog().file().add_filter("Imagen", &["png", "jpg", "jpeg"]);
    if let Some(t) = title {
        dialog = dialog.set_title(t);
    }
    dialog
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Selector de archivos para «Crear PDF»: imágenes, texto, Markdown, HTML, libros EPUB, cómics CBZ, XPS y documentos de Office.
#[tauri::command]
pub async fn pick_source_files(app: tauri::AppHandle, title: Option<String>) -> Vec<String> {
    let mut dialog = app.dialog().file().add_filter(
        "Imágenes, libros, texto y Office",
        &[
            "jpg", "jpeg", "png", "tif", "tiff", "webp", "gif", "bmp", "txt", "md", "markdown", "html", "htm", "epub",
            "cbz", "xps", "oxps", "doc", "docx", "docm", "rtf", "odt", "xls", "xlsx", "xlsm", "csv", "ods", "ppt",
            "pptx", "odp",
        ],
    );
    if let Some(t) = title {
        dialog = dialog.set_title(t);
    }
    dialog
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Selector de un archivo de certificado con clave (.pfx/.p12); `None` si se cancela.
#[tauri::command]
pub async fn pick_key_file(app: tauri::AppHandle, title: Option<String>) -> Option<String> {
    let mut dialog = app.dialog().file().add_filter("Certificado", &["pfx", "p12"]);
    if let Some(t) = title {
        dialog = dialog.set_title(t);
    }
    dialog
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Selector de una carpeta (índice de búsqueda, lotes). `None` si se cancela.
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle, title: Option<String>) -> Option<String> {
    let mut dialog = app.dialog().file();
    if let Some(t) = title {
        dialog = dialog.set_title(t);
    }
    dialog
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Selector de archivos de cualquier tipo (cartera PDF). Vacío si se cancela.
#[tauri::command]
pub async fn pick_any_files(app: tauri::AppHandle, title: Option<String>) -> Vec<String> {
    let mut dialog = app.dialog().file();
    if let Some(t) = title {
        dialog = dialog.set_title(t);
    }
    dialog
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Selector de una acción guardada (`.toml` o `.json`). `None` si se cancela.
#[tauri::command]
pub async fn pick_action_file(app: tauri::AppHandle, title: Option<String>) -> Option<String> {
    let mut dialog = app.dialog().file().add_filter("Acción", &["toml", "json"]);
    if let Some(t) = title {
        dialog = dialog.set_title(t);
    }
    dialog
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Diálogo «Guardar como» para un PDF. `default_name` es el nombre que se propone; `None` si se cancela.
#[tauri::command]
pub async fn pick_save_pdf(
    app: tauri::AppHandle,
    title: Option<String>,
    default_name: Option<String>,
) -> Option<String> {
    let mut dialog = app.dialog().file().add_filter("PDF", &["pdf"]);
    if let Some(t) = title {
        dialog = dialog.set_title(t);
    }
    if let Some(n) = default_name {
        dialog = dialog.set_file_name(n);
    }
    dialog
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}
