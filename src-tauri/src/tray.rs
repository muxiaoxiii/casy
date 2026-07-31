use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Runtime,
};

/// 构建系统托盘
pub fn setup_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    // 菜单项
    let add_file = MenuItemBuilder::with_id("add_file", "📎 添加文件").build(app)?;
    let add_note = MenuItemBuilder::with_id("add_note", "📝 添加笔记").build(app)?;
    let clipboard_inbox =
        MenuItemBuilder::with_id("clipboard_inbox", "📋 剪贴板到收件箱").build(app)?;
    let show_window = MenuItemBuilder::with_id("show_window", "🪟 打开窗口").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "🚪 退出").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&add_file)
        .item(&add_note)
        .item(&clipboard_inbox)
        .separator()
        .item(&show_window)
        .item(&quit)
        .build()?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("Casy - 案件管理")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "add_file" => {
                    // 显示窗口并导航到收件箱
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        let _ = win.eval("window.location.hash = '#/inbox'");
                    }
                    // 触发前端导入文件事件
                    let _ = app.emit("tray:add_file", ());
                }
                "add_note" => {
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        let _ = win.eval("window.location.hash = '#/inbox'");
                    }
                    let _ = app.emit("tray:add_note", ());
                }
                "clipboard_inbox" => {
                    let _ = app.emit("tray:clipboard_to_inbox", ());
                }
                "show_window" => {
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
