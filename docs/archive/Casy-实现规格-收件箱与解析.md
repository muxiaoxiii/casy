# Casy 实现规格 — 收件箱 + 文档解析 + 文件管理 + 邮件

## 1. 文本提取

### 1.1 PDF 文本提取

```toml
# Cargo.toml
pdf-extract = "0.7"
```

```rust
fn extract_pdf_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let text = pdf_extract::extract_text_from_mem(&bytes)?;
    Ok(text)
}
```

### 1.2 PDF OCR（扫描件）

```toml
# Cargo.toml
rusty-tesseract = "1.1"  # 调用系统 tesseract CLI，无需 FFI 链接
```

```rust
fn ocr_image(image_path: &Path, lang: &str) -> Result<String> {
    let image = rusty_tesseract::Image::from_path(image_path)?;
    let args = rusty_tesseract::Args {
        lang: lang.to_string(),
        ..Default::default()
    };
    let output = rusty_tesseract::tesseract(&image, &args)?;
    Ok(output)
}
```

需要系统安装 Tesseract + 中文语言包：
```bash
# macOS
brew install tesseract tesseract-lang

# Ubuntu
sudo apt install tesseract-ocr tesseract-ocr-chi-sim
```

### 1.3 DOCX 提取

DOCX 本质是 zip 包，内含 `word/document.xml`。用 `zip` crate 解压 + `quick-xml` 提取文本：

```rust
use zip::ZipArchive;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::io::Read;

fn extract_docx_text(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    // 读取 word/document.xml
    let mut doc_xml = archive.by_name("word/document.xml")?;
    let mut xml_content = String::new();
    doc_xml.read_to_string(&mut xml_content)?;

    // 提取所有 <w:t> 节点的文本
    let mut reader = Reader::from_str(&xml_content);
    let mut text = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                if e.name().as_ref() == b"w:t" {
                    // 下一个 Text 事件就是内容
                }
            }
            Ok(Event::Text(ref e)) => {
                let t = e.unescape().unwrap_or_default();
                text.push_str(&t);
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"w:p" {
                    text.push('\n'); // 段落换行
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(text)
}
```

**注意**：calamine 只能读 Excel（.xlsx），不能读 Word（.docx）。DOCX 文本提取必须用 zip + XML 解析。

### 1.4 .eml 解析

```toml
# Cargo.toml
mailparse = "0.15"
```

```rust
fn parse_eml(path: &Path) -> Result<ParsedEmail> {
    let content = std::fs::read_to_string(path)?;
    let parsed = mailparse::parse_mail(content.as_bytes())?;

    Ok(ParsedEmail {
        message_id: parsed.headers.iter()
            .find(|h| h.name == "Message-ID")
            .map(|h| h.value.clone()),
        subject: parsed.headers.iter()
            .find(|h| h.name == "Subject")
            .map(|h| h.value.clone()).unwrap_or_default(),
        from: parsed.headers.iter()
            .find(|h| h.name == "From")
            .map(|h| h.value.clone()).unwrap_or_default(),
        to: parsed.headers.iter()
            .find(|h| h.name == "To")
            .map(|h| h.value.clone()),
        cc: parsed.headers.iter()
            .find(|h| h.name == "Cc")
            .map(|h| h.value.clone()),
        date: parsed.headers.iter()
            .find(|h| h.name == "Date")
            .map(|h| h.value.clone()).unwrap_or_default(),
        body_text: extract_body(&parsed, "text/plain"),
        body_html: extract_body(&parsed, "text/html"),
        attachments: extract_attachments(&parsed),
    })
}
```

---

## 2. 文档解析

### 2.1 传票正则

