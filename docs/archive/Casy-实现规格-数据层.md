# Casy 实现规格 — 数据层

> 补充文档，与 `软件规划文档.md` 配合使用

## 1. Cargo.toml 依赖

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
tauri-plugin-shell = "2"
tauri-plugin-notification = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"
rusqlite = { version = "0.32", features = ["bundled"] }  # v1: 普通 SQLite
# v2 升级: rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }  # AES-256 加密
keyring = "3"               # OS Keychain 访问（macOS Keychain / Windows Credential Manager）
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.12", features = ["json", "stream"] }  # stream 用于 WebDAV 流式上传
tokio = { version = "1", features = ["full"] }
regex = "1"
dirs = "5"
uuid = { version = "1", features = ["v4"] }
quick-xml = "0.37"
calamine = { version = "0.26", features = ["dates"] }  # Excel 读写（飞书导入+批量填写）
rust_xlsxwriter = "0.80"  # Excel 导出
zip = "0.6"               # .docsytpl 解压
base64 = "0.22"
sha2 = "0.10"
log = "0.4"
tracing = "0.1"
tracing-subscriber = "0.3"

# 邮件
mailparse = "0.15"             # .eml 解析（支持 RFC 2047 中文编码）
async-imap = { version = "0.11", default-features = false, features = ["runtime-tokio"] }  # 异步 IMAP + IDLE 推送
async-native-tls = "0.5"       # 异步 TLS for IMAP

# DOCX 文本提取
# 方案：解压 docx (zip) → 读取 word/document.xml → quick-xml 提取文本
# 不依赖 mammoth/python-docx，纯 Rust 实现

# OCR（可选，需系统安装 Tesseract）
rusty-tesseract = "1.1"        # 调用系统 tesseract CLI，无需 FFI 链接
```

## 2. 完整 SQLite Schema

```sql
-- ============================================================
-- 迁移版本管理
-- ============================================================
-- 使用 PRAGMA user_version 跟踪 schema 版本
-- 每次迁移递增版本号

