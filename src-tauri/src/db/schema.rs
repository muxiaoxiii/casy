use rusqlite::{params, Connection};

/// 当前 Schema 版本号
#[allow(dead_code)]
pub const CURRENT_SCHEMA_VERSION: i64 = 2;

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
CREATE INDEX IF NOT EXISTS idx_knowledge_law ON knowledge_items(law_name);

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
