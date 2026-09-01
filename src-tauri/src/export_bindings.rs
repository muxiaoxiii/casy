//! specta TS 绑定导出（D-3）
//!
//! 运行：`cargo test export_bindings`（在 src-tauri/ 下）；
//! 产物 src/types/bindings.ts 为生成物，禁止手改。

#[cfg(test)]
#[test]
fn export_ts_bindings() {
    let conf =
        specta::ts::ExportConfiguration::new().bigint(specta::ts::BigIntExportBehavior::Number);
    specta::export::ts_with_cfg("../src/types/bindings.ts", &conf).expect("specta TS 导出失败");
}
