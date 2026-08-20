use rusqlite::{params, Connection};

/// 当前 Schema 版本号
#[allow(dead_code)]
pub const CURRENT_SCHEMA_VERSION: i64 = 13;

/// 完整数据库 Schema（含所有 CHECK 约束、索引、触发器、FTS 表）
pub const SCHEMA_SQL: &str = r#"
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
  case_status     TEXT,            -- 自动计算：从 case_result 推导
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

-- updated_at 自动更新触发器
CREATE TRIGGER IF NOT EXISTS trg_cases_updated
AFTER UPDATE ON cases
FOR EACH ROW
BEGIN
  UPDATE cases SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
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
-- 期限规则（法定期限模板）
-- ============================================================
CREATE TABLE IF NOT EXISTS deadline_rules (
  id              TEXT PRIMARY KEY,
  track           TEXT NOT NULL,
  rule_name       TEXT NOT NULL,
  legal_basis     TEXT NOT NULL,
  trigger_field   TEXT NOT NULL,
  offset_value    INTEGER NOT NULL,
  offset_unit     TEXT NOT NULL DEFAULT 'day'
                  CHECK(offset_unit IN ('day','calendar_month')),
  calc_method     TEXT NOT NULL DEFAULT 'civil'
                  CHECK(calc_method IN ('civil','patent')),
  procedure_types TEXT,
  deadline_source TEXT NOT NULL DEFAULT 'statutory'
                  CHECK(deadline_source IN ('statutory','recommended')),
  auto_calculate  INTEGER NOT NULL DEFAULT 1,
  priority        INTEGER DEFAULT 0,
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

-- ============================================================
-- 案件实际期限
-- ============================================================
CREATE TABLE IF NOT EXISTS case_deadlines (
  id              TEXT PRIMARY KEY,
  case_id         TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  rule_id         TEXT REFERENCES deadline_rules(id),
  deadline_name   TEXT NOT NULL,
  trigger_date    TEXT,
  due_date        TEXT NOT NULL,
  days_left       INTEGER,
  deadline_source TEXT NOT NULL DEFAULT 'statutory'
                  CHECK(deadline_source IN ('statutory','court','manual')),
  legal_basis     TEXT,
  court_order_ref TEXT,
  notes           TEXT,
  completed       INTEGER DEFAULT 0,
  completed_at    TEXT,
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

-- files_fts 全文搜索
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
  category      TEXT NOT NULL DEFAULT 'other'
                CHECK(category IN (
                  'legal_provision','case_note','email','document_summary',
                  'holiday','cause_action','court_name','judge_info',
                  'common_paragraph','law_reference',
                  'complaint','defense_brief','legal_opinion','lawyer_letter','reply_brief',
                  'other'
                )),
  content       TEXT NOT NULL,
  tags          TEXT,
  source_type   TEXT,
  source_id     TEXT,
  linked_case_id TEXT REFERENCES cases(id) ON DELETE SET NULL,

  -- 法条专用字段
  law_name      TEXT,
  article_no    TEXT,
  effective_date TEXT,
  status        TEXT DEFAULT 'current',

  created_at    TEXT DEFAULT (datetime('now','localtime')),
  updated_at    TEXT DEFAULT (datetime('now','localtime'))
);

-- 知识条目版本
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
-- idx_knowledge_law moved to migration (old DBs may not have law_name column)

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

-- 知识条目向量嵌入（用于语义检索）
CREATE TABLE IF NOT EXISTS knowledge_embeddings (
  item_id       TEXT PRIMARY KEY REFERENCES knowledge_items(id) ON DELETE CASCADE,
  embedding     BLOB NOT NULL,
  model         TEXT NOT NULL DEFAULT 'nomic-embed-text',
  dimension     INTEGER NOT NULL DEFAULT 768,
  created_at    TEXT DEFAULT (datetime('now','localtime'))
);

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
  password_enc  TEXT NOT NULL,
  use_tls       INTEGER DEFAULT 1,
  watch_folders TEXT DEFAULT 'INBOX',
  filter_from   TEXT,
  filter_subject TEXT,
  enabled       INTEGER DEFAULT 1,
  last_sync_uid TEXT,
  created_at    TEXT DEFAULT (datetime('now','localtime'))
);

-- ============================================================
-- 设置（键值对）
-- ============================================================
CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#;

/// 迁移定义: (版本号, SQL)
/// 版本 1 = 初始 schema（含所有表、索引、触发器、FTS）
#[allow(dead_code)]
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("1", SCHEMA_SQL),
    ("2", MIGRATION_V2_SQL),
    ("3", MIGRATION_V3_SQL),
    ("4", MIGRATION_V4_SQL),
    ("5", MIGRATION_V5_SQL),
    ("6", MIGRATION_V6_SQL),
    ("7", MIGRATION_V7_SQL),
    ("8", MIGRATION_V8_SQL),
    ("9", MIGRATION_V9_SQL),
    ("10", MIGRATION_V10_SQL),
    ("11", MIGRATION_V11_SQL),
    ("12", MIGRATION_V12_SQL),
    ("13", MIGRATION_V13_SQL),
];

/// 版本 2: inbox v2.1 — 重建 inbox_items、扩展 cases/tasks、新增推荐/命名表
pub const MIGRATION_V2_SQL: &str = r#"
-- ============================================================
-- inbox_items 重建（扩展 status CHECK + 新增字段）
-- ============================================================
CREATE TABLE IF NOT EXISTS inbox_items_new (
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
  status          TEXT NOT NULL DEFAULT 'pending'
                  CHECK(status IN ('pending','processed','filed','archived','ignored')),
  user_category   TEXT,
  linked_case_id  TEXT REFERENCES cases(id) ON DELETE SET NULL,
  filed_to        TEXT,
  filed_as        TEXT,
  knowledge_mark  INTEGER DEFAULT 0 CHECK(knowledge_mark IN (0,1,2)),
  parent_id       TEXT REFERENCES inbox_items(id),
  quick_category  TEXT,
  quick_confidence REAL,
  ai_analyzed     INTEGER DEFAULT 0,
  copy_progress   INTEGER,
  file_hash       TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  processed_at    TEXT
);

INSERT INTO inbox_items_new (
  id, source_type, source_path, source_url, title,
  content_text, content_html, ai_category, ai_confidence, ai_extracted,
  ai_suggested_case_id, status, user_category, linked_case_id,
  filed_to, filed_as, knowledge_mark, created_at, processed_at
)
SELECT
  id, source_type, source_path, source_url, title,
  content_text, content_html, ai_category, ai_confidence, ai_extracted,
  ai_suggested_case_id,
  CASE status
    WHEN 'processing' THEN 'pending'
    WHEN 'dismissed'  THEN 'ignored'
    ELSE status
  END,
  user_category, linked_case_id,
  filed_to, filed_as, knowledge_mark, created_at, processed_at
FROM inbox_items;

DROP TABLE inbox_items;
ALTER TABLE inbox_items_new RENAME TO inbox_items;

CREATE INDEX IF NOT EXISTS idx_inbox_status ON inbox_items(status);
CREATE INDEX IF NOT EXISTS idx_inbox_category ON inbox_items(ai_category);
CREATE INDEX IF NOT EXISTS idx_inbox_case ON inbox_items(linked_case_id);

-- ============================================================
-- cases 表新增 folder_name / display_name
-- ============================================================
ALTER TABLE cases ADD COLUMN folder_name TEXT;
ALTER TABLE cases ADD COLUMN display_name TEXT;

-- ============================================================
-- tasks 表新增 inbox_source_id
-- ============================================================
ALTER TABLE tasks ADD COLUMN inbox_source_id TEXT REFERENCES inbox_items(id);

