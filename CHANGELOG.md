# 变更记录

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 与语义化版本。
**纪律：每一次修复（尤其是跨平台 bug）都必须在这里留一条**，不只是写进提交信息——
参照律智荟 OA 说明书的做法：它的更新日志把每个版本的新增/优化/修复逐条列出，
连"Utc 时间导致计时错误""计时器与 web 端不同步"这类 bug 都写清楚，
这才是产品迭代思路的沉淀方式。

分组口径：`新增` / `改进` / `修复` / `移除` / `安全` / `文档`。

---

## [未发布]

### 新增

- **法院送达短信取文**：`zxfw.court.gov.cn` 的短信链接是单页应用路由，`#` 之后的
  `sdbh/qdbh/sdsin` 是调用「一张网」送达 API 的凭证而非文件路径。新增
  `extract_service_token`（从查询串或 fragment 取参数）、签收确认
  （`updateSdxxBySdzt`）、文书列表（`getWsListBySdbhNew`）与逐份下载；
  下载使用不带自定义头的客户端（带 `Origin`/`Referer` 会被 OSS 拒签 403）。
  一次送达可能有多份文书，均按真实文书名落盘并在收件箱正文附清单。
- **文档引擎状态区**（设置）：展示 `DocumentEngineStatus` 全部字段——版本、可执行
  路径、当前 OCR provider（内置引擎 / 远端 Myna 及地址）、渲染器 / 坐标模型 /
  韩文 / layout / Ovis / 可搜索 PDF 各自可用性，每个缺失组件附具体修复指引，
  并可一键复制准备命令。
- **期限预警面板**（日历）：按 R1-R4 分组、真实剩余天数、法条依据、跳转案件；
  日历头部按钮显示 R1 计数。
- **开庭倒计时与提醒回执**：开庭行显示 T-N 倒计时与"已发 / 待发 / 已取消 / 失败"
  汇总及明细（新增 `list_hearing_reminder_jobs`——`reminder_jobs` 一直记录这些，
  此前无读取入口）。
- **操作记录**（全局）：`audit_events` 由触发器与 AI/MCP 写路径写入并带 actor
  归因，此前前端零入口。新增可按操作来源 / 变更类型 / 关键词筛选的记录页，
  payload 含 before/after 时渲染差异。
- **议程视图**（日历）：未来 14 天按天罗列开庭 / 法定期限 / 任务 / 日程。
- **OCR 校订版本链可回退**：区域校订生成 `paddle-onnx-corrected` 新版本后，
  旧版本仍在磁盘上但无法再读。新增 `pinned` 指针（迁移 v47）与
  `pin_document_job`；所有"取最新完成版本"的查询改为 `pinned DESC, rowid DESC`，
  未 pin 任何版本时行为不变。文件面板展示版本链并可切换，切换后重建检索索引。
- **待定归属**（收件箱）：内容含案号但无匹配案件也未人工指定的收件项单独成组，
  澄清面板显示检测到的案号并可"按此案号新建案件"。
- **卷宗文件元数据**：`document_date` / `knowledge_summary` / `knowledge_keywords`
  三列自 schema 起存在但无命令可写、列表 DTO 也不返回。新增
  `update_case_file_meta`（日期强制 ISO 校验）并在详情面板可直接编辑。
- **卷宗目录用途说明**：`list_case_dirs` 返回每个目录的用途（取自文件夹模板的
  `description`，或内置目录的既定用途），文件夹选择器与当前目录下方展示。
- **知识索引状态汇总**（处理中心）：已索引 / 总数、排队、进行中、失败数、
  向量接口是否配置，失败时可跳转处理。

### 改进

- **收件箱**：搜索、三种排序、加载更多（首屏 100、上限 500）、本页全选与批量
  忽略；批量结果逐项列成败。已处理项显示"去了哪里"（归入哪个案件、类别、
  处理时间与跳转）。推荐卡显示置信度 / 强度 / 规则还是 AI 判断，并新增
  "以上都不对 → 手动选择"出口（`create_project` 不再被静默过滤）；推荐卡可
  "不采用"并写入 `inbox_feedback`（后端与 service 一直齐全，UI 从不调用）。
  归卷前若识别不出材料类型先确认，不再静默归入"其他"。
- **案件与开庭**：索引卡显示期限提示与下次开庭（`list_cases` 新增
  `nextDeadlineDate` / `nextHearingDate`）；开庭可直接建成"延期待定 / 已取消"
  且强制填依据；改期保存后明示原日期的待发送提醒已作废；开庭表补筛选与折叠；
  开庭阶段改为可自由输入；案件详情 Tab 按 概览 / 进程 / 资料 / 关系 分组并记住
  上次使用的 tab。
- **日历**：日程编辑可改颜色 / 全天 / 时长快选（字段一直在 schema 里但从未
  传出）；拖拽改期给 8 秒撤销条；期限 chip 显示真实剩余天数与分级；今日页新增
  "期限预警"入口。
- **任务**：五种排序；多选批量完成与批量改案件；"排期"入口前置到任务页
  （原藏在 日历 → 预测 → 横向甘特图）；合并"稍后"预设与自定义日期为同一机制，
  "隐藏到某日再出现"独立为不同语义；重复任务行内显示重复规则。
- **全局检索**：新增开庭与期限分组（`cases_fts` 建了却从未 UNION 进
  `global_search`，hearings / case_deadlines 更是完全没查）；每组显示条数，
  本地记住最近 10 次搜索；语义不可用时给"去配置向量接口"按钮。
