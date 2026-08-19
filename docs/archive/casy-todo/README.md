# Casy 待实现模块设计文档

> **日期**: 2026-08-19（更新）  
> **说明**: 大部分模块已实现，剩余为长期愿景

---

## 已完成（后端 + 前端）

| 模块 | 实现内容 |
|------|---------|
| **收件箱** | 快速捕获条 + 截屏捕获 + 剪贴板粘贴 + 文件导入 + 拖拽上传 + AI 分类 + 厘清 + 批处理 |
| **收件箱批处理** | `start_inbox_batch` / `pause` / `resume` / `cancel` / `get_progress` / `retry` |
| **邮件模块** | IMAP 账号 CRUD + 邮件监听启动/停止/状态 + 新邮件自动入收件箱 |
| **知识库** | 6 职能分类 + 块级引用显示 + 混合检索（FTS + 语义）+ 版本管理 |
| **文书工坊** | 草稿 CRUD + 模板浏览器 + 导出状态栏（字数/保存状态/时间） |
| **AI 智伴** | 推荐引擎展示 + 决策记录 + 审计日志 + 工具管理 + 确认机制框架 |
| **MCP Server** | 6 个只读工具（case.query / task.query / knowledge.search / calendar.events / deadline.warnings / dashboard.stats） |

## P2 长期愿景（未实现）

| 模块 | 说明 |
|------|------|
| 完整 MCP 协议 | 当前为 HTTP JSON API，未来演进为标准 MCP 协议 |
| SMTP 发送 | 为日历同步 ICS 邀请提供发送能力 |
| 凭据迁移 keychain | IMAP 密码从 base64 升级为 OS keychain |

---

## 相关文档

- 设计哲学: `docs/casy-design-philosophy.md`
- 设计哲学待办清单: `docs/devlog/TODO-design-philosophy.md`