```rust
use regex::Regex;

lazy_static::lazy_static! {
    // 案号
    static ref CASE_NO_RE: Regex = Regex::new(
        r"[（(]\s*(\d{4})\s*[）)]\s*([一-龥]{2,8})\s*(\d+)\s*号"
    ).unwrap();

    // 日期时间（开庭时间）
    static ref DATETIME_RE: Regex = Regex::new(
        r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日\s*(?:上午|下午)?\s*(\d{1,2})\s*时\s*(\d{1,2})\s*分?"
    ).unwrap();

    // 日期（签发日期）
    static ref DATE_RE: Regex = Regex::new(
        r"(\d{4})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})\s*日"
    ).unwrap();

    // 法院名称
    static ref COURT_RE: Regex = Regex::new(
        r"([一-龥]+(?:人民法院|知识产权法院|仲裁委员会))"
    ).unwrap();

    // 审判长/法官
    static ref JUDGE_RE: Regex = Regex::new(
        r"(?:审判长|审判员|法官)\s*[：:]\s*([一-龥]{2,4})"
    ).unwrap();

    // 书记员
    static ref CLERK_RE: Regex = Regex::new(
        r"书记员\s*[：:]\s*([一-龥]{2,4})"
    ).unwrap();
}

pub struct SummonsInfo {
    pub case_no: Option<String>,
    pub summoned_party: Option<String>,
    pub reason: Option<String>,
    pub hearing_date: Option<String>,
    pub venue: Option<String>,
    pub judge: Option<String>,
    pub clerk: Option<String>,
    pub court: Option<String>,
    pub issue_date: Option<String>,
}

pub fn parse_summons(text: &str) -> SummonsInfo {
    SummonsInfo {
        case_no: CASE_NO_RE.captures(text).map(|c| c[0].to_string()),
        hearing_date: DATETIME_RE.captures(text).map(|c| {
            format!("{}-{:02}-{:02} {:02}:{:02}", &c[1], c[2].parse::<u32>().unwrap_or(1), c[3].parse::<u32>().unwrap_or(1), c[4].parse::<u32>().unwrap_or(0), c[5].parse::<u32>().unwrap_or(0))
        }),
        court: COURT_RE.captures(text).map(|c| c[1].to_string()),
        judge: JUDGE_RE.captures(text).map(|c| c[1].to_string()),
        clerk: CLERK_RE.captures(text).map(|c| c[1].to_string()),
        issue_date: DATE_RE.captures(text).map(|c| format!("{}-{:02}-{:02}", &c[1], c[2].parse::<u32>().unwrap_or(1), c[3].parse::<u32>().unwrap_or(1))),
        ..Default::default()
    }
}
```

### 2.2 口审通知书正则

```rust
lazy_static::lazy_static! {
    // 案件编号（国知局格式: 4W123456）
    static ref CNIPA_NO_RE: Regex = Regex::new(r"(\d+W\d+)").unwrap();

    // 专利号
    static ref PATENT_NO_RE: Regex = Regex::new(r"(?:专利号|ZL)\s*[：:]?\s*(\d{9,13}\.?\d?)").unwrap();

    // 请求人/专利权人
    static ref PARTY_RE: Regex = Regex::new(
        r"(?:请求人|专利权人)\s*[：:]\s*([一-龥\w\(\)（）]+)"
    ).unwrap();

    // 合议组
    static ref PANEL_RE: Regex = Regex::new(
        r"(?:合议组组长|组长)\s*[：:]\s*([一-龥]{2,4})"
    ).unwrap();
}

pub struct HearingNoticeInfo {
    pub case_number: Option<String>,
    pub patent_no: Option<String>,
    pub patent_name: Option<String>,
    pub petitioner: Option<String>,
    pub patentee: Option<String>,
    pub hearing_date: Option<String>,
    pub venue: Option<String>,
    pub panel_chair: Option<String>,
    pub panel_members: Vec<String>,
}
```

### 2.3 AI 增强提取

