//! Platform-specific file access. The browser edition reads picked files
//! asynchronously and hands the text back through a shared slot.

use smith_sphere_core::Lang;
use std::cell::RefCell;
use std::rc::Rc;

/// A file's name and decoded text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedFile {
    pub name: String,
    pub text: String,
}

impl LoadedFile {
    #[must_use]
    pub fn from_bytes(name: impl Into<String>, bytes: &[u8]) -> Self {
        Self {
            name: name.into(),
            text: String::from_utf8_lossy(bytes).into_owned(),
        }
    }
}

/// Slot filled by an asynchronous file picker.
pub type PendingFile = Rc<RefCell<Option<LoadedFile>>>;

pub const FILE_EXTENSIONS: [&str; 6] = ["s1p", "s2p", "csv", "txt", "tsv", "ts"];

/// Opens the platform file picker. On native the pick is synchronous and the
/// result is returned directly; in the browser the result arrives later in
/// `pending`.
#[cfg(not(target_arch = "wasm32"))]
pub fn pick_file(
    _context: &egui::Context,
    _pending: &PendingFile,
    lang: Lang,
) -> Result<Option<LoadedFile>, String> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Touchstone / CSV", &FILE_EXTENSIONS)
        .add_filter(lang.pick("所有文件", "All files"), &["*"])
        .pick_file()
    else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path).map_err(|error| match lang {
        Lang::Chinese => format!("无法读取 {}：{error}", path.display()),
        Lang::English => format!("Could not read {}: {error}", path.display()),
    })?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| lang.pick("文件", "file").to_owned());
    Ok(Some(LoadedFile::from_bytes(name, &bytes)))
}

/// Opens the browser file picker; the file arrives in `pending` on completion.
#[cfg(target_arch = "wasm32")]
pub fn pick_file(
    context: &egui::Context,
    pending: &PendingFile,
    _lang: Lang,
) -> Result<Option<LoadedFile>, String> {
    let slot = Rc::clone(pending);
    let context = context.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let dialog = rfd::AsyncFileDialog::new().add_filter("Touchstone / CSV", &FILE_EXTENSIONS);
        if let Some(handle) = dialog.pick_file().await {
            let bytes = handle.read().await;
            *slot.borrow_mut() = Some(LoadedFile::from_bytes(handle.file_name(), &bytes));
            context.request_repaint();
        }
    });
    Ok(None)
}

/// Converts files dropped onto the window into loaded text. Browser drops
/// are read asynchronously and delivered through `pending`.
pub fn dropped_files(context: &egui::Context, pending: &PendingFile) -> Vec<LoadedFile> {
    let dropped = context.input(|input| input.raw.dropped_files.clone());
    #[cfg_attr(target_arch = "wasm32", expect(unused_mut))]
    let mut loaded = Vec::new();
    for file in dropped {
        let name = file
            .path()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "data".to_owned());
        #[cfg(not(target_arch = "wasm32"))]
        match file.bytes() {
            Ok(bytes) => loaded.push(LoadedFile::from_bytes(name, &bytes)),
            Err(error) => log::warn!("unable to read dropped file {name}: {error}"),
        }
        #[cfg(target_arch = "wasm32")]
        {
            let slot = Rc::clone(pending);
            let context = context.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match file.bytes_async().await {
                    Ok(bytes) => {
                        *slot.borrow_mut() = Some(LoadedFile::from_bytes(name, &bytes));
                        context.request_repaint();
                    }
                    Err(error) => log::warn!("unable to read dropped file: {error}"),
                }
            });
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = pending;
    loaded
}