-- ============================================================
-- 案件表
-- ============================================================
CREATE TABLE IF NOT EXISTS cases (
  id              TEXT PRIMARY KEY,
  track           TEXT NOT NULL DEFAULT 'patent_invalidation'
                  CHECK(track IN ('patent_invalidation','admin_litigation','civil_tort','other')),
  case_name       TEXT NOT NULL,
  case_no         TEXT,
  internal_no     TEXT,
  cause_action    TEXT,

  -- 当事人
  client_name     TEXT NOT NULL,
  our_role        TEXT,
  opponent_name   TEXT NOT NULL DEFAULT '',
  opponent_role   TEXT,
  opponent_firm   TEXT,
  opponent_agent  TEXT,

  -- 审理
  court           TEXT,
  judge_panel     TEXT,
  clerk           TEXT,
  attorneys       TEXT,            -- JSON array
  case_level      TEXT CHECK(case_level IN ('一审','二审','再审','结案',NULL)),
  case_status     TEXT,            -- 自动计算：从 case_result 推导，存库以支持高效查询
  case_progress   TEXT,
  case_result     TEXT,

  -- 专利
  patent_name     TEXT,
  patent_app_no   TEXT,
  procedure_type  TEXT CHECK(procedure_type IN ('普通','简易',NULL)),

  -- 日期里程碑
  filing_date     TEXT,
  complaint_received_date TEXT,
  trial_date      TEXT,
  trial2_date     TEXT,
  trial3_date     TEXT,
  verdict_type    TEXT,
  verdict_date    TEXT,
  stay_date       TEXT,
  relief_deadline TEXT,

  -- 专利无效专属
  petitioner_first_invalid TEXT,
  petitioner_supp_deadline TEXT,
  petitioner_submit_date   TEXT,
  petitioner_received_date TEXT,
  petitioner_reply_deadline TEXT,
  patentee_received_date   TEXT,
  patentee_statement_deadline TEXT,
  patentee_received_supp_date TEXT,
  patentee_supp_deadline TEXT,
  patentee_submit_supp_date TEXT,

  -- 文件夹
  folder_path     TEXT,

  -- 文书
  last_doc_path   TEXT,
  last_doc_at     TEXT,

  -- 进度
  completed_text  TEXT,
  notes           TEXT,

  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_cases_track ON cases(track);
CREATE INDEX IF NOT EXISTS idx_cases_client ON cases(client_name);
CREATE INDEX IF NOT EXISTS idx_cases_court ON cases(court);
CREATE INDEX IF NOT EXISTS idx_cases_status ON cases(case_status);
CREATE INDEX IF NOT EXISTS idx_cases_filing ON cases(filing_date);
CREATE INDEX IF NOT EXISTS idx_cases_progress ON cases(case_progress);

-- 注意：删除案件时，级联删除日志/庭审/任务/关系/文件记录
-- 物理文件不自动删除，移入系统回收站（macOS: ~/.Trash，Windows: 回收站）
-- 实现在 Rust 层的 delete_case 命令中处理

-- updated_at 自动更新触发器
CREATE TRIGGER IF NOT EXISTS trg_cases_updated
AFTER UPDATE ON cases
FOR EACH ROW
BEGIN
  UPDATE cases SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- case_status 自动计算触发器
CREATE TRIGGER IF NOT EXISTS trg_cases_status_insert
AFTER INSERT ON cases
FOR EACH ROW
WHEN NEW.case_status IS NULL
BEGIN
  UPDATE cases SET case_status = CASE
    WHEN NEW.case_result IN ('结案','胜诉','败诉','对方撤案','撤诉','解除委托') THEN '已完结'
    WHEN NEW.case_result IS NOT NULL AND NEW.case_result != '' THEN '进行中'
    ELSE '未知'
  END WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_cases_status_update
AFTER UPDATE OF case_result ON cases
FOR EACH ROW
BEGIN
  UPDATE cases SET case_status = CASE
    WHEN NEW.case_result IN ('结案','胜诉','败诉','对方撤案','撤诉','解除委托') THEN '已完结'
    WHEN NEW.case_result IS NOT NULL AND NEW.case_result != '' THEN '进行中'
    ELSE '未知'
  END WHERE id = NEW.id;
END;

-- FTS5 全文搜索
CREATE VIRTUAL TABLE IF NOT EXISTS cases_fts USING fts5(
  case_name, case_no, client_name, opponent_name,
  patent_name, notes, content=cases, content_rowid=rowid
);

-- FTS 同步触发器
CREATE TRIGGER IF NOT EXISTS trg_cases_ai AFTER INSERT ON cases BEGIN
  INSERT INTO cases_fts(rowid, case_name, case_no, client_name, opponent_name, patent_name, notes)
  VALUES (new.rowid, new.case_name, new.case_no, new.client_name, new.opponent_name, new.patent_name, new.notes);
END;
CREATE TRIGGER IF NOT EXISTS trg_cases_ad AFTER DELETE ON cases BEGIN
  INSERT INTO cases_fts(cases_fts, rowid, case_name, case_no, client_name, opponent_name, patent_name, notes)
  VALUES ('delete', old.rowid, old.case_name, old.case_no, old.client_name, old.opponent_name, old.patent_name, old.notes);
END;
CREATE TRIGGER IF NOT EXISTS trg_cases_au AFTER UPDATE ON cases BEGIN
  INSERT INTO cases_fts(cases_fts, rowid, case_name, case_no, client_name, opponent_name, patent_name, notes)
  VALUES ('delete', old.rowid, old.case_name, old.case_no, old.client_name, old.opponent_name, old.patent_name, old.notes);
  INSERT INTO cases_fts(rowid, case_name, case_no, client_name, opponent_name, patent_name, notes)
  VALUES (new.rowid, new.case_name, new.case_no, new.client_name, new.opponent_name, new.patent_name, new.notes);
END;

-- ============================================================
-- 客户表
-- ============================================================
CREATE TABLE IF NOT EXISTS clients (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL UNIQUE,
  type            TEXT DEFAULT 'company' CHECK(type IN ('company','individual')),
  contact_person  TEXT,
  phone           TEXT,
  email           TEXT,
  address         TEXT,
  notes           TEXT,
  case_count      INTEGER DEFAULT 0,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE TRIGGER IF NOT EXISTS trg_clients_updated
AFTER UPDATE ON clients FOR EACH ROW
BEGIN
  UPDATE clients SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- 办案日志
-- ============================================================
CREATE TABLE IF NOT EXISTS case_logs (
  id            TEXT PRIMARY KEY,
  case_id       TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  event_summary TEXT NOT NULL,
  event_name    TEXT,
  event_type    TEXT NOT NULL CHECK(event_type IN ('task','submitted','received','record','email')),
  event_date    TEXT NOT NULL,
  content       TEXT,
  files_json    TEXT,            -- [{name, path, size}]
  created_at    TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_logs_case ON case_logs(case_id);
CREATE INDEX IF NOT EXISTS idx_logs_date ON case_logs(event_date);
CREATE INDEX IF NOT EXISTS idx_logs_type ON case_logs(event_type);

-- ============================================================
-- 庭审信息
-- ============================================================
CREATE TABLE IF NOT EXISTS hearings (
  id              TEXT PRIMARY KEY,
  case_id         TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  hearing_record  TEXT NOT NULL,
  hearing_name    TEXT,
  hearing_date    TEXT NOT NULL,
  venue           TEXT,
  attendees       TEXT,
  judges          TEXT,            -- JSON array
  court           TEXT,
  case_level      TEXT,
  contact_info    TEXT,
  actual_status   TEXT CHECK(actual_status IN ('已开','未开',NULL)),
  files_json      TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_hearings_case ON hearings(case_id);
CREATE INDEX IF NOT EXISTS idx_hearings_date ON hearings(hearing_date);

-- ============================================================
-- 任务管理
-- ============================================================
CREATE TABLE IF NOT EXISTS tasks (
  id            TEXT PRIMARY KEY,
  case_id       TEXT REFERENCES cases(id) ON DELETE CASCADE,
  task_name     TEXT NOT NULL,
  description   TEXT,
  created_date  TEXT NOT NULL,
  deadline      TEXT,
  priority      TEXT CHECK(priority IN ('urgent_important','important','urgent','normal',NULL)),
  completed     INTEGER DEFAULT 0 CHECK(completed IN (0,1)),
  assignee      TEXT,
  finish_note   TEXT,
  source_log_id TEXT,
  created_at    TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_tasks_case ON tasks(case_id);
CREATE INDEX IF NOT EXISTS idx_tasks_deadline ON tasks(deadline);
CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority);
CREATE INDEX IF NOT EXISTS idx_tasks_completed ON tasks(completed);

-- ============================================================
-- 官方人员
-- ============================================================
CREATE TABLE IF NOT EXISTS officials (
  id              TEXT PRIMARY KEY,
  name            TEXT,
  role            TEXT NOT NULL CHECK(role IN ('法官','法官助理','书记员','法院')),
  court           TEXT NOT NULL,
  contact_detail  TEXT NOT NULL,
  contact_text    TEXT,
  contact_record  TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_officials_court ON officials(court);
CREATE INDEX IF NOT EXISTS idx_officials_role ON officials(role);

-- ============================================================
-- 案件-官方人员关联
-- ============================================================
CREATE TABLE IF NOT EXISTS case_officials (
  case_id       TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  official_id   TEXT NOT NULL REFERENCES officials(id) ON DELETE CASCADE,
  PRIMARY KEY (case_id, official_id)
);

-- ============================================================
-- 案件关系（自引用多对多）
-- ============================================================
CREATE TABLE IF NOT EXISTS case_relations (
  id              TEXT PRIMARY KEY,
  source_case_id  TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  target_case_id  TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  relation_type   TEXT NOT NULL CHECK(relation_type IN ('same_patent','same_party','appeal_of','cross_reference')),
  label           TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  UNIQUE(source_case_id, target_case_id, relation_type)
);

-- ============================================================
-- 期限规则（法定期限模板，由系统预设，用户可查看但不随意修改）
-- ============================================================
CREATE TABLE IF NOT EXISTS deadline_rules (
  id              TEXT PRIMARY KEY,
  track           TEXT NOT NULL,               -- patent_invalidation / admin_litigation / civil_tort
  rule_name       TEXT NOT NULL,               -- 期限名称: "答辩期" / "补充意见期限" / "预估审限"
  legal_basis     TEXT NOT NULL,               -- 法律依据: "民事诉讼法第128条" / "专利法实施细则第5条"
  trigger_field   TEXT NOT NULL,               -- 触发字段: filing_date / complaint_received_date / ...
  offset_value    INTEGER NOT NULL,            -- 偏移值
  offset_unit     TEXT NOT NULL DEFAULT 'day'
                  CHECK(offset_unit IN ('day','calendar_month')),  -- 自然日 / 日历月
  calc_method     TEXT NOT NULL DEFAULT 'civil'
                  CHECK(calc_method IN ('civil','patent')),  -- 诉讼法算法 / 专利法实施细则算法
  procedure_types TEXT,                        -- 适用程序 JSON: ["普通","简易"] 或 NULL=全部
  deadline_source TEXT NOT NULL DEFAULT 'statutory'
                  CHECK(deadline_source IN ('statutory','recommended')),  -- 法定 / 推荐（可选）
  auto_calculate  INTEGER NOT NULL DEFAULT 1,  -- 1=自动计算, 0=仅作推荐模板
  priority        INTEGER DEFAULT 0,
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

-- 案件实际期限（每个案件的具体期限，含手动录入的法院指定/自行确定期限）
-- ============================================================
CREATE TABLE IF NOT EXISTS case_deadlines (
  id              TEXT PRIMARY KEY,
  case_id         TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  rule_id         TEXT REFERENCES deadline_rules(id),  -- 关联法定规则（手动录入时为 NULL）
  deadline_name   TEXT NOT NULL,               -- 期限名称
  trigger_date    TEXT,                        -- 触发日期（自动计算时有值）
  due_date        TEXT NOT NULL,               -- 届满日期
  days_left       INTEGER,                     -- 距今天数（实时计算，不存库也可）
  deadline_source TEXT NOT NULL DEFAULT 'statutory'
                  CHECK(deadline_source IN ('statutory','court','manual')),  -- 法定 / 法院指定 / 自行确定
  legal_basis     TEXT,                        -- 法律依据（法定期限时有值）
  court_order_ref TEXT,                        -- 法院指定依据（如通知书编号）
  notes           TEXT,                        -- 备注
  completed       INTEGER DEFAULT 0,           -- 是否已完成
  completed_at    TEXT,                        -- 完成时间
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_case_deadlines_case ON case_deadlines(case_id);
CREATE INDEX IF NOT EXISTS idx_case_deadlines_due ON case_deadlines(due_date);
CREATE INDEX IF NOT EXISTS idx_case_deadlines_source ON case_deadlines(deadline_source);

CREATE TRIGGER IF NOT EXISTS trg_case_deadlines_updated
AFTER UPDATE ON case_deadlines FOR EACH ROW
BEGIN
  UPDATE case_deadlines SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- 收件箱
-- ============================================================
CREATE TABLE IF NOT EXISTS inbox_items (
  id              TEXT PRIMARY KEY,
  source_type     TEXT NOT NULL CHECK(source_type IN ('file','email','sms','note','paste','imap')),
  source_path     TEXT,
  source_url      TEXT,
  source_time     TEXT,
  title           TEXT,
  content_text    TEXT,
  content_html    TEXT,
  ai_category     TEXT,
  ai_confidence   REAL,
  ai_extracted    TEXT,
  ai_suggested_case_id TEXT,
  status          TEXT DEFAULT 'pending' CHECK(status IN ('pending','processing','filed','dismissed')),
  user_category   TEXT,
  linked_case_id  TEXT REFERENCES cases(id) ON DELETE SET NULL,
  filed_to        TEXT,
  filed_as        TEXT,
  knowledge_mark  INTEGER DEFAULT 0 CHECK(knowledge_mark IN (0,1,2)),
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  processed_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_inbox_status ON inbox_items(status);
CREATE INDEX IF NOT EXISTS idx_inbox_category ON inbox_items(ai_category);
CREATE INDEX IF NOT EXISTS idx_inbox_case ON inbox_items(linked_case_id);

-- ============================================================
-- 案卷文件
-- ============================================================
CREATE TABLE IF NOT EXISTS case_files (
  id              TEXT PRIMARY KEY,
  case_id         TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  file_name       TEXT NOT NULL,
  file_path       TEXT NOT NULL,
  file_size       INTEGER,
  file_type       TEXT,
  category        TEXT NOT NULL CHECK(category IN ('summons','evidence','submitted','received','internal','correspondence','other')),
  sub_category    TEXT,
  source_inbox_id TEXT,
  source_type     TEXT CHECK(source_type IN ('inbox','manual','generated','imported',NULL)),
  knowledge_mark  INTEGER DEFAULT 0 CHECK(knowledge_mark IN (0,1,2)),
  knowledge_summary TEXT,
  knowledge_keywords TEXT,
  document_date   TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_files_case ON case_files(case_id);
CREATE INDEX IF NOT EXISTS idx_files_category ON case_files(category);
CREATE INDEX IF NOT EXISTS idx_files_knowledge ON case_files(knowledge_mark);

CREATE TRIGGER IF NOT EXISTS trg_files_updated
AFTER UPDATE ON case_files FOR EACH ROW
BEGIN
  UPDATE case_files SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- FTS5
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
  file_name, knowledge_summary, knowledge_keywords,
  content=case_files, content_rowid=rowid
);

CREATE TRIGGER IF NOT EXISTS trg_files_ai AFTER INSERT ON case_files BEGIN
  INSERT INTO files_fts(rowid, file_name, knowledge_summary, knowledge_keywords)
  VALUES (new.rowid, new.file_name, new.knowledge_summary, new.knowledge_keywords);
END;
CREATE TRIGGER IF NOT EXISTS trg_files_ad AFTER DELETE ON case_files BEGIN
  INSERT INTO files_fts(files_fts, rowid, file_name, knowledge_summary, knowledge_keywords)
  VALUES ('delete', old.rowid, old.file_name, old.knowledge_summary, old.knowledge_keywords);
END;
CREATE TRIGGER IF NOT EXISTS trg_files_au AFTER UPDATE ON case_files BEGIN
  INSERT INTO files_fts(files_fts, rowid, file_name, knowledge_summary, knowledge_keywords)
  VALUES ('delete', old.rowid, old.file_name, old.knowledge_summary, old.knowledge_keywords);
  INSERT INTO files_fts(rowid, file_name, knowledge_summary, knowledge_keywords)
  VALUES (new.rowid, new.file_name, new.knowledge_summary, new.knowledge_keywords);
END;

-- ============================================================
-- 邮件记录
-- ============================================================
CREATE TABLE IF NOT EXISTS email_records (
  id              TEXT PRIMARY KEY,
  message_id      TEXT,
  subject         TEXT NOT NULL,
  from_address    TEXT NOT NULL,
  from_name       TEXT,
  to_addresses    TEXT,
  cc_addresses    TEXT,
  date            TEXT NOT NULL,
  body_text       TEXT,
  body_html       TEXT,
  linked_case_id  TEXT REFERENCES cases(id) ON DELETE SET NULL,
  source_inbox_id TEXT,
  email_type      TEXT CHECK(email_type IN ('correspondence','court_notice','client_instruction','opposing_counsel','other',NULL)),
  importance      TEXT DEFAULT 'normal' CHECK(importance IN ('high','normal','low')),
  knowledge_mark  INTEGER DEFAULT 0,
  knowledge_summary TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_email_case ON email_records(linked_case_id);
CREATE INDEX IF NOT EXISTS idx_email_date ON email_records(date);
CREATE INDEX IF NOT EXISTS idx_email_from ON email_records(from_address);

-- ============================================================
-- 知识库
-- ============================================================
CREATE TABLE IF NOT EXISTS knowledge_items (
  id            TEXT PRIMARY KEY,
  title         TEXT NOT NULL,
  category      TEXT NOT NULL
                CHECK(category IN (
                  'legal_provision','case_note','email','document_summary',
                  'holiday','cause_action','court_name','judge_info','other'
                )),
  content       TEXT NOT NULL,
  tags          TEXT,                   -- JSON array
  source_type   TEXT,                   -- inbox / manual / auto_extract
  source_id     TEXT,                   -- 来源收件箱项 ID
  linked_case_id TEXT REFERENCES cases(id) ON DELETE SET NULL,

  -- 法条专用字段
  law_name      TEXT,                   -- 法律名称（如"专利法实施细则"）
  article_no    TEXT,                   -- 条号（如"第5条"）
  effective_date TEXT,                  -- 生效日期
  status        TEXT DEFAULT 'current'  -- current / amended / repealed

  created_at    TEXT DEFAULT (datetime('now','localtime')),
  updated_at    TEXT DEFAULT (datetime('now','localtime'))
);

-- 知识条目版本（跟踪修改历史）
CREATE TABLE IF NOT EXISTS knowledge_versions (
  id            TEXT PRIMARY KEY,
  item_id       TEXT NOT NULL REFERENCES knowledge_items(id) ON DELETE CASCADE,
  content       TEXT NOT NULL,
  changed_at    TEXT DEFAULT (datetime('now','localtime')),
  change_reason TEXT
);

-- 知识条目关系
CREATE TABLE IF NOT EXISTS knowledge_relations (
  id            TEXT PRIMARY KEY,
  source_id     TEXT NOT NULL REFERENCES knowledge_items(id) ON DELETE CASCADE,
  target_id     TEXT NOT NULL REFERENCES knowledge_items(id) ON DELETE CASCADE,
  relation_type TEXT NOT NULL CHECK(relation_type IN ('cites','amends','supersedes','implements','related')),
  UNIQUE(source_id, target_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_category ON knowledge_items(category);
CREATE INDEX IF NOT EXISTS idx_knowledge_case ON knowledge_items(linked_case_id);

CREATE VIRTUAL TABLE IF NOT EXISTS knowledge_fts USING fts5(
  title, content, tags,
  content=knowledge_items, content_rowid=rowid
);

CREATE TRIGGER IF NOT EXISTS trg_knowledge_ai AFTER INSERT ON knowledge_items BEGIN
  INSERT INTO knowledge_fts(rowid, title, content, tags)
  VALUES (new.rowid, new.title, new.content, new.tags);
END;
CREATE TRIGGER IF NOT EXISTS trg_knowledge_ad AFTER DELETE ON knowledge_items BEGIN
  INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content, tags)
  VALUES ('delete', old.rowid, old.title, old.content, old.tags);
END;
CREATE TRIGGER IF NOT EXISTS trg_knowledge_au AFTER UPDATE ON knowledge_items BEGIN
  INSERT INTO knowledge_fts(knowledge_fts, rowid, title, content, tags)
  VALUES ('delete', old.rowid, old.title, old.content, old.tags);
  INSERT INTO knowledge_fts(rowid, title, content, tags)
  VALUES (new.rowid, new.title, new.content, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS trg_knowledge_updated
AFTER UPDATE ON knowledge_items FOR EACH ROW
BEGIN
  UPDATE knowledge_items SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- Skill 工作流
-- ============================================================
CREATE TABLE IF NOT EXISTS skills (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  category      TEXT,
  description   TEXT,
  workflow_json TEXT,
  template_path TEXT,
  knowledge_ids TEXT,
  created_at    TEXT DEFAULT (datetime('now','localtime'))
);

-- ============================================================
-- 草稿
-- ============================================================
CREATE TABLE IF NOT EXISTS drafts (
  id            TEXT PRIMARY KEY,
  case_id       TEXT REFERENCES cases(id) ON DELETE SET NULL,
  title         TEXT NOT NULL,
  content       TEXT,
  template_path TEXT,
  status        TEXT DEFAULT 'draft' CHECK(status IN ('draft','final','archived')),
  version       INTEGER DEFAULT 1,
  created_at    TEXT DEFAULT (datetime('now','localtime')),
  updated_at    TEXT DEFAULT (datetime('now','localtime'))
);

CREATE TRIGGER IF NOT EXISTS trg_drafts_updated
AFTER UPDATE ON drafts FOR EACH ROW
BEGIN
  UPDATE drafts SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- 同步映射
-- ============================================================
CREATE TABLE IF NOT EXISTS sync_map (
  id              TEXT PRIMARY KEY,
  local_table     TEXT NOT NULL,
  local_id        TEXT NOT NULL,
  remote_id       TEXT,
  remote_source   TEXT NOT NULL CHECK(remote_source IN ('feishu','webdav')),
  local_updated   TEXT,
  remote_updated  TEXT,
  sync_status     TEXT DEFAULT 'synced' CHECK(sync_status IN ('synced','local_newer','remote_newer','conflict','push_failed')),
  conflict_fields TEXT,
  last_synced_at  TEXT,
  UNIQUE(local_table, local_id, remote_source)
);

CREATE INDEX IF NOT EXISTS idx_sync_status ON sync_map(sync_status);

-- ============================================================
-- 同步队列
-- ============================================================
CREATE TABLE IF NOT EXISTS sync_queue (
  id            TEXT PRIMARY KEY,
  direction     TEXT NOT NULL CHECK(direction IN ('push','pull')),
  source        TEXT NOT NULL CHECK(source IN ('feishu','webdav')),
  local_table   TEXT NOT NULL,
  local_id      TEXT,
  remote_id     TEXT,
  payload_json  TEXT,
  attempts      INTEGER DEFAULT 0,
  max_attempts  INTEGER DEFAULT 3,
  last_error    TEXT,
  status        TEXT DEFAULT 'pending' CHECK(status IN ('pending','processing','done','failed')),
  created_at    TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_syncq_status ON sync_queue(status);

-- ============================================================
-- IMAP 配置
-- ============================================================
CREATE TABLE IF NOT EXISTS imap_accounts (
  id            TEXT PRIMARY KEY,
  email_address TEXT NOT NULL,
  imap_server   TEXT NOT NULL,
  imap_port     INTEGER DEFAULT 993,
  username      TEXT NOT NULL,
  password_enc  TEXT NOT NULL,       -- 加密存储
  use_tls       INTEGER DEFAULT 1,
  watch_folders TEXT DEFAULT 'INBOX', -- JSON array
  filter_from   TEXT,                 -- 白名单发件人 JSON array
  filter_subject TEXT,                -- 主题关键词 JSON array
  enabled       INTEGER DEFAULT 1,
  last_sync_uid TEXT,
  created_at    TEXT DEFAULT (datetime('now','localtime'))
);
```

## 3. Rust 结构体（核心）

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    pub id: String,
    pub track: String,
    pub case_name: String,
    pub case_no: Option<String>,
    pub internal_no: Option<String>,
    pub cause_action: Option<String>,
    pub client_name: String,
    pub our_role: Option<String>,
    pub opponent_name: String,
    pub opponent_role: Option<String>,
    pub opponent_firm: Option<String>,
    pub opponent_agent: Option<String>,
    pub court: Option<String>,
    pub judge_panel: Option<String>,
    pub clerk: Option<String>,
    pub attorneys: Option<String>,  // JSON
    pub case_level: Option<String>,
    pub case_status: Option<String>,
    pub case_progress: Option<String>,
    pub case_result: Option<String>,
    pub patent_name: Option<String>,
    pub patent_app_no: Option<String>,
    pub procedure_type: Option<String>,
    pub filing_date: Option<String>,
    pub complaint_received_date: Option<String>,
    pub trial_date: Option<String>,
    pub trial2_date: Option<String>,
    pub trial3_date: Option<String>,
    pub verdict_type: Option<String>,
    pub verdict_date: Option<String>,
    pub stay_date: Option<String>,
    pub relief_deadline: Option<String>,
    pub petitioner_first_invalid: Option<String>,
    pub petitioner_supp_deadline: Option<String>,
    pub petitioner_submit_date: Option<String>,
    pub petitioner_received_date: Option<String>,
    pub petitioner_reply_deadline: Option<String>,
    pub patentee_received_date: Option<String>,
    pub patentee_statement_deadline: Option<String>,
    pub patentee_received_supp_date: Option<String>,
    pub patentee_supp_deadline: Option<String>,
    pub patentee_submit_supp_date: Option<String>,
    pub folder_path: Option<String>,
    pub last_doc_path: Option<String>,
    pub last_doc_at: Option<String>,
    pub completed_text: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseLog {
    pub id: String,
    pub case_id: String,
    pub event_summary: String,
    pub event_name: Option<String>,
    pub event_type: String,
    pub event_date: String,
    pub content: Option<String>,
    pub files_json: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hearing {
    pub id: String,
    pub case_id: String,
    pub hearing_record: String,
    pub hearing_name: Option<String>,
    pub hearing_date: String,
    pub venue: Option<String>,
    pub attendees: Option<String>,
    pub judges: Option<String>,
    pub court: Option<String>,
    pub case_level: Option<String>,
    pub contact_info: Option<String>,
    pub actual_status: Option<String>,
    pub files_json: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub case_id: Option<String>,
    pub task_name: String,
    pub description: Option<String>,
    pub created_date: String,
    pub deadline: Option<String>,
    pub priority: Option<String>,
    pub completed: i32,
    pub assignee: Option<String>,
    pub finish_note: Option<String>,
    pub source_log_id: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub source_type: String,
    pub source_path: Option<String>,
    pub source_url: Option<String>,
    pub source_time: Option<String>,
    pub title: Option<String>,
    pub content_text: Option<String>,
    pub content_html: Option<String>,
    pub ai_category: Option<String>,
    pub ai_confidence: Option<f64>,
    pub ai_extracted: Option<String>,
    pub ai_suggested_case_id: Option<String>,
    pub status: String,
    pub user_category: Option<String>,
    pub linked_case_id: Option<String>,
    pub filed_to: Option<String>,
    pub filed_as: Option<String>,
    pub knowledge_mark: i32,
    pub created_at: Option<String>,
    pub processed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseFile {
    pub id: String,
    pub case_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub file_type: Option<String>,
    pub category: String,
    pub sub_category: Option<String>,
    pub source_inbox_id: Option<String>,
    pub source_type: Option<String>,
    pub knowledge_mark: i32,
    pub knowledge_summary: Option<String>,
    pub knowledge_keywords: Option<String>,
    pub document_date: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailRecord {
    pub id: String,
    pub message_id: Option<String>,
    pub subject: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: Option<String>,
    pub cc_addresses: Option<String>,
    pub date: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub linked_case_id: Option<String>,
    pub source_inbox_id: Option<String>,
    pub email_type: Option<String>,
    pub importance: String,
    pub knowledge_mark: i32,
    pub knowledge_summary: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineRule {
    pub id: String,
    pub track: String,
    pub trigger_field: String,
    pub offset_days: i64,
    pub offset_unit: String,
    pub rule_name: String,
    pub priority: i32,
    pub condition: Option<String>,
    pub created_at: Option<String>,
}
```

## 4. 关键查询

```rust
// === 案件 ===

// 列表（分页+筛选）
// SQL: SELECT * FROM cases WHERE ($track IS NULL OR track=$track) AND ($client IS NULL OR client_name=$client) ORDER BY filing_date DESC LIMIT $limit OFFSET $offset
fn list_cases(conn: &Connection, filter: &CaseFilter, page: i64, per_page: i64) -> Result<Vec<Case>>;

// 全文搜索
// SQL: SELECT c.* FROM cases_fts f JOIN cases c ON c.rowid=f.rowid WHERE cases_fts MATCH $query ORDER BY rank LIMIT 20
fn search_cases(conn: &Connection, query: &str) -> Result<Vec<Case>>;

// 活跃案件（有期限的）
// SQL: SELECT * FROM cases WHERE case_status != '已完结' OR case_status IS NULL
fn active_cases(conn: &Connection) -> Result<Vec<Case>>;

// 按客户分组统计
// SQL: SELECT client_name, COUNT(*) as cnt FROM cases GROUP BY client_name ORDER BY cnt DESC
fn case_counts_by_client(conn: &Connection) -> Result<Vec<(String, i64)>>;

// === 日志 ===

// 案件时间线
// SQL: SELECT * FROM case_logs WHERE case_id=$id ORDER BY event_date DESC
fn case_timeline(conn: &Connection, case_id: &str) -> Result<Vec<CaseLog>>;

// === 庭审 ===

// 近期开庭
// SQL: SELECT h.*, c.case_name FROM hearings h JOIN cases c ON c.id=h.case_id WHERE h.hearing_date >= date('now') ORDER BY h.hearing_date LIMIT 20
fn upcoming_hearings(conn: &Connection) -> Result<Vec<Hearing>>;

// === 任务 ===

// 未完成任务
// SQL: SELECT * FROM tasks WHERE completed=0 ORDER BY CASE priority WHEN 'urgent_important' THEN 1 WHEN 'urgent' THEN 2 WHEN 'important' THEN 3 ELSE 4 END, deadline ASC
fn pending_tasks(conn: &Connection) -> Result<Vec<Task>>;

// === 收件箱 ===

// 待处理
// SQL: SELECT * FROM inbox_items WHERE status='pending' ORDER BY created_at DESC
fn pending_inbox(conn: &Connection) -> Result<Vec<InboxItem>>;

// === 文件 ===

// 案件文件按分类
// SQL: SELECT * FROM case_files WHERE case_id=$id ORDER BY category, document_date DESC
fn case_files(conn: &Connection, case_id: &str) -> Result<Vec<CaseFile>>;

// === 期限 ===

// 所有活跃案件的期限预警
// SQL: SELECT c.id, c.case_name, r.rule_name, <computed_deadline>, <days_left>
//      FROM cases c, deadline_rules r
//      WHERE c.track=r.track AND c.case_status != '已完结'
fn deadline_warnings(conn: &Connection) -> Result<Vec<DeadlineWarning>>;
```

## 5. 数据库初始化

```rust
use rusqlite::Connection;

const CURRENT_SCHEMA_VERSION: i64 = 1;

pub fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    if version == 0 {
        // 首次创建，执行全部建表语句
        conn.execute_batch(include_str!("schema.sql"))?;
        conn.execute_batch(&format!("PRAGMA user_version = {};", CURRENT_SCHEMA_VERSION))?;
        seed_deadline_rules(conn)?;
    } else if version < CURRENT_SCHEMA_VERSION {
        // 迁移
        run_migrations(conn, version)?;
    }

    Ok(())
}

fn seed_deadline_rules(conn: &Connection) -> Result<()> {
    // ── 专利无效（专利法实施细则算法：起算日不计、日历月、休假日顺延）──
    let patent_rules: Vec<(&str, &str, &str, &str, &str, i64, &str, &str, &str, i32)> = vec![
        // (id, track, rule_name, legal_basis, trigger_field, offset_value, offset_unit, calc_method, deadline_source, priority)
        ("rule-pi-001", "patent_invalidation", "专利权人陈述意见期限",
         "专利法实施细则第58条", "patentee_received_date", 1, "calendar_month", "patent", "statutory", 10),
        ("rule-pi-002", "patent_invalidation", "请求人答复意见期限",
         "专利法实施细则第58条", "petitioner_received_date", 1, "calendar_month", "patent", "statutory", 10),
        ("rule-pi-003", "patent_invalidation", "专利权人补充意见期限",
         "专利法实施细则第58条", "patentee_received_supp_date", 1, "calendar_month", "patent", "statutory", 10),
        ("rule-pi-004", "patent_invalidation", "请求人补充意见期限",
         "专利法实施细则第58条", "petitioner_submit_date", 1, "calendar_month", "patent", "statutory", 10),
        ("rule-pi-005", "patent_invalidation", "预估审限（无效）",
         "专利审查指南第4部分第3章", "filing_date", 5, "calendar_month", "patent", "recommended", 0),
    ];

    // ── 行政诉讼（行政诉讼法算法：自然日、日历月）──
    let admin_rules: Vec<(&str, &str, &str, &str, &str, i64, &str, &str, &str, i32, Option<&str>)> = vec![
        ("rule-al-001", "admin_litigation", "提交答辩状期间",
         "行政诉讼法第67条", "complaint_received_date", 15, "day", "civil", "statutory", 10, None),
        ("rule-al-002", "admin_litigation", "预估审限（简易）",
         "行政诉讼法第84条", "filing_date", 3, "calendar_month", "civil", "recommended", 5, Some("简易")),
        ("rule-al-003", "admin_litigation", "预估审限（普通）",
         "行政诉讼法第81条", "filing_date", 6, "calendar_month", "civil", "recommended", 5, Some("普通")),
        ("rule-al-004", "admin_litigation", "判决上诉期",
         "行政诉讼法第85条", "verdict_date", 15, "day", "civil", "statutory", 10, None),
        ("rule-al-005", "admin_litigation", "裁定上诉期",
         "行政诉讼法第85条", "verdict_date", 10, "day", "civil", "statutory", 10, None),
    ];

    // ── 民事侵权（民事诉讼法算法：自然日、日历月）──
    let civil_rules: Vec<(&str, &str, &str, &str, &str, i64, &str, &str, &str, i32, Option<&str>)> = vec![
        ("rule-ct-001", "civil_tort", "提交答辩状期间",
         "民事诉讼法第128条", "complaint_received_date", 15, "day", "civil", "statutory", 10, None),
        ("rule-ct-002", "civil_tort", "预估审限（简易）",
         "民事诉讼法第164条", "filing_date", 3, "calendar_month", "civil", "recommended", 5, Some("简易")),
        ("rule-ct-003", "civil_tort", "预估审限（普通）",
         "民事诉讼法第152条", "filing_date", 6, "calendar_month", "civil", "recommended", 5, Some("普通")),
        ("rule-ct-004", "civil_tort", "判决上诉期",
         "民事诉讼法第171条", "verdict_date", 15, "day", "civil", "statutory", 10, None),
        ("rule-ct-005", "civil_tort", "裁定上诉期",
         "民事诉讼法第171条", "verdict_date", 10, "day", "civil", "statutory", 10, None),
    ];

    // 插入专利无效规则
    for (id, track, name, basis, field, offset, unit, calc, source, priority) in patent_rules {
        conn.execute(
            "INSERT INTO deadline_rules (id, track, rule_name, legal_basis, trigger_field, offset_value, offset_unit, calc_method, deadline_source, auto_calculate, priority)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10)",
            rusqlite::params![id, track, name, basis, field, offset, unit, calc, source, priority],
        )?;
    }

    // 插入行政诉讼和民事规则
    for (id, track, name, basis, field, offset, unit, calc, source, priority, proc_type) in admin_rules.into_iter().chain(civil_rules) {
        conn.execute(
            "INSERT INTO deadline_rules (id, track, rule_name, legal_basis, trigger_field, offset_value, offset_unit, calc_method, deadline_source, auto_calculate, priority, procedure_types)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?11)",
            rusqlite::params![id, track, name, basis, field, offset, unit, calc, source, priority,
                proc_type.map(|p| format!(r#"["{}"]"#, p))],
        )?;
    }

    Ok(())
}
```

## 6. ID 生成

```rust
use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}
```

所有表的 `id` 字段使用 UUID v4，在 Rust 层生成，不依赖 SQLite 的 ROWID。

## 7. 核心工具函数

```rust
// src-tauri/src/db/mod.rs

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

/// 全局数据库连接（WAL 模式支持并发读）
static DB: Mutex<Option<Connection>> = Mutex::new(None);

/// 打开数据库连接
pub fn open_db() -> Result<Connection> {
    let path = db_path();
    let conn = Connection::open(&path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.execute_batch("PRAGMA busy_timeout=5000;")?;  // 5秒等待，避免 SQLITE_BUSY
    Ok(conn)
}

/// 数据库文件路径
fn db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Casy")
        .join("casy.db")
}

/// 生成 UUID v4
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 当前时间（本地时区）
pub fn now_local() -> String {
    chrono::Local::now().naive_local().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 今天日期
pub fn today() -> String {
    chrono::Local::now().naive_local().date().format("%Y-%m-%d").to_string()
}

/// 某月最后一天
pub fn days_in_month(year: i32, month: u32) -> u32 {
    if month == 12 {
        31
    } else {
        let next = chrono::NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
        let curr = chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        (next - curr).num_days() as u32
    }
}
```

```rust
// src-tauri/src/commands/mod.rs

use std::future::Future;

/// 将阻塞任务放入线程池执行
pub async fn run_blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
```

```rust
// src-tauri/src/files/mod.rs

use std::path::PathBuf;
use crate::db::Case;

/// 案件文件夹根目录
pub fn case_folder_base() -> PathBuf {
    dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Casy")
        .join("cases")
}

/// 确保案件文件夹存在，返回路径
pub fn ensure_case_folder(case: &Case) -> Result<PathBuf> {
    let base = case_folder_base();
    let case_no = case.case_no.as_deref().unwrap_or("无案号");
    let short_id = &case.id[..8.min(case.id.len())];
    let folder_name = format!("{}_{}", sanitize_filename(case_no), short_id);
    let folder = base.join(&folder_name);

    std::fs::create_dir_all(&folder)?;
    for sub in &["传票", "证据", "交文", "收文", "内部", "通信", "其他"] {
        std::fs::create_dir_all(folder.join(sub))?;
    }

    Ok(folder)
}

/// 清理文件名中的非法字符
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect()
}
```

## 8. 迁移框架

```rust
// src-tauri/src/db/migrate.rs

use rusqlite::Connection;

const MIGRATIONS: &[(&str, &str)] = &[
    ("1", "CREATE TABLE IF NOT EXISTS ..."),  // 初始 schema
    // ("2", "ALTER TABLE cases ADD COLUMN new_field TEXT;"),  // 未来迁移
    // ("3", "CREATE INDEX IF NOT EXISTS ...;"),
];

pub fn run_migrations(conn: &Connection, from_version: i64) -> Result<()> {
    for (version, sql) in MIGRATIONS {
        let v: i64 = version.parse().unwrap_or(0);
        if v > from_version {
            conn.execute_batch(sql)?;
            conn.execute_batch(&format!("PRAGMA user_version = {};", v))?;
            log::info!("Migration v{} applied", v);
        }
    }
    Ok(())
}
```

## 9. case_stats 命令

```rust
// src-tauri/src/commands/cases.rs

#[derive(Serialize)]
pub struct CaseStats {
    pub total: i64,
    pub active: i64,
    pub closed: i64,
    pub by_track: Vec<(String, i64)>,
    pub by_client: Vec<(String, i64)>,
    pub upcoming_hearings: i64,
    pub overdue_deadlines: i64,
}

#[tauri::command]
pub async fn case_stats() -> Result<CaseStats, String> {
    run_blocking(|| {
        let conn = open_db()?;
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM cases", [], |r| r.get(0))?;
        let active: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cases WHERE case_status IS NULL OR case_status != '已完结'", [], |r| r.get(0)
        )?;
        let closed = total - active;
        let by_track = db::cases::case_counts_by_track(&conn)?;
        let by_client = db::cases::case_counts_by_client(&conn)?;
        let upcoming: i64 = conn.query_row(
            "SELECT COUNT(*) FROM hearings WHERE hearing_date >= date('now')", [], |r| r.get(0)
        )?;
        Ok(CaseStats { total, active, closed, by_track, by_client, upcoming_hearings: upcoming, overdue_deadlines: 0 })
    }).await
}
```

## 10. update_case 实现

```rust
/// PATCH 语义：只更新提供的字段，不覆盖未提供的字段
pub fn update_case(conn: &Connection, id: &str, data: &serde_json::Value) -> Result<Case> {
    let mut sql = String::from("UPDATE cases SET updated_at = datetime('now','localtime')");
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    // 只处理 data 中实际提供的字段
    let fields = [
        ("case_name", "case_name"), ("case_no", "case_no"), ("cause_action", "cause_action"),
        ("client_name", "client_name"), ("opponent_name", "opponent_name"), ("court", "court"),
        ("case_level", "case_level"), ("case_progress", "case_progress"), ("case_result", "case_result"),
        ("patent_name", "patent_name"), ("patent_app_no", "patent_app_no"),
        ("filing_date", "filing_date"), ("trial_date", "trial_date"), ("verdict_date", "verdict_date"),
        ("notes", "notes"), ("our_role", "our_role"), ("opponent_role", "opponent_role"),
        // ... 所有可更新字段
    ];

    for (json_key, db_col) in &fields {
        if let Some(val) = data.get(*json_key) {
            sql.push_str(&format!(", {} = ?", db_col));
            match val {
                serde_json::Value::String(s) => params.push(Box::new(s.clone())),
                serde_json::Value::Null => params.push(Box::new(rusqlite::types::Null)),
                _ => params.push(Box::new(val.to_string())),
            }
        }
    }

    sql.push_str(" WHERE id = ?");
    params.push(Box::new(id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice())?;

    get_case(conn, id)
}
```

## 11. extract_body / extract_attachments（邮件解析辅助）

```rust
use mailparse::ParsedMail;

/// 提取邮件纯文本正文
pub fn extract_body_text(mail: &ParsedMail) -> Option<String> {
    if mail.subparts.is_empty() {
        let ct = mail.ctype.mimetype.to_lowercase();
        if ct.contains("text/plain") {
            return mail.get_body().ok();
        }
    }
    for part in &mail.subparts {
        if let Some(body) = extract_body_text(part) {
            return Some(body);
        }
    }
    None
}

/// 提取邮件 HTML 正文
pub fn extract_body_html(mail: &ParsedMail) -> Option<String> {
    if mail.subparts.is_empty() {
        let ct = mail.ctype.mimetype.to_lowercase();
        if ct.contains("text/html") {
            return mail.get_body().ok();
        }
    }
    for part in &mail.subparts {
        if let Some(body) = extract_body_html(part) {
            return Some(body);
        }
    }
    None
}

/// 提取附件信息
pub fn extract_attachments(mail: &ParsedMail) -> Vec<EmailAttachment> {
    let mut attachments = Vec::new();
    if let Some(disposition) = &mail.get_content_disposition().disposition {
        if *disposition == mailparse::DispositionType::Attachment {
            let filename = mail.get_content_disposition().params
                .get("filename")
                .cloned()
                .unwrap_or_else(|| "unnamed".to_string());
            attachments.push(EmailAttachment {
                filename,
                content_type: mail.ctype.mimetype.clone(),
                data: mail.get_body_raw().unwrap_or_default(),
            });
        }
    }
    for part in &mail.subparts {
        attachments.extend(extract_attachments(part));
    }
    attachments
}

pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}
```

## 12. rule_based_classify（本地模式文档分类）

```rust
/// 基于规则的文档分类（无需 AI）
pub fn rule_based_classify(text: &str) -> DocumentClassification {
    let text_lower = text.to_lowercase();

    // 传票
    if text.contains("传票") || (text.contains("传唤") && text.contains("开庭")) {
        return DocumentClassification {
            doc_type: "summons".to_string(),
            confidence: 0.85,
            ..Default::default()
        };
    }

    // 口审通知书
    if text.contains("口头审理通知书") || text.contains("口审") {
        return DocumentClassification {
            doc_type: "hearing_notice".to_string(),
            confidence: 0.85,
            ..Default::default()
        };
    }

    // 判决/裁定/决定
    if text.contains("判决书") || text.contains("裁定书") || text.contains("无效决定") {
        return DocumentClassification {
            doc_type: "judgment".to_string(),
            confidence: 0.8,
            ..Default::default()
        };
    }

    // 起诉状
    if text.contains("起诉状") || text.contains("行政起诉") {
        return DocumentClassification {
            doc_type: "complaint".to_string(),
            confidence: 0.8,
            ..Default::default()
        };
    }

    // 答辩状
    if text.contains("答辩状") || text.contains("答辩意见") {
        return DocumentClassification {
            doc_type: "defense".to_string(),
            confidence: 0.8,
            ..Default::default()
        };
    }

    // 默认
    DocumentClassification {
        doc_type: "other".to_string(),
        confidence: 0.3,
        ..Default::default()
    }
}

#[derive(Default)]
pub struct DocumentClassification {
    pub doc_type: String,
    pub confidence: f64,
    pub extracted: Option<serde_json::Value>,
}
