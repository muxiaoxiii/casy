//! specta TS 绑定导出桩（D-3）
//!
//! specta v1 的 `#[derive(Type)]` 宏通过 ctor 将全部派生类型注册进全局
//! `TYPES` 表；本测试触发 `export::ts_with_cfg` 一次性导出到前端 bindings 文件。
//! CI 校验（git diff --exit-code）随 R-3 流水线接入。
//!
//! 口径：i64 → `number`（与手写 types/index.ts 的 number 约定一致；
//! Casy 的 i64 均为计数/时间戳，处于 JS 安全整数范围）。
//!
//! 运行：`cargo test export_bindings`（在 src-tauri/ 下）

#[cfg(test)]
mod export_bindings {
    #[test]
    fn export_ts_bindings() {
        let conf = specta::ts::ExportConfiguration::new()
            .bigint(specta::ts::BigIntExportBehavior::Number);
        specta::export::ts_with_cfg("../src/types/bindings.ts", &conf)
            .expect("specta TS 导出失败");
    }
}
