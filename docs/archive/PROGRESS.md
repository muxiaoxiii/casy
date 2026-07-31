# Casy 开发进度

> 最后更新：2026-07-31

## 已完成

### Phase 1：数据层 + 案件 CRUD ✅
- [x] 项目初始化（Tauri 2 + Vue 3 + Vite）
- [x] SQLite schema（18 张表 + FTS5 + 触发器 + 种子数据）
- [x] 案件 CRUD（7 个命令）
- [x] 飞书数据导入（5 张表）
- [x] 期限引擎（15 条法定规则）

### Phase 2：核心 UI ✅
- [x] 路由 + Pinia Store + 侧边栏布局
- [x] 首页 Dashboard
- [x] 案件列表（分组/筛选/排序/分页）
- [x] 案件详情（三栏布局 + 自动保存）
- [x] 任务管理（四象限）
- [x] 日历视图（月视图 + 事件标记）
- [x] 设置页（飞书导入）

### Phase 3：时间线 ✅
- [x] 时间线命令（get/add/delete case_log）
- [x] 案件详情集成时间线显示
- [x] 添加/删除日志弹窗

### Phase 4：同步基础 ✅
- [x] WebDAV 客户端（PUT/GET/HEAD/MKCOL）
- [x] 同步状态查询命令
- [x] 连接测试命令

### Phase 5：文件管理 ✅
- [x] 文件夹自动创建（7 个子目录）
- [x] 智能命名（SHA-256 防覆盖）
- [x] 自动分类规则

### Phase 6：文档解析 ✅
- [x] 规则分类（传票/口审/判决/起诉/答辩）
- [x] 传票解析（案号/日期/法院/法官/书记员）
- [x] 口审通知书解析（案件编号/专利号/当事人/合议组）
- [x] 判决书解析（案号/当事人）

### Phase 7：知识库 ✅
- [x] knowledge_items CRUD + 版本追踪
- [x] knowledge_relations（条目关系）
- [x] FTS5 全文搜索
- [x] 法条专用字段（law_name/article_no/effective_date/status）

### Phase 8：收件箱 ✅
- [x] 收件箱 CRUD（添加/列表/处理/归档/忽略）
- [x] 自动分类（11 种类型）
- [x] 案件匹配（案号/当事人模糊匹配）
- [x] 节假日通知解析
- [x] 前端收件箱 UI（待处理/已归档 + 文件导入 + 笔记添加）

## 后端命令清单（28 个）

| 命令 | 用途 |
|------|------|
| list_cases | 案件列表 |
| get_case | 获取案件 |
| create_case | 创建案件 |
| update_case | 更新案件 |
| delete_case | 删除案件 |
| search_cases | 全文搜索 |
| case_stats | 案件统计 |
| list_tasks | 任务列表 |
| create_task | 创建任务 |
| toggle_task | 切换完成 |
| delete_task | 删除任务 |
| get_calendar_events | 日历事件 |
| get_case_timeline | 时间线 |
| add_case_log | 添加日志 |
| delete_case_log | 删除日志 |
| get_sync_status | 同步状态 |
| test_webdav_connection | 测试连接 |
| import_feishu_data | 飞书导入 |
| get_deadline_warnings | 期限预警 |
| list_knowledge | 知识库列表 |
| create_knowledge | 创建知识条目 |
| update_knowledge | 更新知识条目 |
| delete_knowledge | 删除知识条目 |
| search_knowledge | 知识库搜索 |
| knowledge_stats | 知识库统计 |
| add_inbox_item | 添加收件项 |
| list_inbox_items | 收件箱列表 |
| process_inbox_item | 处理收件项 |
| file_inbox_item | 归档收件项 |
| dismiss_inbox_item | 忽略收件项 |
| parse_holiday_notice | 解析节假日通知 |

## 设计文档

- `docs/inbox-system-design.md` — 大口袋完整设计（入口/分类/路由/内部数据库更新/知识库）

## 待做

- [ ] Phase 9: 大口袋多入口（系统托盘/快捷键/悬浮窗）
- [ ] Phase 10: 文书工坊 + TipTap Copilot
- [ ] Phase 11: 邮件记录 + IMAP 监听
- [ ] Phase 12: 关系网络
- [ ] Phase 13: 飞书双向同步
- [ ] Phase 14: AI 模式集成
