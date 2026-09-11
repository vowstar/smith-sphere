//! Browser document labels surrounding the shared application canvas.

use smith_sphere_core::Lang;

/// Keeps the browser title and accessible canvas instructions in the UI language.
pub fn sync_language(lang: Lang) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    document.set_title(lang.pick("SmithSphere 史密斯球", "SmithSphere · Impedance sphere"));
    if let Some(root) = document.document_element() {
        let _ = root.set_attribute("lang", lang.pick("zh-CN", "en"));
    }
    if let Some(canvas) = document.get_element_by_id("smith_sphere_canvas") {
        let _ = canvas.set_attribute(
            "aria-label",
            lang.pick("SmithSphere 史密斯球工作区", "SmithSphere workspace"),
        );
    }
    if let Some(instructions) = document.get_element_by_id("canvas_instructions") {
        instructions.set_text_content(Some(lang.pick(
            "SmithSphere 交互画布。使用 Tab 与 Shift+Tab 在控件间移动，Enter 或空格激活控件，左右方向键移动频率滑块。",
            "SmithSphere interactive canvas. Use Tab and Shift+Tab to move between controls, Enter or Space to activate them, and the left and right arrow keys to step through frequencies.",
        )));
    }
}

/// Replaces the loading message while retaining the startup panel's structure.
pub fn show_startup_error() {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let lang = document
        .document_element()
        .and_then(|root| root.get_attribute("lang"))
        .and_then(|code| Lang::from_code(&code))
        .unwrap_or_default();
    if let Some(status) = document.get_element_by_id("startup_status") {
        let _ = status.set_attribute("data-failed", "true");
    }
    if let Some(message) = document.get_element_by_id("startup_message") {
        message.set_text_content(Some(lang.pick(
            "应用启动失败，请刷新页面重试。",
            "The application could not start. Reload the page to try again.",
        )));
    }
}