```rust
pub async fn ai_extract(text: &str, doc_type: &str, ai: &AiBackend) -> Result<serde_json::Value> {
    let prompt = format!(
        "你是法律文档信息提取助手。请从以下{doc_type}文本中提取结构化信息。\n\
         返回 JSON 格式，字段如下：\n\
         - case_no: 案号\n\
         - parties: [{{name, role}}] 当事人列表\n\
         - date: 日期\n\
         - court: 法院/机关\n\
         - judge: 法官/审查员\n\
         - clerk: 书记员\n\
         - patent_no: 专利号（如有）\n\
         - patent_name: 专利名称（如有）\n\
         - hearing_date: 开庭/口审时间（如有）\n\
         - venue: 地点（如有）\n\n\
         文档内容：\n{text}"
    );
    ai.call_json(&prompt).await
}
```

### 2.4 案件匹配

```rust
pub fn match_case(conn: &Connection, info: &ExtractedInfo) -> Result<Vec<CaseMatch>> {
    let mut matches = Vec::new();

    // 1. 精确匹配案号
    if let Some(case_no) = &info.case_no {
        if let Ok(case) = query_case_by_no(conn, case_no) {
            matches.push(CaseMatch { case, score: 100, reason: "案号精确匹配".into() });
            return Ok(matches);
        }
    }

    // 2. 专利号匹配
    if let Some(patent_no) = &info.patent_no {
        let cases = query_cases_by_patent(conn, patent_no)?;
        for case in cases {
            matches.push(CaseMatch { case, score: 90, reason: "专利号匹配".into() });
        }
    }

    // 3. 当事人名称模糊匹配
    for party in &info.parties {
        let cases = query_cases_by_party_name(conn, &party.name)?;
        for case in cases {
            let score = if party.name == case.client_name { 70 } else { 50 };
            matches.push(CaseMatch { case, score, reason: format!("当事人匹配: {}", party.name) });
        }
    }

    // 4. 去重排序
    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches.dedup_by(|a, b| a.case.id == b.case.id);
    matches.truncate(5);
    Ok(matches)
}
```

---

## 3. 收件箱处理管道

```rust
pub struct InboxProcessor {
    conn: Connection,
    ai: AiBackend,
    calendar: HolidayCalendar,
}

impl InboxProcessor {
    /// 处理一个收件项
    pub async fn process(&self, item: &mut InboxItem) -> Result<()> {
        // 1. 提取文本
        let text = match item.source_type.as_str() {
            "file" => extract_file_text(Path::new(item.source_path.as_ref().unwrap()))?,
            "email" | "imap" => item.content_text.clone().unwrap_or_default(),
            "note" | "paste" => item.content_text.clone().unwrap_or_default(),
            _ => return Ok(()),
        };
        item.content_text = Some(text.clone());

        // 2. AI 分类
        if self.ai.is_available() {
            let category = self.ai.classify_document(&text).await?;
            item.ai_category = Some(category.doc_type);
            item.ai_confidence = Some(category.confidence);
            item.ai_extracted = Some(serde_json::to_string(&category.extracted)?);

            // 3. 案件匹配
            if let Some(extracted) = &category.extracted {
                let matches = match_case(&self.conn, extracted)?;
                if let Some(best) = matches.first() {
                    item.ai_suggested_case_id = Some(best.case.id.clone());
                }
            }
        } else {
            // 本地模式：规则匹配
            item.ai_category = Some(rule_based_classify(&text));
        }

        // 4. 更新状态
        item.status = "pending".to_string();
        Ok(())
    }

    /// 归档到案件（原子操作：文件拷贝 + 数据库事务）
    pub fn file_to_case(&self, item_id: &str, case_id: &str, category: &str) -> Result<()> {
        let item = get_inbox_item(&self.conn, item_id)?;
        let case = get_case(&self.conn, case_id)?;

        // 1. 准备文件路径（在事务外完成）
        let case_folder = ensure_case_folder(&case)?;
        let sub_folder = match category {
            "summons" => "传票",
            "evidence" => "证据",
            "submitted" => "交文",
            "received" => "收文",
            "internal" => "内部",
            "correspondence" => "通信",
            _ => "其他",
        };
        let target_dir = case_folder.join(sub_folder);
        std::fs::create_dir_all(&target_dir)?;
        let new_name = smart_rename(
            &item.source_path.as_deref().unwrap_or("file"),
            &case,
            category,
            None,
        );
        let target_path = target_dir.join(&new_name);
        let file_path_rel = format!("{}/{}", sub_folder, new_name);

        // 2. 拷贝文件（在事务外，避免长时间持锁）
        if let Some(src) = &item.source_path {
            std::fs::copy(src, &target_path)?;
        }

        // 3. 数据库事务：记录文件 + 日志 + 更新收件箱状态
        let tx = self.conn.transaction()?;
        let file_id = new_id();
        let log_id = new_id();

        tx.execute(
            "INSERT INTO case_files (id, case_id, file_name, file_path, category, source_inbox_id, source_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'inbox')",
            rusqlite::params![file_id, case_id, new_name, file_path_rel, category, item_id],
        )?;

        tx.execute(
            "INSERT INTO case_logs (id, case_id, event_summary, event_type, event_date)
             VALUES (?1, ?2, ?3, 'record', ?4)",
            rusqlite::params![log_id, case_id, format!("归档文件: {}", item.title.as_deref().unwrap_or("")), today()],
        )?;

        tx.execute(
            "UPDATE inbox_items SET status = 'filed', linked_case_id = ?1, filed_to = ?2, filed_as = ?3, processed_at = datetime('now','localtime')
             WHERE id = ?4",
            rusqlite::params![case_id, file_path_rel, category, item_id],
        )?;

        tx.commit()?;

        Ok(())
    }
}
```