-- ============================================================
-- inbox_recommendations 表
-- ============================================================
CREATE TABLE IF NOT EXISTS inbox_recommendations (
  id              TEXT PRIMARY KEY,
  inbox_item_id   TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
  action          TEXT NOT NULL,
  target_case_id  TEXT,
  target_folder   TEXT,
  reason          TEXT,
  confidence      REAL,
  accepted        INTEGER,
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_inbox_rec_item ON inbox_recommendations(inbox_item_id);

-- ============================================================
-- file_naming_rules 表
-- ============================================================
CREATE TABLE IF NOT EXISTS file_naming_rules (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  template    TEXT NOT NULL,
  pattern     TEXT NOT NULL,
  is_default  INTEGER DEFAULT 0,
  created_at  TEXT DEFAULT (datetime('now','localtime')),
  updated_at  TEXT DEFAULT (datetime('now','localtime'))
);

INSERT OR IGNORE INTO file_naming_rules (id, name, template, pattern, is_default)
VALUES ('rule-default', '四段式', 'four_segment', '{case_no}_{client}_{user}_{date}', 1);
"#;

/// 版本 3: 飞书同步增强 — 字段元数据、链接缓存、公式缓存列、配置表
pub const MIGRATION_V3_SQL: &str = r#"
-- ============================================================
-- 飞书字段元数据表 (§5.1)
-- ============================================================
CREATE TABLE IF NOT EXISTS feishu_field_meta (
  id              TEXT PRIMARY KEY,
  table_id        TEXT NOT NULL,
  table_name      TEXT NOT NULL,
  field_id        TEXT NOT NULL,
  field_name      TEXT NOT NULL,
  field_type      INTEGER NOT NULL,
  ui_type         TEXT NOT NULL,
  is_primary      INTEGER DEFAULT 0,
  formula_expression TEXT,
  property_json   TEXT,
  local_table     TEXT,
  local_column    TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime')),
  UNIQUE(table_id, field_id)
);

CREATE INDEX IF NOT EXISTS idx_feishu_meta_table ON feishu_field_meta(table_id);

CREATE TRIGGER IF NOT EXISTS trg_feishu_meta_updated
AFTER UPDATE ON feishu_field_meta FOR EACH ROW
BEGIN
  UPDATE feishu_field_meta SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- 飞书链接缓存 (§5.2)
-- ============================================================
CREATE TABLE IF NOT EXISTS feishu_link_cache (
  id              TEXT PRIMARY KEY,
  source_table    TEXT NOT NULL,
  source_field    TEXT NOT NULL,
  source_record   TEXT NOT NULL,
  target_table    TEXT NOT NULL,
  target_record   TEXT NOT NULL,
  link_type       TEXT NOT NULL DEFAULT 'duplex'
                  CHECK(link_type IN ('duplex','single')),
  synced_at       TEXT DEFAULT (datetime('now','localtime')),
  UNIQUE(source_table, source_field, source_record, target_record)
);

-- ============================================================
-- 飞书配置表 (§5.6)
-- ============================================================
CREATE TABLE IF NOT EXISTS feishu_base_config (
  id              TEXT PRIMARY KEY,
  app_token       TEXT NOT NULL,
  base_name       TEXT,
  table_mappings  TEXT NOT NULL,
  sync_direction  TEXT DEFAULT 'bidirectional'
                  CHECK(sync_direction IN ('pull_only','push_only','bidirectional')),
  last_full_sync  TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

-- ============================================================
-- cases 表新增公式缓存列 (§5.3)
-- ============================================================
ALTER TABLE cases ADD COLUMN formula_case_status TEXT;
ALTER TABLE cases ADD COLUMN formula_defense_deadline TEXT;
ALTER TABLE cases ADD COLUMN formula_estimated_trial_limit TEXT;
ALTER TABLE cases ADD COLUMN formula_petitioner_first TEXT;
ALTER TABLE cases ADD COLUMN formula_petitioner_supp TEXT;
ALTER TABLE cases ADD COLUMN formula_petitioner_reply TEXT;
ALTER TABLE cases ADD COLUMN formula_patentee_statement TEXT;
ALTER TABLE cases ADD COLUMN formula_patentee_supp TEXT;

-- ============================================================
-- hearings 表新增公式缓存列 (§5.3)
-- ============================================================
ALTER TABLE hearings ADD COLUMN formula_status TEXT;

-- ============================================================
-- tasks 表新增公式缓存列 (§5.3)
-- ============================================================
ALTER TABLE tasks ADD COLUMN formula_days_until_deadline TEXT;

-- ============================================================
-- cases 表新增关联案件 ID (§5.5)
-- ============================================================
ALTER TABLE cases ADD COLUMN related_case_ids TEXT;
"#;

/// 版本 4: 飞书通用同步 v3.0 — 连接管理、表结构缓存、字段映射
pub const MIGRATION_V4_SQL: &str = r#"
-- 飞书连接配置
CREATE TABLE IF NOT EXISTS feishu_connections (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    app_id          TEXT NOT NULL,
    app_secret      TEXT NOT NULL,
    app_token       TEXT,
    base_name       TEXT,
    status          TEXT DEFAULT 'disconnected'
                    CHECK(status IN ('connected','disconnected','error')),
    last_sync_at    TEXT,
    created_at      TEXT DEFAULT (datetime('now','localtime')),
    updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE TRIGGER IF NOT EXISTS trg_feishu_connections_updated
AFTER UPDATE ON feishu_connections FOR EACH ROW
BEGIN
    UPDATE feishu_connections SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- 飞书表结构缓存
CREATE TABLE IF NOT EXISTS feishu_tables (
    id              TEXT PRIMARY KEY,
    connection_id   TEXT NOT NULL REFERENCES feishu_connections(id) ON DELETE CASCADE,
    table_id        TEXT NOT NULL,
    table_name      TEXT NOT NULL,
    field_count     INTEGER,
    record_count    INTEGER,
    revision        INTEGER,
    synced_at       TEXT DEFAULT (datetime('now','localtime')),
    UNIQUE(connection_id, table_id)
);

-- 飞书字段定义缓存
CREATE TABLE IF NOT EXISTS feishu_fields (
    id              TEXT PRIMARY KEY,
    table_id        TEXT NOT NULL,
    field_id        TEXT NOT NULL,
    field_name      TEXT NOT NULL,
    field_type      INTEGER NOT NULL,
    type_name       TEXT NOT NULL,
    is_primary      INTEGER DEFAULT 0,
    property_json   TEXT,
    formula_expr    TEXT,
    created_at      TEXT DEFAULT (datetime('now','localtime')),
    UNIQUE(table_id, field_id)
);

-- 字段映射表（连接飞书和本地的桥梁）
CREATE TABLE IF NOT EXISTS feishu_field_mappings (
    id              TEXT PRIMARY KEY,
    connection_id   TEXT NOT NULL REFERENCES feishu_connections(id) ON DELETE CASCADE,
    feishu_table_id TEXT NOT NULL,
    feishu_field_id TEXT NOT NULL,
    feishu_field_name TEXT NOT NULL,
    feishu_field_type INTEGER NOT NULL,
    local_table     TEXT NOT NULL,
    local_column    TEXT NOT NULL,
    transform_rule  TEXT,
    sync_direction  TEXT DEFAULT 'bidirectional'
                    CHECK(sync_direction IN ('pull_only','push_only','bidirectional','none')),
    is_formula      INTEGER DEFAULT 0,
    is_link         INTEGER DEFAULT 0,
    is_lookup       INTEGER DEFAULT 0,
    created_at      TEXT DEFAULT (datetime('now','localtime')),
    updated_at      TEXT DEFAULT (datetime('now','localtime')),
    UNIQUE(connection_id, feishu_table_id, feishu_field_id)
);

CREATE TRIGGER IF NOT EXISTS trg_feishu_field_mappings_updated
AFTER UPDATE ON feishu_field_mappings FOR EACH ROW
BEGIN
    UPDATE feishu_field_mappings SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;
"#;

/// 版本 5: 多通道提醒系统 + 任务模板
pub const MIGRATION_V5_SQL: &str = r#"
-- ============================================================
-- 提醒规则
-- ============================================================
CREATE TABLE IF NOT EXISTS reminder_rules (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    trigger_type    TEXT NOT NULL
                    CHECK(trigger_type IN (
                        'deadline_before',
                        'deadline_on',
                        'deadline_after',
                        'hearing_before',
                        'task_due',
                        'task_overdue'
                    )),
    trigger_value   INTEGER,
    channels        TEXT NOT NULL,
    message_template TEXT,
    case_types      TEXT,
    enabled         INTEGER DEFAULT 1,
    created_at      TEXT DEFAULT (datetime('now','localtime'))
);

-- ============================================================
-- 提醒日志
-- ============================================================
CREATE TABLE IF NOT EXISTS reminder_log (
    id              TEXT PRIMARY KEY,
    rule_id         TEXT NOT NULL REFERENCES reminder_rules(id),
    case_id         TEXT,
    task_id         TEXT,
    channel         TEXT NOT NULL,
    message         TEXT NOT NULL,
    level           TEXT CHECK(level IN ('R1','R2','R3','R4')),
    status          TEXT DEFAULT 'sent'
                    CHECK(status IN ('sent','failed','snoozed')),
    sent_at         TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_reminder_log_rule ON reminder_log(rule_id);
CREATE INDEX IF NOT EXISTS idx_reminder_log_sent ON reminder_log(sent_at);

-- ============================================================
-- 任务模板
-- ============================================================
CREATE TABLE IF NOT EXISTS task_templates (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    trigger_type    TEXT,
    tasks_json      TEXT NOT NULL,
    case_types      TEXT,
    enabled         INTEGER DEFAULT 1,
    created_at      TEXT DEFAULT (datetime('now','localtime'))
);

-- ============================================================
-- tasks 表新增 feishu_task_id 列
-- ============================================================
ALTER TABLE tasks ADD COLUMN feishu_task_id TEXT;

-- ============================================================
-- 默认提醒规则
-- ============================================================
INSERT INTO reminder_rules (id, name, trigger_type, trigger_value, channels) VALUES
('rule-1', '期限前7天提醒', 'deadline_before', 7, '["feishu_message"]'),
('rule-2', '期限前3天紧急提醒', 'deadline_before', 3, '["local","feishu_message","feishu_task"]'),
('rule-3', '期限当天强提醒', 'deadline_on', 0, '["local","system","feishu_message","feishu_task"]'),
('rule-4', '开庭前7天准备提醒', 'hearing_before', 7, '["feishu_message"]'),
('rule-5', '开庭前1天最终提醒', 'hearing_before', 1, '["local","system","feishu_message","feishu_task"]'),
('rule-6', '任务到期提醒', 'task_due', 0, '["local","feishu_message"]');
"#;

/// 版本 6: 动态字段系统 + 跨类型筛选视图
pub const MIGRATION_V6_SQL: &str = r#"
-- ============================================================
-- 动态字段分组
-- ============================================================
CREATE TABLE IF NOT EXISTS field_groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    case_types TEXT,   -- JSON: ["专利无效"] or null for all
    court_levels TEXT, -- JSON: ["国知局"] or null for all
    sort_order INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now','localtime'))
);

CREATE TABLE IF NOT EXISTS field_group_items (
    id TEXT PRIMARY KEY,
    group_id TEXT NOT NULL REFERENCES field_groups(id) ON DELETE CASCADE,
    column_name TEXT NOT NULL,
    label TEXT NOT NULL,
    field_type TEXT NOT NULL DEFAULT 'text',
    options TEXT,  -- JSON: for select type
    required INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    UNIQUE(group_id, column_name)
);

CREATE INDEX IF NOT EXISTS idx_field_group_items_group ON field_group_items(group_id);

-- 预置字段分组（中国专利法）
INSERT INTO field_groups (id, name, description, case_types, sort_order) VALUES
('fg-common', '通用字段', '所有案件类型通用', NULL, 0),
('fg-invalidation', '专利无效专用', '无效宣告案件专用字段', '["专利无效"]', 1),
('fg-infringement', '专利侵权专用', '侵权诉讼案件专用字段', '["专利侵权","侵害发明专利权","侵害实用新型专利权"]', 2),
('fg-administrative', '行政诉讼专用', '行政诉讼案件专用字段', '["专利行政","行政诉讼"]', 3);

-- 通用字段
INSERT INTO field_group_items (id, group_id, column_name, label, field_type, sort_order) VALUES
('fgi-c1', 'fg-common', 'case_name', '案件名称', 'text', 0),
('fgi-c2', 'fg-common', 'case_no', '案号', 'text', 1),
('fgi-c3', 'fg-common', 'client_name', '客户名称', 'text', 2),
('fgi-c4', 'fg-common', 'opponent_name', '对方名称', 'text', 3),
('fgi-c5', 'fg-common', 'our_role', '我方诉讼地位', 'select', 4),
('fgi-c6', 'fg-common', 'opponent_role', '对方诉讼地位', 'select', 5),
('fgi-c7', 'fg-common', 'court', '审理机关', 'select', 6),
('fgi-c8', 'fg-common', 'case_level', '审级', 'select', 7),
('fgi-c9', 'fg-common', 'attorneys', '办案人', 'text', 8),
('fgi-c10', 'fg-common', 'filing_date', '立案时间', 'date', 9),
('fgi-c11', 'fg-common', 'trial_date', '开庭/口审', 'date', 10),
('fgi-c12', 'fg-common', 'notes', '备注', 'textarea', 11);

-- 专利无效专用字段
INSERT INTO field_group_items (id, group_id, column_name, label, field_type, sort_order) VALUES
('fgi-i1', 'fg-invalidation', 'petitioner_first_invalid', '请求人首次无效时间', 'date', 0),
('fgi-i2', 'fg-invalidation', 'petitioner_received_date', '请求人收到专利权人意见时间', 'date', 1),
('fgi-i3', 'fg-invalidation', 'patentee_received_date', '专利权人收到受通时间', 'date', 2),
('fgi-i4', 'fg-invalidation', 'patentee_received_supp_date', '专利权人收到补充意见时间', 'date', 3),
('fgi-i5', 'fg-invalidation', 'formula_petitioner_first', '请求人首次无效（公式）', 'date', 4),
('fgi-i6', 'fg-invalidation', 'formula_petitioner_supp', '请求人补充意见期限（公式）', 'date', 5),
('fgi-i7', 'fg-invalidation', 'formula_petitioner_reply', '请求人答复意见期限（公式）', 'date', 6),
('fgi-i8', 'fg-invalidation', 'formula_patentee_statement', '专利权人陈述意见期限（公式）', 'date', 7),
('fgi-i9', 'fg-invalidation', 'formula_patentee_supp', '专利权人补充意见时间（公式）', 'date', 8);

-- 专利侵权专用字段
INSERT INTO field_group_items (id, group_id, column_name, label, field_type, sort_order) VALUES
('fgi-n1', 'fg-infringement', 'complaint_received_date', '收到起诉状时间', 'date', 0),
('fgi-n2', 'fg-infringement', 'formula_defense_deadline', '提交答辩状期间（公式）', 'date', 1),
('fgi-n3', 'fg-infringement', 'procedure_type', '诉讼程序', 'select', 2),
('fgi-n4', 'fg-infringement', 'stay_date', '裁定中止日', 'date', 3),
('fgi-n5', 'fg-infringement', 'formula_estimated_trial_limit', '预估审限（公式）', 'date', 4),
('fgi-n6', 'fg-infringement', 'patent_name', '专利名称', 'text', 5),
('fgi-n7', 'fg-infringement', 'patent_app_no', '专利申请号', 'text', 6);

-- 行政诉讼专用字段
INSERT INTO field_group_items (id, group_id, column_name, label, field_type, sort_order) VALUES
('fgi-a1', 'fg-administrative', 'relief_deadline', '救济期限', 'date', 0),
('fgi-a2', 'fg-administrative', 'verdict_type', '判决/裁定', 'select', 1),
('fgi-a3', 'fg-administrative', 'verdict_date', '判决日期', 'date', 2);

-- ============================================================
-- 跨类型统一筛选视图
-- ============================================================
CREATE VIEW IF NOT EXISTS v_case_unified AS
SELECT
    c.id,
    c.case_name,
    c.case_no,
    c.client_name,
    c.cause_action,
    c.track,
    c.case_status AS status,
    c.court,
    c.case_level,
    c.attorneys AS operator,
    c.trial_date,
    c.filing_date,
    -- 统一期限：取每种类型对应的期限字段
    COALESCE(
        CASE WHEN c.cause_action LIKE '%无效%' THEN c.formula_petitioner_supp END,
        CASE WHEN c.cause_action LIKE '%侵权%' OR c.cause_action LIKE '%侵害%' THEN c.formula_defense_deadline END,
        CASE WHEN c.cause_action LIKE '%行政%' THEN c.relief_deadline END,
        c.formula_estimated_trial_limit
    ) AS next_deadline,
    -- 统一庭审日期
    c.trial_date AS next_hearing,
    c.updated_at
FROM cases c;
"#;

/// 版本 7: 案件文件夹模板系统
pub const MIGRATION_V7_SQL: &str = r#"
-- ============================================================
-- 案件文件夹模板
-- ============================================================
CREATE TABLE IF NOT EXISTS case_folder_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    case_type TEXT NOT NULL,
    is_builtin INTEGER DEFAULT 0,
    directories_json TEXT NOT NULL,
    file_naming_json TEXT,
    created_at TEXT DEFAULT (datetime('now','localtime'))
);

ALTER TABLE cases ADD COLUMN folder_template_id TEXT;

-- 内置模板
INSERT INTO case_folder_templates (id, name, case_type, is_builtin, directories_json) VALUES
('tpl-litigation', '诉讼案件（默认）', 'litigation', 1, '[{"id":"01","name":"委托材料","desc":"委托合同、授权书、身份证明"},{"id":"02","name":"案件分析","desc":"案件分析、争议焦点、诉讼策略"},{"id":"03","name":"法律研究","desc":"法条检索、判例研究"},{"id":"04","name":"客户提供","desc":"客户提供的所有材料"},{"id":"05","name":"证据材料","desc":"证据清单、质证意见"},{"id":"06","name":"法律文书","desc":"起诉状、答辩状、代理词"},{"id":"07","name":"对方提交","desc":"对方当事人提交的材料"},{"id":"08","name":"法院文书","desc":"传票、判决书、裁定书、送达文书"},{"id":"09","name":"庭审材料","desc":"庭审笔录、庭后分析"},{"id":"10","name":"综合报告","desc":"进展报告、客户汇报"},{"id":"11","name":"其他","desc":"辅助性参考材料"}]'),
('tpl-patent', '专利案件（默认）', 'patent', 1, '[{"id":"01","name":"委托材料","desc":"代理委托书、合同、工作记录"},{"id":"02","name":"申请清单","desc":"拟申请专利清单"},{"id":"03","name":"客户提供","desc":"技术交底书、现有技术资料"},{"id":"04","name":"律师工作","desc":"检索报告、分析、申请规划"},{"id":"05","name":"申请文件","desc":"请求书、说明书、权利要求书"},{"id":"06","name":"国知局文件","desc":"受理通知书、审查意见、授权通知"},{"id":"07","name":"对方提交","desc":"对方意见、无效请求"},{"id":"08","name":"证据材料","desc":"证据清单、对比文件"},{"id":"09","name":"财务","desc":"代理费发票、官费凭证"}]'),
('tpl-trademark', '商标案件（默认）', 'trademark', 1, '[{"id":"01","name":"委托材料","desc":"委托书、合同、工作记录"},{"id":"02","name":"商标图样","desc":"商标图样、设计稿"},{"id":"03","name":"申请文件","desc":"申请书、商品清单"},{"id":"04","name":"律师工作","desc":"检索报告、分析、策略"},{"id":"05","name":"官方文书","desc":"受理通知书、驳回决定"},{"id":"06","name":"商标注册证","desc":"注册证、续展证明"},{"id":"07","name":"证据材料","desc":"异议/无效证据"},{"id":"08","name":"对方提交","desc":"对方意见、答辩"},{"id":"09","name":"财务","desc":"代理费发票、官费凭证"}]'),
('tpl-consultation', '咨询/其他（默认）', 'consultation', 1, '[{"id":"01","name":"客户材料","desc":"客户提供的所有材料"},{"id":"02","name":"工作文件","desc":"律师工作产出"},{"id":"03","name":"其他","desc":"辅助性材料"}]');

-- 文件命名默认设置
INSERT OR IGNORE INTO settings (key, value) VALUES
('folder_naming_date_format', '"YYYY-MM-DD"'),
('folder_naming_case_no_format', '"{case_no}_{short_id}"'),
('folder_naming_file_format', '"{date}_{category}_{case_no}_{hash}.{ext}"');
"#;

/// 版本 8: 双轨状态机 + 审级历程 + 批量处理队列
pub const MIGRATION_V8_SQL: &str = r#"
-- ============================================================
-- cases 表：双轨状态机字段
-- ============================================================
ALTER TABLE cases ADD COLUMN case_route TEXT NOT NULL DEFAULT '民事诉讼'
  CHECK(case_route IN ('民事诉讼','专利无效','行政诉讼','民事诉讼+专利无效','专利无效+行政诉讼','三轨并行'));

ALTER TABLE cases ADD COLUMN civil_status TEXT DEFAULT 'intake'
  CHECK(civil_status IN ('intake','filed','pre_hearing','in_trial','settled','awaiting_verdict','verdict_issued','appeal_period','second_instance','second_verdict','retrial','enforcement','suspended','closed'));

ALTER TABLE cases ADD COLUMN invalidation_status TEXT
  CHECK(invalidation_status IN ('preparing','filed','pre_oral','oral_done','awaiting_decision','decision_issued'));

ALTER TABLE cases ADD COLUMN admin_status TEXT
  CHECK(admin_status IN ('filed','pre_hearing','in_trial','awaiting_verdict','verdict_issued','second_instance','closed'));

-- 无效程序新增日期
ALTER TABLE cases ADD COLUMN invalidation_decision_date TEXT;
ALTER TABLE cases ADD COLUMN invalidation_decision_type TEXT;

-- 行政诉讼新增日期
ALTER TABLE cases ADD COLUMN admin_filing_date TEXT;
ALTER TABLE cases ADD COLUMN admin_verdict_date TEXT;
ALTER TABLE cases ADD COLUMN admin_trial2_date TEXT;

-- 新增索引
CREATE INDEX IF NOT EXISTS idx_cases_route ON cases(case_route);
CREATE INDEX IF NOT EXISTS idx_cases_civil_status ON cases(civil_status);
CREATE INDEX IF NOT EXISTS idx_cases_invalidation_status ON cases(invalidation_status);
CREATE INDEX IF NOT EXISTS idx_cases_admin_status ON cases(admin_status);

-- ============================================================
-- 审级历程表
-- ============================================================
CREATE TABLE IF NOT EXISTS case_track_history (
  id          TEXT PRIMARY KEY,
  case_id     TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
  track       TEXT NOT NULL CHECK(track IN ('民事诉讼','专利无效','行政诉讼')),
  from_status TEXT,
  to_status   TEXT NOT NULL,
  changed_at  TEXT NOT NULL DEFAULT (datetime('now','localtime')),
  source      TEXT NOT NULL DEFAULT 'manual' CHECK(source IN ('manual','auto','ai')),
  note        TEXT
);

CREATE INDEX IF NOT EXISTS idx_track_history_case ON case_track_history(case_id, track);

-- ============================================================
-- 收件箱批量处理字段
-- ============================================================
ALTER TABLE inbox_items ADD COLUMN retry_count INTEGER DEFAULT 0;
ALTER TABLE inbox_items ADD COLUMN last_error TEXT;
ALTER TABLE inbox_items ADD COLUMN processing_started_at TEXT;

-- 扩展 status CHECK（保留现有值，新增 processing/failed）
-- 注意：SQLite 不支持 ALTER CHECK，需要重建表
CREATE TABLE IF NOT EXISTS inbox_items_v8 (
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
  status          TEXT NOT NULL DEFAULT 'pending'
                  CHECK(status IN ('pending','processing','processed','filed','archived','ignored','failed')),
  user_category   TEXT,
  linked_case_id  TEXT REFERENCES cases(id) ON DELETE SET NULL,
  filed_to        TEXT,
  filed_as        TEXT,
  knowledge_mark  INTEGER DEFAULT 0 CHECK(knowledge_mark IN (0,1,2)),
  parent_id       TEXT REFERENCES inbox_items(id),
  quick_category  TEXT,
  quick_confidence REAL,
  ai_analyzed     INTEGER DEFAULT 0,
  copy_progress   INTEGER,
  file_hash       TEXT,
  retry_count     INTEGER DEFAULT 0,
  last_error      TEXT,
  processing_started_at TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  processed_at    TEXT
);

INSERT INTO inbox_items_v8 (
  id, source_type, source_path, source_url, title,
  content_text, content_html, ai_category, ai_confidence, ai_extracted,
  ai_suggested_case_id, status, user_category, linked_case_id,
  filed_to, filed_as, knowledge_mark, parent_id, quick_category,
  quick_confidence, ai_analyzed, copy_progress, file_hash,
  created_at, processed_at
)
SELECT
  id, source_type, source_path, source_url, title,
  content_text, content_html, ai_category, ai_confidence, ai_extracted,
  ai_suggested_case_id, status, user_category, linked_case_id,
  filed_to, filed_as, knowledge_mark, parent_id, quick_category,
  quick_confidence, ai_analyzed, copy_progress, file_hash,
  created_at, processed_at
FROM inbox_items;

DROP TABLE inbox_items;
ALTER TABLE inbox_items_v8 RENAME TO inbox_items;

CREATE INDEX IF NOT EXISTS idx_inbox_status ON inbox_items(status);
CREATE INDEX IF NOT EXISTS idx_inbox_category ON inbox_items(ai_category);
CREATE INDEX IF NOT EXISTS idx_inbox_case ON inbox_items(linked_case_id);

-- 知识库 law_name 索引在 run_migrations 中条件创建（旧 DB 可能缺少该列）

-- 处理队列视图
CREATE VIEW IF NOT EXISTS v_inbox_queue AS
SELECT id, title, source_type, content_text, source_path, retry_count
FROM inbox_items
WHERE status IN ('pending', 'failed')
ORDER BY
  CASE source_type
    WHEN 'manual' THEN 1
    WHEN 'paste' THEN 2
    WHEN 'file' THEN 3
    WHEN 'email' THEN 4
    WHEN 'imap' THEN 5
    WHEN 'sms' THEN 6
    WHEN 'note' THEN 7
    ELSE 8
  END,
  created_at ASC;
"#;

/// 版本 9: GTD 化改造 — 任务/案件 GTD 字段 + 领域 + 智伴/审计新表
pub const MIGRATION_V9_SQL: &str = r#"
-- ============================================================
-- 领域表（areas）—— 长期业务方向
-- ============================================================
CREATE TABLE IF NOT EXISTS areas (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL UNIQUE,
  description     TEXT,
  icon            TEXT,
  sort_order      INTEGER DEFAULT 0,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE TRIGGER IF NOT EXISTS trg_areas_updated
AFTER UPDATE ON areas FOR EACH ROW
BEGIN
  UPDATE areas SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- 种子数据：默认领域
INSERT OR IGNORE INTO areas (id, name, description, sort_order) VALUES
  ('area-patent-litigation', '专利诉讼', '专利侵权、无效等诉讼业务', 1),
  ('area-patent-invalidation', '专利无效', '专利无效宣告程序', 2),
  ('area-admin-litigation', '行政诉讼', '行政诉讼业务', 3),
  ('area-advisory', '顾问咨询', '常年顾问、法律咨询', 4);

-- ============================================================
-- tasks 表：GTD 化字段
-- ============================================================
-- 任务类型
ALTER TABLE tasks ADD COLUMN task_type TEXT DEFAULT 'action'
  CHECK(task_type IN ('action','waiting','delegated','someday'));

-- 时间双轨
ALTER TABLE tasks ADD COLUMN start_date TEXT;
ALTER TABLE tasks ADD COLUMN due_date TEXT;

-- 等待/委派
ALTER TABLE tasks ADD COLUMN waiting_for TEXT;
ALTER TABLE tasks ADD COLUMN follow_up_date TEXT;

-- 上下文
ALTER TABLE tasks ADD COLUMN context TEXT;

-- 旗标
ALTER TABLE tasks ADD COLUMN flagged INTEGER DEFAULT 0 CHECK(flagged IN (0,1));

-- 顺序项目
ALTER TABLE tasks ADD COLUMN sequential INTEGER DEFAULT 0 CHECK(sequential IN (0,1));
ALTER TABLE tasks ADD COLUMN blocked INTEGER DEFAULT 0 CHECK(blocked IN (0,1));
ALTER TABLE tasks ADD COLUMN sequence_order INTEGER DEFAULT 0;

-- 时间桶
ALTER TABLE tasks ADD COLUMN start_bucket TEXT DEFAULT 'anytime'
  CHECK(start_bucket IN ('inbox','anytime','someday','today'));

-- Today 排序
ALTER TABLE tasks ADD COLUMN today_index INTEGER DEFAULT 0;

-- 时间预估/实际
ALTER TABLE tasks ADD COLUMN estimated_minutes INTEGER;
ALTER TABLE tasks ADD COLUMN actual_minutes INTEGER;

-- 缓存标志
ALTER TABLE tasks ADD COLUMN is_overdue INTEGER DEFAULT 0 CHECK(is_overdue IN (0,1));
ALTER TABLE tasks ADD COLUMN due_soon INTEGER DEFAULT 0 CHECK(due_soon IN (0,1));

-- 回顾
ALTER TABLE tasks ADD COLUMN last_review_date TEXT;
ALTER TABLE tasks ADD COLUMN next_review_date TEXT;

-- 关联
ALTER TABLE tasks ADD COLUMN area_id TEXT REFERENCES areas(id);
ALTER TABLE tasks ADD COLUMN knowledge_id TEXT REFERENCES knowledge_items(id);

-- 更新时间
ALTER TABLE tasks ADD COLUMN updated_at TEXT DEFAULT (datetime('now','localtime'));

-- 新增索引
CREATE INDEX IF NOT EXISTS idx_tasks_type ON tasks(task_type);
CREATE INDEX IF NOT EXISTS idx_tasks_start_date ON tasks(start_date);
CREATE INDEX IF NOT EXISTS idx_tasks_due_date ON tasks(due_date);
CREATE INDEX IF NOT EXISTS idx_tasks_bucket ON tasks(start_bucket);
CREATE INDEX IF NOT EXISTS idx_tasks_flagged ON tasks(flagged);
CREATE INDEX IF NOT EXISTS idx_tasks_blocked ON tasks(blocked);
CREATE INDEX IF NOT EXISTS idx_tasks_overdue ON tasks(is_overdue);
CREATE INDEX IF NOT EXISTS idx_tasks_area ON tasks(area_id);

-- tasks updated_at 触发器
CREATE TRIGGER IF NOT EXISTS trg_tasks_updated
AFTER UPDATE ON tasks FOR EACH ROW
BEGIN
  UPDATE tasks SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- cases 表：GTD 化字段
-- ============================================================
-- 顺序项目
ALTER TABLE cases ADD COLUMN sequential INTEGER DEFAULT 1 CHECK(sequential IN (0,1));
ALTER TABLE cases ADD COLUMN next_action_id TEXT REFERENCES tasks(id);

-- 统计缓存
ALTER TABLE cases ADD COLUMN overdue_task_count INTEGER DEFAULT 0;
ALTER TABLE cases ADD COLUMN remaining_task_count INTEGER DEFAULT 0;

-- 回顾
ALTER TABLE cases ADD COLUMN next_review_date TEXT;

-- 客户/领域关联
ALTER TABLE cases ADD COLUMN client_id TEXT REFERENCES clients(id);
ALTER TABLE cases ADD COLUMN area_id TEXT REFERENCES areas(id);

-- 案件类型（计算/探索/成长）
ALTER TABLE cases ADD COLUMN case_type TEXT DEFAULT 'exploratory'
  CHECK(case_type IN ('computational','exploratory','growth'));

-- 案件目标（30字内）
ALTER TABLE cases ADD COLUMN case_goal TEXT;

-- 新增索引
CREATE INDEX IF NOT EXISTS idx_cases_sequential ON cases(sequential);
CREATE INDEX IF NOT EXISTS idx_cases_next_action ON cases(next_action_id);
CREATE INDEX IF NOT EXISTS idx_cases_review_date ON cases(next_review_date);
CREATE INDEX IF NOT EXISTS idx_cases_client_id ON cases(client_id);
CREATE INDEX IF NOT EXISTS idx_cases_area_id ON cases(area_id);
CREATE INDEX IF NOT EXISTS idx_cases_case_type ON cases(case_type);

-- ============================================================
-- 行为事件表（task_events）—— 学习数据
-- ============================================================
CREATE TABLE IF NOT EXISTS task_events (
  id              TEXT PRIMARY KEY,
  task_id         TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  event_type      TEXT NOT NULL CHECK(event_type IN (
    'created','completed','deferred','snoozed','reminded',
    'overdue','escalated','cancelled','moved'
  )),
  occurred_at     TEXT NOT NULL DEFAULT (datetime('now','localtime')),
  payload         TEXT,  -- JSON
  actor           TEXT DEFAULT 'user' CHECK(actor IN ('user','ai','system'))
);

CREATE INDEX IF NOT EXISTS idx_task_events_task ON task_events(task_id);
CREATE INDEX IF NOT EXISTS idx_task_events_type ON task_events(event_type);
CREATE INDEX IF NOT EXISTS idx_task_events_time ON task_events(occurred_at);

-- ============================================================
-- 决策记录表（decisions）
-- ============================================================
CREATE TABLE IF NOT EXISTS decisions (
  id              TEXT PRIMARY KEY,
  entity_type     TEXT NOT NULL CHECK(entity_type IN ('case','client','task','knowledge')),
  entity_id       TEXT NOT NULL,
  decision_type   TEXT NOT NULL CHECK(decision_type IN (
    'appeal','settle','accept','refuse','other',
    'recommend_today','recommend_priority','recommend_estimate',
    'recommend_schedule','recommend_action','recommend_followup'
  )),
  decision        TEXT NOT NULL,
  basis           TEXT,  -- JSON: 决策依据
  ai_advice       TEXT,  -- AI 建议留档
  ai_model        TEXT,
  source_ref      TEXT,  -- JSON: 依据来源
  status          TEXT DEFAULT 'proposed' CHECK(status IN ('proposed','confirmed','rejected','voided')),
  recursive_checked INTEGER DEFAULT 0 CHECK(recursive_checked IN (0,1)),
  confirmed_at    TEXT,
  review_due      TEXT,
  reviewed_at     TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_decisions_entity ON decisions(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_decisions_type ON decisions(decision_type);
CREATE INDEX IF NOT EXISTS idx_decisions_status ON decisions(status);

CREATE TRIGGER IF NOT EXISTS trg_decisions_updated
AFTER UPDATE ON decisions FOR EACH ROW
BEGIN
  UPDATE decisions SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- AI 审计表（ai_runs + ai_context_items）
-- ============================================================
CREATE TABLE IF NOT EXISTS ai_runs (
  id              TEXT PRIMARY KEY,
  provider        TEXT NOT NULL,
  model           TEXT NOT NULL,
  purpose         TEXT NOT NULL,
  prompt_version  TEXT,
  status          TEXT DEFAULT 'pending' CHECK(status IN ('pending','running','completed','failed')),
  input_hash      TEXT,
  output_hash     TEXT,
  job_id          TEXT,
  error_message   TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  completed_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_ai_runs_purpose ON ai_runs(purpose);
CREATE INDEX IF NOT EXISTS idx_ai_runs_model ON ai_runs(model);
CREATE INDEX IF NOT EXISTS idx_ai_runs_status ON ai_runs(status);

CREATE TABLE IF NOT EXISTS ai_context_items (
  id              TEXT PRIMARY KEY,
  run_id          TEXT NOT NULL REFERENCES ai_runs(id) ON DELETE CASCADE,
  source_type     TEXT NOT NULL,
  source_id       TEXT NOT NULL,
  source_field    TEXT,
  content_hash    TEXT,
  snapshot_version TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_ai_context_run ON ai_context_items(run_id);

-- ============================================================
-- 领域事件表（audit_events）
-- ============================================================
CREATE TABLE IF NOT EXISTS audit_events (
  id              TEXT PRIMARY KEY,
  aggregate_type  TEXT NOT NULL,
  aggregate_id    TEXT NOT NULL,
  event_type      TEXT NOT NULL,
  payload         TEXT,  -- JSON
  actor           TEXT DEFAULT 'user' CHECK(actor IN ('user','ai','system','mcp','skill')),
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_audit_aggregate ON audit_events(aggregate_type, aggregate_id);
CREATE INDEX IF NOT EXISTS idx_audit_type ON audit_events(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_time ON audit_events(created_at);

-- ============================================================
-- 报表/总结表（smart_summaries）
-- ============================================================
CREATE TABLE IF NOT EXISTS smart_summaries (
  id              TEXT PRIMARY KEY,
  summary_type    TEXT NOT NULL CHECK(summary_type IN ('daily','weekly','monthly','project','client')),
  entity_type     TEXT,  -- case/client/null
  entity_id       TEXT,
  title           TEXT NOT NULL,
  content         TEXT,  -- Markdown/JSON
  structured_data TEXT,  -- JSON: 结构化数据
  ai_model        TEXT,
  status          TEXT DEFAULT 'draft' CHECK(status IN ('draft','confirmed','archived')),
  period_start    TEXT,
  period_end      TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_summaries_type ON smart_summaries(summary_type);
CREATE INDEX IF NOT EXISTS idx_summaries_entity ON smart_summaries(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_summaries_period ON smart_summaries(period_start, period_end);

CREATE TRIGGER IF NOT EXISTS trg_summaries_updated
AFTER UPDATE ON smart_summaries FOR EACH ROW
BEGIN
  UPDATE smart_summaries SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- 每日统计表（daily_stats）
-- ============================================================
CREATE TABLE IF NOT EXISTS daily_stats (
  id              TEXT PRIMARY KEY,
  date            TEXT NOT NULL UNIQUE,
  task_done       INTEGER DEFAULT 0,
  task_total      INTEGER DEFAULT 0,
  overdue_count   INTEGER DEFAULT 0,
  overdue_days    INTEGER DEFAULT 0,
  hearing_count   INTEGER DEFAULT 0,
  deadline_count  INTEGER DEFAULT 0,
  waiting_overdue_3d INTEGER DEFAULT 0,
  case_transitions TEXT,  -- JSON
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_daily_stats_date ON daily_stats(date);

-- ============================================================
-- 外置记忆表（memory_entries）
-- ============================================================
CREATE TABLE IF NOT EXISTS memory_entries (
  id              TEXT PRIMARY KEY,
  layer           TEXT NOT NULL CHECK(layer IN ('l1','l2','l3')),
  content         TEXT NOT NULL,
  source_ref      TEXT,  -- JSON: 单一来源
  status          TEXT DEFAULT 'active' CHECK(status IN ('active','stale','archived')),
  confidence      REAL DEFAULT 0.5,
  ai_model        TEXT,
  last_used_at    TEXT,
  merged_from     TEXT,  -- JSON: 合并来源 ID 列表
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_memory_layer ON memory_entries(layer);
CREATE INDEX IF NOT EXISTS idx_memory_status ON memory_entries(status);
CREATE INDEX IF NOT EXISTS idx_memory_last_used ON memory_entries(last_used_at);

CREATE TRIGGER IF NOT EXISTS trg_memory_updated
AFTER UPDATE ON memory_entries FOR EACH ROW
BEGIN
  UPDATE memory_entries SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- 多来源引用表（provenance）
-- ============================================================
CREATE TABLE IF NOT EXISTS provenance (
  id              TEXT PRIMARY KEY,
  entity_type     TEXT NOT NULL,
  entity_id       TEXT NOT NULL,
  source_type     TEXT NOT NULL,
  source_id       TEXT NOT NULL,
  source_field    TEXT,
  relation        TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_provenance_entity ON provenance(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_provenance_source ON provenance(source_type, source_id);

-- ============================================================
-- 提醒作业表（reminder_jobs）
-- ============================================================
CREATE TABLE IF NOT EXISTS reminder_jobs (
  id                TEXT PRIMARY KEY,
  rule_id           TEXT REFERENCES reminder_rules(id),
  entity_type       TEXT NOT NULL CHECK(entity_type IN ('case','task','hearing','deadline')),
  entity_id         TEXT NOT NULL,
  channel           TEXT NOT NULL CHECK(channel IN ('local','system','calendar','email_ics','feishu_message','feishu_task')),
  executor          TEXT DEFAULT 'local' CHECK(executor IN ('local','calendar')),
  scheduled_at      TEXT NOT NULL,
  timezone          TEXT NOT NULL DEFAULT 'Asia/Shanghai',
  offset_snapshot   TEXT,
  calendar_account  TEXT,
  calendar_event_id TEXT,
  calendar_etag     TEXT,
  content           TEXT,
  masked_content    TEXT,
  due_snapshot      TEXT,
  status            TEXT DEFAULT 'pending' CHECK(status IN (
    'pending','synced','sent','delivered','read',
    'sync_failed','delivery_unknown','cancelled','dead_lettered'
  )),
  attempts          INTEGER DEFAULT 0,
  last_error        TEXT,
  next_attempt_at   TEXT,
  supersedes_id     TEXT,
  version           INTEGER DEFAULT 1,
  server_msg_id     TEXT,
  created_at        TEXT DEFAULT (datetime('now','localtime')),
  updated_at        TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_reminder_jobs_entity ON reminder_jobs(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_reminder_jobs_status ON reminder_jobs(status);
CREATE INDEX IF NOT EXISTS idx_reminder_jobs_scheduled ON reminder_jobs(scheduled_at);
CREATE INDEX IF NOT EXISTS idx_reminder_jobs_executor ON reminder_jobs(executor);

CREATE TRIGGER IF NOT EXISTS trg_reminder_jobs_updated
AFTER UPDATE ON reminder_jobs FOR EACH ROW
BEGIN
  UPDATE reminder_jobs SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- AI 洞察表（ai_insights）
-- ============================================================
CREATE TABLE IF NOT EXISTS ai_insights (
  id              TEXT PRIMARY KEY,
  insight_type    TEXT NOT NULL CHECK(insight_type IN ('pattern','recommendation','warning','correlation')),
  entity_type     TEXT,
  entity_id       TEXT,
  title           TEXT NOT NULL,
  content         TEXT NOT NULL,
  confidence      REAL DEFAULT 0.5,
  source_ref      TEXT,  -- JSON
  status          TEXT DEFAULT 'pending' CHECK(status IN ('pending','confirmed','rejected','archived')),
  ai_model        TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_insights_type ON ai_insights(insight_type);
CREATE INDEX IF NOT EXISTS idx_insights_entity ON ai_insights(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_insights_status ON ai_insights(status);

CREATE TRIGGER IF NOT EXISTS trg_insights_updated
AFTER UPDATE ON ai_insights FOR EACH ROW
BEGIN
  UPDATE ai_insights SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;

-- ============================================================
-- 更新 clients 表：添加别名归一支持
-- ============================================================
ALTER TABLE clients ADD COLUMN aliases TEXT;  -- JSON: 别名列表
ALTER TABLE clients ADD COLUMN normalized_name TEXT;
CREATE INDEX IF NOT EXISTS idx_clients_normalized ON clients(normalized_name);

-- ============================================================
-- 更新 cases 表：添加 deadline 字段别名
-- ============================================================
-- due_date 是 deadline 的别名，保持兼容
ALTER TABLE cases ADD COLUMN due_date TEXT;

-- ============================================================
-- 双路径路由表（Rule/AI 路径标记）
-- ============================================================
CREATE TABLE IF NOT EXISTS command_routes (
  command_name    TEXT PRIMARY KEY,
  route_type      TEXT NOT NULL CHECK(route_type IN ('rule','ai','hybrid')),
  description     TEXT,
  requires_confirmation INTEGER DEFAULT 0,
  min_confirm_level TEXT DEFAULT 'L1' CHECK(min_confirm_level IN ('L1','L2','L3')),
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);

-- 种子数据：核心命令路由
INSERT OR IGNORE INTO command_routes (command_name, route_type, description, requires_confirmation, min_confirm_level) VALUES
  ('create_case', 'rule', '创建案件', 0, 'L1'),
  ('update_case', 'rule', '更新案件', 0, 'L1'),
  ('delete_case', 'rule', '删除案件', 1, 'L3'),
  ('create_task', 'rule', '创建任务', 0, 'L1'),
  ('update_task', 'rule', '更新任务', 0, 'L1'),
  ('toggle_task', 'rule', '完成任务', 0, 'L1'),
  ('delete_task', 'rule', '删除任务', 1, 'L3'),
  ('process_inbox_item', 'ai', '处理收件箱', 1, 'L2'),
  ('generate_writing_suggestion', 'ai', '生成写作建议', 1, 'L2'),
  ('classify_document_with_prompt', 'ai', 'AI 文档分类', 1, 'L2'),
  ('extract_info_with_prompt', 'ai', 'AI 信息提取', 1, 'L2');
"#;

/// 版本 10: Saved Filters + memory_entries status 扩展（蒸馏确认区）
pub const MIGRATION_V10_SQL: &str = r#"
-- ============================================================
-- Saved Filters（设计哲学 §9：筛选/排序/分组规则可保存复用）
-- ============================================================
CREATE TABLE IF NOT EXISTS saved_filters (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  entity_type TEXT NOT NULL,
  name        TEXT NOT NULL,
  filter_json TEXT NOT NULL,
  sort_order  INTEGER DEFAULT 0,
  created_at  TEXT DEFAULT (datetime('now','localtime')),
  updated_at  TEXT DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_saved_filters_entity ON saved_filters(entity_type);

-- ============================================================
-- memory_entries 重建：扩展 status CHECK
-- 新增 pending（待确认）/ merged（已合并）/ dismissed（已丢弃），供数据蒸馏确认区使用
-- SQLite 不支持 ALTER CHECK，按 inbox_items v8 模式重建表
-- ============================================================
DROP TABLE IF EXISTS memory_entries_v10;

CREATE TABLE memory_entries_v10 (
  id              TEXT PRIMARY KEY,
  layer           TEXT NOT NULL CHECK(layer IN ('l1','l2','l3')),
  content         TEXT NOT NULL,
  source_ref      TEXT,  -- JSON: 单一来源
  status          TEXT DEFAULT 'active'
                  CHECK(status IN ('active','stale','archived','pending','merged','dismissed')),
  confidence      REAL DEFAULT 0.5,
  ai_model        TEXT,
  last_used_at    TEXT,
  merged_from     TEXT,  -- JSON: 合并来源 ID 列表
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime'))
);

INSERT INTO memory_entries_v10 (
  id, layer, content, source_ref, status, confidence, ai_model,
  last_used_at, merged_from, created_at, updated_at
)
SELECT
  id, layer, content, source_ref, status, confidence, ai_model,
  last_used_at, merged_from, created_at, updated_at
FROM memory_entries;

DROP TABLE memory_entries;
ALTER TABLE memory_entries_v10 RENAME TO memory_entries;

CREATE INDEX IF NOT EXISTS idx_memory_layer ON memory_entries(layer);
CREATE INDEX IF NOT EXISTS idx_memory_status ON memory_entries(status);
CREATE INDEX IF NOT EXISTS idx_memory_last_used ON memory_entries(last_used_at);

CREATE TRIGGER IF NOT EXISTS trg_memory_updated
AFTER UPDATE ON memory_entries FOR EACH ROW
BEGIN
  UPDATE memory_entries SET updated_at = datetime('now','localtime') WHERE id = NEW.id;
END;
"#;

/// 版本 11: 知识块级化 + 报表叙事层 + L3 递归确认
///
/// 实际变更全部在 run_migrations 的条件执行段完成（PRAGMA 探测后按需 ALTER/重建），
/// 保证迁移幂等；此常量仅作版本占位，用于把 user_version 推进到 11。
///
/// 内容：
/// - knowledge_items 加 parent_id（自引用）/ block_type（'page'/'block'/'reference'）+ parent_id 索引（§8.2）
/// - smart_summaries 加 narrative_source（'rule'/'ai'，叙事层来源标记，§11.3）
/// - task_events 重建：event_type CHECK 新增 'recursion_gap'，task_id 改为可空（§11.5）
pub const MIGRATION_V11_SQL: &str = r#"
-- v11 实际变更见 run_migrations 条件执行段（幂等）
SELECT 1;
"#;

/// 版本 13: 任务具体时间点（设计哲学 §7 时间分配：due_time HH:MM）
/// 幂等说明：ADD COLUMN 在 run_migrations 的条件补列段执行（PRAGMA 探测），
/// 此处只推进版本号，避免重复迁移时 duplicate column。
pub const MIGRATION_V13_SQL: &str = r#"
-- tasks.due_time 由条件补列段添加（幂等）
"#;

/// 版本 12: MCP 写操作待确认队列（设计哲学 §11.11 安全约束）
///
/// 外部 AI 经 MCP 通道发起的写操作不直接执行，先落队等待应用内确认；
/// 确认/拒绝留痕 audit_events（actor='mcp'）。
pub const MIGRATION_V12_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS mcp_pending_writes (
  id          TEXT PRIMARY KEY,
  tool        TEXT NOT NULL,
  arguments   TEXT NOT NULL,  -- JSON
  status      TEXT NOT NULL DEFAULT 'pending'
              CHECK(status IN ('pending','approved','rejected','executed','failed')),
  result      TEXT,
  created_at  TEXT DEFAULT (datetime('now','localtime')),
  resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_mcp_pending_writes_status ON mcp_pending_writes(status);
CREATE INDEX IF NOT EXISTS idx_mcp_pending_writes_created ON mcp_pending_writes(created_at);
"#;

/// task_events 重建 SQL（v11）：event_type CHECK 扩展 recursion_gap，task_id 可空
/// 仅在旧表 CHECK 不含 recursion_gap 时由 run_migrations 执行
const TASK_EVENTS_REBUILD_V11_SQL: &str = r#"
DROP TABLE IF EXISTS task_events_v11;

CREATE TABLE task_events_v11 (
  id              TEXT PRIMARY KEY,
  task_id         TEXT REFERENCES tasks(id) ON DELETE CASCADE,
  event_type      TEXT NOT NULL CHECK(event_type IN (
    'created','completed','deferred','snoozed','reminded',
    'overdue','escalated','cancelled','moved','recursion_gap'
  )),
  occurred_at     TEXT NOT NULL DEFAULT (datetime('now','localtime')),
  payload         TEXT,  -- JSON
  actor           TEXT DEFAULT 'user' CHECK(actor IN ('user','ai','system'))
);

INSERT INTO task_events_v11 (id, task_id, event_type, occurred_at, payload, actor)
SELECT id, task_id, event_type, occurred_at, payload, actor FROM task_events;

DROP TABLE task_events;
ALTER TABLE task_events_v11 RENAME TO task_events;

CREATE INDEX IF NOT EXISTS idx_task_events_task ON task_events(task_id);
CREATE INDEX IF NOT EXISTS idx_task_events_type ON task_events(event_type);
CREATE INDEX IF NOT EXISTS idx_task_events_time ON task_events(occurred_at);
"#;

/// 执行迁移：从 from_version 之后的版本逐条应用
#[allow(dead_code)]
pub fn run_migrations(conn: &Connection, from_version: i64) -> Result<(), anyhow::Error> {
    for (version, sql) in MIGRATIONS {
        let v: i64 = version.parse().unwrap_or(0);
        if v > from_version {
            conn.execute_batch(sql)?;
            conn.execute_batch(&format!("PRAGMA user_version = {};", v))?;
            log::info!("Migration v{} applied", v);
        }
    }

    // 条件补列 + 索引：knowledge_items.law_name（旧 DB 可能缺少该列）
    let has_law_name: bool = conn
        .prepare("PRAGMA table_info(knowledge_items)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|col| col == "law_name");
    if !has_law_name {
        conn.execute_batch("ALTER TABLE knowledge_items ADD COLUMN law_name TEXT;")?;
        conn.execute_batch("ALTER TABLE knowledge_items ADD COLUMN article_no TEXT;")?;
        conn.execute_batch("ALTER TABLE knowledge_items ADD COLUMN effective_date TEXT;")?;
        conn.execute_batch("ALTER TABLE knowledge_items ADD COLUMN status TEXT DEFAULT 'current';")?;
        log::info!("Added law_name/article_no/effective_date/status columns to knowledge_items");
    }
    conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_knowledge_law ON knowledge_items(law_name);")?;

    // 条件补列：reminder_log.level（R1-R4 分级，旧 DB 可能缺少该列）
    let has_reminder_level: bool = conn
        .prepare("PRAGMA table_info(reminder_log)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|col| col == "level");
    if !has_reminder_level {
        conn.execute_batch(
            "ALTER TABLE reminder_log ADD COLUMN level TEXT CHECK(level IN ('R1','R2','R3','R4'));",
        )?;
        log::info!("Added level column to reminder_log (R1-R4 classification)");
    }

    // 条件补列：ai_runs / ai_context_items 等表的索引（旧 DB 可能缺少）
    conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_ai_runs_created ON ai_runs(created_at);")?;

    // ── v11 条件执行段（幂等：PRAGMA 探测后按需变更）────────────────

    // 知识块级化（§8.2）：knowledge_items.parent_id / block_type
    let ki_cols: Vec<String> = conn
        .prepare("PRAGMA table_info(knowledge_items)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();
    if !ki_cols.iter().any(|c| c == "parent_id") {
        conn.execute_batch("ALTER TABLE knowledge_items ADD COLUMN parent_id TEXT;")?;
        log::info!("Added parent_id column to knowledge_items (block hierarchy)");
    }
    if !ki_cols.iter().any(|c| c == "block_type") {
        conn.execute_batch("ALTER TABLE knowledge_items ADD COLUMN block_type TEXT DEFAULT 'page';")?;
        log::info!("Added block_type column to knowledge_items (page/block/reference)");
    }
    conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_knowledge_parent ON knowledge_items(parent_id);")?;

    // 报表叙事层（§11.3）：smart_summaries.narrative_source（'rule'/'ai'）
    let ss_cols: Vec<String> = conn
        .prepare("PRAGMA table_info(smart_summaries)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();
    if !ss_cols.iter().any(|c| c == "narrative_source") {
        conn.execute_batch("ALTER TABLE smart_summaries ADD COLUMN narrative_source TEXT DEFAULT 'rule';")?;
        log::info!("Added narrative_source column to smart_summaries (rule/ai)");
    }

    // v13：tasks.due_time（具体时间点，旧库保护）
    let task_cols: Vec<String> = conn
        .prepare("PRAGMA table_info(tasks)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();
    if !task_cols.iter().any(|c| c == "due_time") {
        conn.execute_batch("ALTER TABLE tasks ADD COLUMN due_time TEXT;")?;
        conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_tasks_due_time ON tasks(due_time);")?;
        log::info!("Added due_time column to tasks (v13 time model)");
    }

    // L3 递归确认（§11.5）：task_events.event_type CHECK 扩展 'recursion_gap'，task_id 可空
    let te_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'task_events'",
            [],
            |r| r.get(0),
        )
        .ok();
    if let Some(sql) = te_sql {
        if !sql.contains("recursion_gap") {
            conn.execute_batch(TASK_EVENTS_REBUILD_V11_SQL)?;
            log::info!("Rebuilt task_events with recursion_gap event type (v11)");
        }
    }

    Ok(())
}

/// 种子数据：法定期限规则
pub fn seed_deadline_rules(conn: &Connection) -> Result<(), anyhow::Error> {
    let rules: Vec<(&str, &str, &str, &str, &str, i64, &str, &str, &str, i32, Option<&str>)> = vec![
        // ── 专利无效（专利法实施细则算法）──
        ("rule-pi-001", "patent_invalidation", "专利权人陈述意见期限",
         "专利法实施细则第58条", "patentee_received_date", 1, "calendar_month", "patent", "statutory", 10, None),
        ("rule-pi-002", "patent_invalidation", "请求人答复意见期限",
         "专利法实施细则第58条", "petitioner_received_date", 1, "calendar_month", "patent", "statutory", 10, None),
        ("rule-pi-003", "patent_invalidation", "专利权人补充意见期限",
         "专利法实施细则第58条", "patentee_received_supp_date", 1, "calendar_month", "patent", "statutory", 10, None),
        ("rule-pi-004", "patent_invalidation", "请求人补充意见期限",
         "专利法实施细则第58条", "petitioner_submit_date", 1, "calendar_month", "patent", "statutory", 10, None),
        ("rule-pi-005", "patent_invalidation", "预估审限（无效）",
         "专利审查指南第4部分第3章", "filing_date", 5, "calendar_month", "patent", "recommended", 0, None),
        // ── 行政诉讼（诉讼法算法）──
        ("rule-al-001", "admin_litigation", "提交答辩状期间",
         "行政诉讼法第67条", "complaint_received_date", 15, "day", "civil", "statutory", 10, None),
        ("rule-al-002", "admin_litigation", "预估审限（简易）",
         "行政诉讼法第84条", "filing_date", 3, "calendar_month", "civil", "recommended", 5, Some("简易")),
        ("rule-al-003", "admin_litigation", "预估审限（普通）",
         "行政诉讼法第81条", "filing_date", 6, "calendar_month", "civil", "recommended", 5, Some("普通")),
        ("rule-al-004", "admin_litigation", "判决上诉期",
         "行政诉讼法第85条", "verdict_date", 15, "day", "civil", "statutory", 10, Some(r#"{"verdict_type":"判决"}"#)),
        ("rule-al-005", "admin_litigation", "裁定上诉期",
         "行政诉讼法第85条", "verdict_date", 10, "day", "civil", "statutory", 10, Some(r#"{"verdict_type":"裁定"}"#)),
        // ── 民事侵权（诉讼法算法）──
        ("rule-ct-001", "civil_tort", "提交答辩状期间",
         "民事诉讼法第128条", "complaint_received_date", 15, "day", "civil", "statutory", 10, None),
        ("rule-ct-002", "civil_tort", "预估审限（简易）",
         "民事诉讼法第164条", "filing_date", 3, "calendar_month", "civil", "recommended", 5, Some("简易")),
        ("rule-ct-003", "civil_tort", "预估审限（普通）",
         "民事诉讼法第152条", "filing_date", 6, "calendar_month", "civil", "recommended", 5, Some("普通")),
        ("rule-ct-004", "civil_tort", "判决上诉期",
         "民事诉讼法第171条", "verdict_date", 15, "day", "civil", "statutory", 10, Some(r#"{"verdict_type":"判决"}"#)),
        ("rule-ct-005", "civil_tort", "裁定上诉期",
         "民事诉讼法第171条", "verdict_date", 10, "day", "civil", "statutory", 10, Some(r#"{"verdict_type":"裁定"}"#)),
    ];

    for (id, track, name, basis, field, offset, unit, calc, source, priority, proc_type) in rules {
        conn.execute(
            "INSERT INTO deadline_rules (id, track, rule_name, legal_basis, trigger_field, offset_value, offset_unit, calc_method, deadline_source, auto_calculate, priority, procedure_types)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?11)",
            params![id, track, name, basis, field, offset, unit, calc, source, priority,
                proc_type.map(|p| if p.starts_with('{') { p.to_string() } else { format!(r#"["{}"]"#, p) })],
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// v11 迁移幂等性：从 v1 全量迁移两次，结果一致且新列/新约束就位
    #[test]
    fn test_v11_migration_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();

        run_migrations(&conn, 1).unwrap();
        // v11 条件段幂等：从 v11 边界重复执行，条件段每次都会跑，必须安全
        run_migrations(&conn, 11).unwrap();
        run_migrations(&conn, 11).unwrap();

        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        let ki_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(knowledge_items)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(ki_cols.iter().any(|c| c == "parent_id"));
        assert!(ki_cols.iter().any(|c| c == "block_type"));

        let ss_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(smart_summaries)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(ss_cols.iter().any(|c| c == "narrative_source"));

        // task_events：recursion_gap 可写、task_id 可空
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, payload, actor)
             VALUES ('te-v11', NULL, 'recursion_gap', '{}', 'system')",
            [],
        )
        .unwrap();
    }

    /// v12 迁移幂等性：mcp_pending_writes 表存在且可重复迁移
    #[test]
    fn test_v12_migration_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();

        run_migrations(&conn, 1).unwrap();
        run_migrations(&conn, 12).unwrap();
        run_migrations(&conn, 12).unwrap();

        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        // 表结构：status CHECK 生效，默认 pending
        conn.execute(
            "INSERT INTO mcp_pending_writes (id, tool, arguments)
             VALUES ('w1', 'case_create_task', '{}')",
            [],
        )
        .unwrap();
        let status: String = conn
            .query_row("SELECT status FROM mcp_pending_writes WHERE id='w1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "pending");

        let bad = conn.execute(
            "INSERT INTO mcp_pending_writes (id, tool, arguments, status)
             VALUES ('w2', 'case_create_task', '{}', 'bogus')",
            [],
        );
        assert!(bad.is_err(), "非法 status 应被 CHECK 拒绝");
    }
}