- **提醒规则**：补"消息模板"（含变量说明）与"适用案件类型"多选——
  `message_template` / `case_types` 自 schema 起存在，此前无表单入口，
  规则只能全局生效。
- **通知中心**：分"待处理 / 已处理"两个 tab（`read_at` 一直在记，此前只展示
  未读，处理后再无法回看）；每项支持"1 小时后再次提醒"。
- **卷宗**：多选批量入队 OCR（逐项成败明细）；查看器支持 ←/→ 翻页、+/- 缩放、
  Esc 关闭；文件可"存为知识"（带出处引用块 + OCR 正文摘录 + 回链文件与案件）。
- **早报**：改为结构化取数（期限预警 + 临近开庭 + 任务列表），不再正则解析
  提醒日志的文案。
- **AI 徽章**：`degraded` 分支此前永不返回（没有任何东西产生该状态）。usage 现
  记录今日失败与连续失败次数，连续 2 次失败转 amber 并说明已回退规则版，
  每分钟刷新以便恢复也能看到；popover 显示今日失败数。

### 修复

- **Windows 法院送达文书无法下载**：链接被当作文件直链，实际是单页应用路由，
  直接 GET 只拿到 801 字节 HTML 外壳，报"需要登录或验证码"。
- **Windows 知识引用改名漏改**：已登记卷宗被 canonicalize，DB 里所有路径带
  `\\?\` 前缀，而 LIKE 模式用原始路径构造，永远匹配不上正文里的普通路径，
  改名时引用重写被静默跳过。改为用去前缀的原生形式构造模式。
- **Windows 引擎无法编译**：`libc::getrusage` / `rusage` 在 Windows 不存在
  （`peak_rss_bytes` 遥测），改为 `#[cfg(unix)]` 分流。
- **Windows 链接期 CRT 冲突**：`tokenizers` 默认开启 `esaxx_fast`，其 C++ 以
  `/MT` 静态 CRT 编译，与 `ort` 的 `/MD` 冲突（LNK2038）。改为
  `default-features = false` + `onig`——esaxx 只服务 Unigram 训练，推理用不到。
- **Windows 架构校验误判**：licenses 目录下 PDF 渲染器 recipes 附带的 arm64
  辅助工具被当成运行时二进制，`verify-bundle` 拒绝打包。licenses/ 属溯源产物，
  不参与架构校验。
- **Windows 引擎栈溢出**：`process` 以 0xC00000FD 终止。Windows 主线程默认栈
  1MB（macOS 8MB），OCR/ONNX Runtime 调用链超出。改为在显式 64MB 栈的工作
  线程里运行引擎主体。
- **macOS DMG 偶发生成失败**：`hdiutil create` 报 `Resource busy`。改为先卸下
  同名旧卷，再带间隔重试最多三次。
- **CI 依赖审计**：升级 `rustls 0.23.45`、`lopdf 0.42`、`calamine 0.36`；
  引擎侧 `harumi 1.22` 带上 lopdf 0.42。余下 3 项（quick-xml ×2、rkyv）来自
  typst 0.15.1（当前最新版）传递链，兼容区间内无修复版本，已在 CI 加带移除
  条件的 `--ignore`。
- **rustc 1.99 clippy**：`chunks_exact_to_as_chunks` 新 lint，`embeddings.rs` 与
  `vector_index.rs` 改用 `as_chunks::<4>()`。
- **冒烟测试读错字段**：`smoke-bundle.mjs` 仍在读 `result.pages.length`，而
  R-02 起 `ProcessResult` 只回传 `pageCount`（页面集合唯一事实源是落盘页 IR）。
- **`FILE_COLUMNS` 漂移**：两处手写 SELECT 列清单与行映射常量不一致，新增列后
  报 `Invalid column index: 9`。已统一收敛到 `FILE_COLUMNS`。
- **`read-integrity` lint**：新代码里的 `rows.flatten()` 会静默丢弃 SQL 行解码
  错误（导致"下次开庭"无故缺失），已改为上抛而非放宽 lint。
- **视图切换测试脆弱**：日历测试用数组下标点视图按钮，插入"议程"后全部移位；
  且测试 i18n 无 messages 使按钮渲染 raw key。按钮加 `data-view`，测试改用
  key 定位。

### 文档

- `docs/UPDATE_PLAN_0.2.0.md`：0.2.0 架构与分期（取文→解析→建议→冲突确认→
  建档→留痕），含 Document Facts / Proposal / 冲突协议 C1-C5 / 数据模型变更。
- `docs/DETAIL_POLISH_0.2.0.md`：参照律智荟 OA 说明书（188 页全量抓取）整理出
  的模块级学习记录 + 40 余条打磨项（含文件:行号与量级）与三批实施进度。
- `docs/DOCUMENT_PIPELINE.md`：法院送达取文的完整链路与踩坑（fragment 凭证、
  OSS 预签名链接对请求头敏感）。
- `docs/DEVELOPMENT.md`：模型准备命令与档位布局。

---

## [0.1.4-beta.1] - 2026-10-10

macOS arm64 与 Windows x64 双端 beta 包；包含本地 OCR、版面分析、E5 嵌入模型、
PDF 渲染器与 Zvec 运行库。详见 `docs/releases/0.1.3-beta.1.md` 的验证口径。

[未发布]: https://github.com/muxiaoxiii/casy/compare/v0.1.4-beta.1...HEAD