---

## 4. IMAP 邮件监听

### 4.1 依赖

```toml
async-imap = { version = "0.11", features = ["tokio"] }
async-native-tls = "0.5"
```

### 4.2 ImapWatcher

```rust
pub struct ImapWatcher {
    accounts: Vec<ImapAccount>,
    processor: Arc<InboxProcessor>,
    cancel: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct ImapAccount {
    pub id: String,
    pub email: String,
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,  // 从 OS keychain 读取
    pub watch_folders: Vec<String>,
    pub filter_from: Vec<String>,
    pub filter_subject: Vec<String>,
    pub last_uid: u32,
}

impl ImapWatcher {
    /// 启动后台监听（每个账号一个 tokio task）
    pub fn start(&self, app: tauri::AppHandle) {
        for account in &self.accounts {
            let account = account.clone();
            let processor = self.processor.clone();
            let cancel = self.cancel.clone();
            let app = app.clone();

            tokio::spawn(async move {
                watch_account(account, processor, cancel, app).await;
            });
        }
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// 使用 async-imap 的 IDLE 模式监听新邮件
async fn watch_account(
    account: ImapAccount,
    processor: Arc<InboxProcessor>,
    cancel: Arc<AtomicBool>,
    app: tauri::AppHandle,
) {
    loop {
        if cancel.load(Ordering::SeqCst) { break; }

        match connect_and_idle(&account, &processor, &app).await {
            Ok(_) => {
                // IDLE 正常结束（超时），重新连接
                log::info!("IMAP IDLE timeout, reconnecting...");
            }
            Err(e) => {
                log::error!("IMAP error: {}, reconnecting in 30s...", e);
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    }
}

async fn connect_and_idle(
    account: &ImapAccount,
    processor: &Arc<InboxProcessor>,
    app: &tauri::AppHandle,
) -> Result<()> {
    // 连接
    let tls = async_native_tls::TlsConnector::new();
    let client = async_imap::connect(
        (account.server.as_str(), account.port),
        &account.server,
        tls,
    ).await?;

    let mut session = client.login(&account.username, &account.password).await
        .map_err(|(e, _)| anyhow::anyhow!("IMAP login failed: {}", e))?;

    // 选择文件夹
    let folder = account.watch_folders.first()
        .map(|s| s.as_str())
        .unwrap_or("INBOX");
    session.select(folder).await?;

    // 检查新邮件
    let new_uids = session.uid_search(format!("UID {}:*", account.last_uid)).await?;
    for uid in &new_uids {
        if *uid <= account.last_uid { continue; }

        let messages = session.uid_fetch(uid.to_string(), "RFC822").await?;
        for message in messages {
            if let Some(body) = message.body() {
                let email = mailparse::parse_mail(body)?;

                if !should_process(&email, account) { continue; }

                let item = InboxItem {
                    id: new_id(),
                    source_type: "imap".to_string(),
                    title: Some(email.headers.get_first_value("Subject").unwrap_or_default()),
                    content_text: extract_body_text(&email),
                    content_html: extract_body_html(&email),
                    source_time: email.headers.get_first_value("Date"),
                    ..Default::default()
                };

                let mut item = item;
                let _ = processor.process(&mut item).await;

                // 通知前端
                let _ = app.emit("inbox-new-item", &item.id);
            }
        }
    }

    // 更新 last_uid（持久化到 imap_accounts 表）
    if let Some(max_uid) = new_uids.iter().max() {
        if *max_uid > account.last_uid {
            update_last_uid(&account.id, *max_uid).await?;
        }
    }

    // 进入 IDLE 模式（等待新邮件推送）
    let mut idle = session.idle();
    idle.init().await?;
    let result = idle.wait_with_timeout(Duration::from_secs(29 * 60)).await;

    // DONE 退出 IDLE
    session = idle.done().await?;

    Ok(())
}

/// 持久化 last_uid 到数据库
async fn update_last_uid(account_id: &str, uid: u32) -> Result<()> {
    // 注意：这里需要异步数据库访问，实际实现中使用 tokio::task::spawn_blocking
    let conn = crate::db::open_db()?;
    conn.execute(
        "UPDATE imap_accounts SET last_uid = ?1 WHERE id = ?2",
        rusqlite::params![uid, account_id],
    )?;
    Ok(())
}

/// UIDVALIDITY 检查（RFC 3501）
/// 如果 UIDVALIDITY 变化，所有缓存的 UID 都失效，需要重新同步
async fn check_uid_validity(session: &mut async_imap::Session, account: &ImapAccount) -> Result<bool> {
    let status = session.status("INBOX", "(UIDVALIDITY)").await?;
    let current_validity = status.uid_validity.unwrap_or(0);
    let stored_validity = get_stored_uid_validity(&account.id)?;

    if stored_validity != 0 && current_validity != stored_validity {
        // UIDVALIDITY 变化，重置 last_uid
        update_last_uid(&account.id, 0).await?;
        update_uid_validity(&account.id, current_validity).await?;
        return Ok(true);  // 需要重新同步
    }

    if stored_validity == 0 {
        update_uid_validity(&account.id, current_validity).await?;
    }

    Ok(false)
}

fn should_process(email: &ParsedEmail, account: &ImapAccount) -> bool {
    // 发件人白名单
    if !account.filter_from.is_empty() {
        let from = email.from.to_lowercase();
        if !account.filter_from.iter().any(|f| from.contains(&f.to_lowercase())) {
            return false;
        }
    }
    // 主题关键词
    if !account.filter_subject.is_empty() {
        let subject = email.subject.to_lowercase();
        if !account.filter_subject.iter().any(|k| subject.contains(&k.to_lowercase())) {
            return false;
        }
    }
    true
}
```

