fn main() {
    #[cfg(feature = "webview")]
    tauri_build::build();
    #[cfg(all(feature = "native", not(feature = "webview")))]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        tauri_winres::WindowsResource::new()
            .set_icon("icons/icon.ico")
            .set("ProductName", "ai2tiku")
            .set("FileDescription", "ai2tiku · AI 题库")
            .compile()
            .expect("嵌入 Windows 图标失败");
    }
}