### 4.3 Token 保护

```rust
pub struct TokenBudget {
    daily_limit: u32,
    used_today: AtomicU32,
    last_reset: Mutex<NaiveDate>,
}

impl TokenBudget {
    pub fn new(daily_limit: u32) -> Self;

    /// 检查是否还有预算
    pub fn can_spend(&self) -> bool {
        self.maybe_reset();
        self.used_today.load(Ordering::SeqCst) < self.daily_limit
    }

    /// 消耗一次
    pub fn spend(&self) {
        self.used_today.fetch_add(1, Ordering::SeqCst);
    }

    /// 今日剩余
    pub fn remaining(&self) -> u32 {
        self.maybe_reset();
        self.daily_limit.saturating_sub(self.used_today.load(Ordering::SeqCst))
    }

    fn maybe_reset(&self) {
        let today = Local::now().naive_local().date();
        let mut last = self.last_reset.lock().unwrap();
        if *last < today {
            self.used_today.store(0, Ordering::SeqCst);
            *last = today;
        }
    }
}

/// 在 AI 调用前检查预算
pub async fn call_ai_with_budget(
    ai: &AiBackend,
    prompt: &str,
    budget: &TokenBudget,
) -> Result<Option<String>> {
    if !budget.can_spend() {
        log::warn!("AI budget exhausted, skipping");
        return Ok(None);
    }
    budget.spend();
    ai.call(prompt).await.map(Some)
}
```

---

## 5. 文件管理

### 5.1 案件文件夹结构

```rust
pub fn ensure_case_folder(case: &Case) -> Result<PathBuf> {
    let base = case_folder_base(); // ~/Documents/Casy/cases/
    let folder_name = format!("{}_{}", case.case_no.as_deref().unwrap_or("无案号"), case.id[..8].to_string());
    let folder = base.join(&folder_name);

    // 创建子目录
    for sub in &["传票", "证据", "交文", "收文", "内部", "通信", "其他"] {
        std::fs::create_dir_all(folder.join(sub))?;
    }

    Ok(folder)
}

fn case_folder_base() -> PathBuf {
    dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Casy")
        .join("cases")
}
```

### 5.2 智能命名

```rust
pub fn smart_rename(original: &str, case: &Case, category: &str, doc_date: Option<NaiveDate>) -> String {
    let date_str = doc_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| Local::now().naive_local().date().format("%Y-%m-%d").to_string());

    let category_cn = match category {
        "summons" => "传票",
        "evidence" => "证据",
        "submitted" => "交文",
        "received" => "收文",
        "internal" => "内部",
        "correspondence" => "通信",
        _ => "其他",
    };

    let ext = Path::new(original)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("pdf");

    let case_no = case.case_no.as_deref().unwrap_or("无案号");
    let clean_no: String = case_no.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();

    // 基础文件名
    let base = format!("{date_str}_{category_cn}_{clean_no}.{ext}");

    // 防覆盖：如果目标目录已存在同名文件，追加序号
    // 实际使用时传入 target_dir 参数，在此检查文件是否存在
    // 简化实现：追加原始文件名的 SHA-256 前 8 位作为唯一后缀
    let hash = {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(original.as_bytes());
        hasher.update(case.id.as_bytes());
        format!("{:x}", hasher.finalize())
    };
    let hash_suffix = &hash[..8];

    format!("{date_str}_{category_cn}_{clean_no}_{hash_suffix}.{ext}")
}
```

**防覆盖策略**：文件名包含原始文件名 + 案件 ID 的 SHA-256 前 8 位，确保同一天同一案件的同类文件不会互相覆盖。

### 5.3 知识沉淀

```rust
pub async fn extract_knowledge(
    ai: &AiBackend,
    file_path: &Path,
    budget: &TokenBudget,
) -> Result<KnowledgeExtraction> {
    let text = extract_file_text(file_path)?;

    // 截断到前 3000 字（节省 token）
    let truncated = if text.len() > 3000 {
        format!("{}...(省略)...{}", &text[..1500], &text[text.len()-1500..])
    } else {
        text
    };

    let prompt = format!(
        "请从以下法律文档中提取：\n\
         1. 一句话摘要（不超过50字）\n\
         2. 关键词（3-5个，逗号分隔）\n\
         3. 文档类型（传票/证据/交文/收文/内部/通信/其他）\n\
         4. 涉及的当事人名称\n\
         5. 涉及的案号\n\n\
         返回 JSON：{{\"summary\": \"...\", \"keywords\": \"...\", \"doc_type\": \"...\", \"parties\": [...], \"case_nos\": [...]}}\n\n\
         文档内容：\n{truncated}"
    );

    let result = call_ai_with_budget(ai, &prompt, budget).await?;
    match result {
        Some(json) => Ok(serde_json::from_str(&json)?),
        None => Ok(KnowledgeExtraction::default()),
    }
}
```
