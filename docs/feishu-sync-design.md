# 飞书多维表格通用同步设计文档 v3.0

> 版本: 3.0 | 日期: 2026-08-03
> 替代: feishu-sync-design.md v1.0（固定 5 表方案）
> 参考: use-bitable/Excel-Compare-and-Import（飞书表格扩展脚本）
> 状态: 设计评审中

---

## v3.0 变更说明

v1.0 设计是**硬编码 5 张固定表**的同步方案（案件主表、办案日志、庭审信息、任务管理、官方人员）。

**v3.0 核心转变**：从"固定表同步"变为"通用表格处理"——Casy 能连接**任意**飞书多维表格，自动读取表结构，让用户建立字段映射，然后做比较、导入、双向同步。

**设计理念**：飞书多维表格的本质是一个"表 → 字段 → 记录"的通用数据容器。Casy 的飞书模块应该理解这个通用结构，而不是只理解某几张特定的表。

---

## 一、总体架构

### 1.1 核心概念

```
飞书多维表格                    Casy 本地
┌──────────────┐              ┌──────────────┐
│  Base (app)  │              │  SQLite DB   │
│  ├─ Table A  │  ←映射→     │  ├─ cases     │
│  ├─ Table B  │  ←映射→     │  ├─ hearings  │
│  ├─ Table C  │  ←映射→     │  ├─ tasks     │
│  └─ Table D  │  ←新建→     │  └─ custom_X  │
└──────────────┘              └──────────────┘

核心数据结构：
  Base     = 一个多维表格（app_token）
  Table    = 一张表（table_id）
  Field    = 一个字段（field_id + field_name + field_type）
  Record   = 一行数据（record_id + fields{}）
```

### 1.2 功能模块

```
┌─────────────────────────────────────────────────────────┐
│                  飞书同步模块                              │
│                                                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │  连接管理    │  │  表结构发现  │  │  字段映射    │    │
│  │             │  │             │  │             │    │
│  │ App ID/Secret│  │ 列出所有表  │  │ 自动匹配    │    │
│  │ app_token   │  │ 读取字段定义│  │ 手动映射    │    │
│  │ 测试连接    │  │ 类型识别    │  │ 类型转换    │    │
│  └─────────────┘  └─────────────┘  └─────────────┘    │
│                                                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │  比较引擎    │  │  导入引擎    │  │  同步引擎    │    │
│  │             │  │             │  │             │    │
│  │ Schema diff │  │ 全量导入    │  │ 增量 Pull   │    │
│  │ Record diff │  │ 增量导入    │  │ 增量 Push   │    │
│  │ 冲突检测    │  │ 选择性导入  │  │ 冲突解决    │    │
│  └─────────────┘  └─────────────┘  └─────────────┘    │
│                                                         │
│  ┌─────────────┐  ┌─────────────┐                      │
│  │  公式引擎    │  │  链接引擎    │                      │
│  │             │  │             │                      │
│  │ 飞书公式解析│  │ DuplexLink  │                      │
│  │ 本地计算    │  │ SingleLink  │                      │
│  │ 结果缓存    │  │ Lookup      │                      │
│  └─────────────┘  └─────────────┘                      │
└─────────────────────────────────────────────────────────┘
```

---

## 二、连接管理

### 2.1 配置存储

```sql
-- 飞书连接配置
CREATE TABLE IF NOT EXISTS feishu_connections (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,              -- 连接名称（用户自定义）
    app_id          TEXT NOT NULL,
    app_secret      TEXT NOT NULL,              -- 加密存储
    app_token       TEXT,                       -- 多维表格 app_token（可选）
    base_name       TEXT,                       -- 多维表格名称
    status          TEXT DEFAULT 'disconnected'
                    CHECK(status IN ('connected','disconnected','error')),
    last_sync_at    TEXT,
    created_at      TEXT DEFAULT (datetime('now','localtime')),
    updated_at      TEXT DEFAULT (datetime('now','localtime'))
);
```

### 2.2 连接流程

```
用户输入 App ID + App Secret
    ↓
测试连接 → 飞书 API 返回 tenant_access_token
    ↓
输入 app_token（多维表格 URL 中提取）
    ↓
自动发现所有表 → 列出表名和字段数
    ↓
用户选择要同步的表
    ↓
保存连接配置
```

---

## 三、表结构发现

### 3.1 API 调用

```rust
/// 获取多维表格中所有表的列表
async fn list_tables(app_token: &str, auth: &FeishuAuth) -> Result<Vec<TableInfo>> {
    // GET /bitable/v1/apps/{app_token}/tables
    // 返回: [{table_id, name, revision}]
}

/// 获取指定表的所有字段定义
async fn list_fields(app_token: &str, table_id: &str, auth: &FeishuAuth) -> Result<Vec<FieldInfo>> {
    // GET /bitable/v1/apps/{app_token}/tables/{table_id}/fields
    // 返回: [{field_id, field_name, type, property, ...}]
}

/// 获取指定表的所有记录（分页）
async fn list_records(app_token: &str, table_id: &str, auth: &FeishuAuth) -> Result<Vec<RecordInfo>> {
    // GET /bitable/v1/apps/{app_token}/tables/{table_id}/records
    // 分页: page_token + page_size (最大 500)
    // 返回: [{record_id, fields: {field_name: value}, created_by, last_modified_by, ...}]
}
```

### 3.2 表结构缓存

```sql
-- 飞书表结构缓存
CREATE TABLE IF NOT EXISTS feishu_tables (
    id              TEXT PRIMARY KEY,
    connection_id   TEXT NOT NULL REFERENCES feishu_connections(id) ON DELETE CASCADE,
    table_id        TEXT NOT NULL,              -- 飞书 table_id
    table_name      TEXT NOT NULL,              -- 飞书表名
    field_count     INTEGER,
    record_count    INTEGER,
    revision        INTEGER,                    -- 版本号（用于变更检测）
    synced_at       TEXT,
    UNIQUE(connection_id, table_id)
);

-- 飞书字段定义缓存
CREATE TABLE IF NOT EXISTS feishu_fields (
    id              TEXT PRIMARY KEY,
    table_id        TEXT NOT NULL,              -- 飞书 table_id
    field_id        TEXT NOT NULL,              -- 飞书 field_id
    field_name      TEXT NOT NULL,              -- 飞书字段名
    field_type      INTEGER NOT NULL,           -- 飞书类型码
    type_name       TEXT NOT NULL,              -- 类型名称（Text/Number/Select/...）
    is_primary      INTEGER DEFAULT 0,
    property_json   TEXT,                       -- 完整 property（选项/公式/链接配置）
    formula_expr    TEXT,                       -- 公式表达式（仅 Formula 类型）
    created_at      TEXT DEFAULT (datetime('now','localtime')),
    UNIQUE(table_id, field_id)
);
```

### 3.3 飞书字段类型完整映射

```rust
/// 飞书字段类型码 → 类型名 → 本地 SQLite 类型
fn feishu_type_to_sqlite(type_code: i32) -> (&'static str, &'static str) {
    match type_code {
        1     => ("Text", "TEXT"),               // 多行文本
        2     => ("Number", "REAL"),             // 数字
        3     => ("SingleSelect", "TEXT"),        // 单选
        4     => ("MultiSelect", "TEXT"),          // 多选（JSON array）
        5     => ("DateTime", "TEXT"),             // 日期（yyyy-MM-dd HH:mm）
        7     => ("Checkbox", "INTEGER"),          // 复选框（0/1）
        11    => ("User", "TEXT"),                 // 人员（JSON）
        13    => ("Phone", "TEXT"),                // 电话
        15    => ("Url", "TEXT"),                  // 超链接
        17    => ("Attachment", "TEXT"),            // 附件（JSON）
        18    => ("SingleLink", "TEXT"),            // 单向链接（JSON record_ids）
        19    => ("Lookup", "TEXT"),                // 查找引用（不存储，实时计算）
        20    => ("Formula", "TEXT"),               // 公式（存计算结果）
        21    => ("DuplexLink", "TEXT"),             // 双向链接（JSON record_ids）
        22    => ("Location", "TEXT"),              // 地理位置
        23    => ("GroupChat", "TEXT"),             // 群聊
        1001  => ("CreatedTime", "TEXT"),           // 创建时间
        1002  => ("ModifiedTime", "TEXT"),          // 修改时间
        1003  => ("CreatedUser", "TEXT"),           // 创建人
        1004  => ("ModifiedUser", "TEXT"),          // 修改人
        1005  => ("AutoNumber", "TEXT"),            // 自动编号
        3001  => ("Button", ""),                    // 按钮（忽略）
        _     => ("Unknown", "TEXT"),
    }
}
```

---

## 四、字段映射

### 4.1 自动匹配

```
飞书字段 → 本地列 自动匹配规则：

1. 名称完全匹配（优先级最高）
   飞书 "案件信息" → 本地 cases.case_title (如果列名是 case_title 但别名匹配)

2. 名称模糊匹配
   飞书 "案件信息" ↔ 本地 cases.case_info / case_name / title
   飞书 "案号" ↔ 本地 cases.case_no
   飞书 "客户名称" ↔ 本地 cases.client_name

3. 类型匹配
   飞书 DateTime → 本地 TEXT (yyyy-MM-dd)
   飞书 SingleSelect → 本地 TEXT
   飞书 Number → 本地 REAL

4. 已有映射记忆
   feishu_field_mappings 表记录历史映射，下次自动应用
```

### 4.2 映射配置

```sql
-- 字段映射表（核心！连接飞书和本地的桥梁）
CREATE TABLE IF NOT EXISTS feishu_field_mappings (
    id              TEXT PRIMARY KEY,
    connection_id   TEXT NOT NULL REFERENCES feishu_connections(id) ON DELETE CASCADE,
    feishu_table_id TEXT NOT NULL,
    feishu_field_id TEXT NOT NULL,
    feishu_field_name TEXT NOT NULL,
    feishu_field_type INTEGER NOT NULL,
    local_table     TEXT NOT NULL,               -- 本地表名
    local_column    TEXT NOT NULL,               -- 本地列名
    transform_rule  TEXT,                        -- 转换规则 JSON（可选）
    sync_direction  TEXT DEFAULT 'bidirectional'
                    CHECK(sync_direction IN ('pull_only','push_only','bidirectional','none')),
    is_formula      INTEGER DEFAULT 0,           -- 是否公式字段（Push 时不推送）
    is_link         INTEGER DEFAULT 0,           -- 是否链接字段
    is_lookup       INTEGER DEFAULT 0,           -- 是否 Lookup 字段
    created_at      TEXT DEFAULT (datetime('now','localtime')),
    updated_at      TEXT DEFAULT (datetime('now','localtime')),
    UNIQUE(connection_id, feishu_table_id, feishu_field_id)
);

-- 转换规则示例：
-- {"type": "date_format", "from": "timestamp_ms", "to": "yyyy-MM-dd"}
-- {"type": "select_map", "map": {"选项A": "value_a", "选项B": "value_b"}}
-- {"type": "link_resolve", "target_table": "cases", "match_field": "case_no"}
```

### 4.3 映射 UI

```
┌─────────────────────────────────────────────────────────┐
│  字段映射: 案件主表 (tbl4fMNw2UJfXBgy) → cases          │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  飞书字段              本地列              同步方向      │
│  ─────────────────    ────────────────    ──────────    │
│  案件信息 (Text)  →   case_title (TEXT)   ↔ 双向   ✅  │
│  案号 (Text)      →   case_no (TEXT)      ↔ 双向   ✅  │
│  案件状态 (Formula)→   case_status (TEXT)  ← 仅拉取 ✅  │
│  客户名称 (Text)  →   client_name (TEXT)  ↔ 双向   ✅  │
│  事件记录 (DupLink)→   [跳过 - 链接字段]  — 不同步 ⬜  │
│  开庭时间 (Date)  →   trial_date (TEXT)   ↔ 双向   ✅  │
│  (新增字段)       →   [创建新列...]       ← 仅拉取 ✅  │
│                                                         │
│  [自动匹配]  [全部拉取]  [全部双向]  [保存映射]         │
└─────────────────────────────────────────────────────────┘
```

---

## 五、比较引擎（核心！）

### 5.1 Schema 比较

```
比较飞书表结构 vs 本地表结构：

1. 飞书有、本地没有的字段 → "新增"（可选创建本地列或跳过）
2. 本地有、飞书没有的字段 → "仅本地"（不同步到飞书）
3. 双方都有但类型不同   → "类型冲突"（需要转换规则）
4. 双方都有且类型兼容   → "可映射"（自动建立映射）
```

### 5.2 记录比较

```
比较飞书记录 vs 本地记录：

匹配策略（用户可选）：
  - 按主键匹配（飞书主键字段 ↔ 本地指定列）
  - 按 record_id 匹配（已同步过的记录）
  - 按模糊匹配（案号/名称等）

比较结果分类：
  ┌───────────────────────────────────────────────┐
  │  类型        数量    操作                      │
  │  ──────────  ────    ──────────────────        │
  │  相同        42      无需操作                  │
  │  仅飞书更新  5       可拉取（飞书 → 本地）     │
  │  仅本地更新  3       可推送（本地 → 飞书）     │
  │  双方都改    2       冲突，需用户选择          │
  │  仅飞书有    8       新记录，可导入            │
  │  仅本地有    1       本地独有，可推送到飞书     │
  └───────────────────────────────────────────────┘
```

### 5.3 比较 UI

```
┌─────────────────────────────────────────────────────────┐
│  表比较: 案件主表 ↔ cases                                │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  📊 比较摘要                                             │
│  相同: 42 | 飞书更新: 5 | 本地更新: 3 | 冲突: 2        │
│  仅飞书: 8 | 仅本地: 1                                  │
│                                                         │
│  [拉取飞书更新(5)] [推送本地更新(3)] [处理冲突(2)]      │
│  [导入仅飞书记录(8)] [推送仅本地记录(1)]                │
│                                                         │
│  ─── 详细差异 ───                                       │
│                                                         │
│  📄 浦项 v NSC (案件信息)                                │
│     飞书: 案件进展="结案"  本地: 案件进展="进行中"       │
│     飞书更新时间: 2026-08-02  本地更新时间: 2026-08-01  │
│     [用飞书] [用本地] [手动编辑]                         │
│                                                         │
│  📄 钛金 v 高德 (案件信息)                               │
│     飞书: 备注="已提交补充意见"  本地: 备注=""           │
│     [用飞书] [用本地] [合并]                             │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

---

## 六、导入引擎

### 6.1 全量导入

```
用户选择飞书表 + 映射配置
    ↓
1. 拉取所有记录（分页，每页 500 条）
2. 按映射规则转换字段值
3. 按主键/record_id 判断：新增 or 更新
4. 批量写入本地 SQLite
5. 记录 record_id 映射到 sync_map
6. 报告导入结果
```

### 6.2 增量导入

```
基于 sync_map 中的 last_sync_at
    ↓
1. 拉取 last_modified_time > 上次同步时间的记录
2. 按映射规则转换
3. 本地有 → 更新；本地无 → 新增
4. 记录同步状态
```

### 6.3 选择性导入

```
用户在比较结果中勾选要导入的记录
    ↓
仅导入勾选的记录
```

---

## 七、同步引擎

### 7.1 双向同步流程

```
同步触发（手动/定时/自动）
    ↓
Phase 1: Pull（飞书 → 本地）
  - 拉取变更记录（增量）
  - 按映射转换
  - 写入本地
  - 更新 sync_map

Phase 2: Push（本地 → 飞书）
  - 检查本地变更（sync_queue）
  - 按映射转换
  - 推送到飞书（注意：跳过 Formula/Lookup/Button 字段）
  - 更新 sync_map

Phase 3: 冲突处理
  - 双方都改过的记录 → 字段级合并
  - 非重叠字段自动合并
  - 重叠字段按策略处理（飞书优先/本地优先/弹窗让用户选）
```

### 7.2 字段同步规则

```
同步方向控制（每个字段独立配置）：

双向 (bidirectional):
  Pull: 飞书值 → 本地
  Push: 本地值 → 飞书
  适用: Text, Number, SingleSelect, DateTime, Checkbox

仅拉取 (pull_only):
  Pull: 飞书值 → 本地
  Push: 不推送
  适用: Formula, Lookup, CreatedTime, ModifiedTime, AutoNumber

仅推送 (push_only):
  Pull: 不拉取
  Push: 本地值 → 飞书
  适用: 本地独有字段

不同步 (none):
  跳过
  适用: Button, 未映射字段
```

### 7.3 公式字段处理

```
Pull 方向:
  飞书 Formula 字段的计算结果 → 本地缓存列
  同时存储飞书的 formula_expression 到 feishu_fields.formula_expr
  本地公式引擎可选择使用飞书公式本地重算

Push 方向:
  Formula 字段不推送（飞书有自己的公式引擎）
  但 Push 前确保依赖的输入字段已推送
  Push 完成后，可重新 Pull Formula 结果
```

### 7.4 链接字段处理

```
DuplexLink / SingleLink:
  Pull: 解析 record_ids → 通过 sync_map 查找本地 ID → 建立本地关联
  Push: 本地关联变更 → 查找飞书 record_id → 通过 API 更新链接

Lookup:
  不存储，通过本地 SQL VIEW 实时查询
  或通过公式引擎本地计算
```

---

## 八、通用本地表管理

### 8.1 动态创建本地表

当飞书表没有对应的本地表时，Casy 可以自动创建：

```
用户选择导入飞书表"合同管理"（Casy 中不存在）
    ↓
Casy 自动创建本地表:
  CREATE TABLE IF NOT EXISTS feishu_contract_mgmt (
    id TEXT PRIMARY KEY,
    feishu_record_id TEXT UNIQUE,
    合同名称 TEXT,
    合同金额 REAL,
    签约日期 TEXT,
    ...
    created_at TEXT,
    updated_at TEXT
  );
    ↓
自动建立字段映射
    ↓
导入数据
```

### 8.2 通用表命名

```
本地表名规则:
  - 如果映射到已有表（如 cases） → 使用已有表
  - 如果是新表 → feishu_{表名拼音/英文}
  - 用户可自定义表名
```

---

## 九、与 Inbox 的集成

```
飞书多维表格新增记录
    ↓
Casy 定时拉取（或手动触发）
    ↓
新记录 → inbox_item（source_type='feishu'）
    ↓
进入 Inbox 推荐流程
    ↓
用户确认 → 归档到本地表
```

---

## 十、实施计划

| 序号 | 任务 | 工作量 | 优先级 |
|------|------|--------|--------|
| 1 | 连接管理（feishu_connections 表 + 测试连接） | 1d | P0 |
| 2 | 表结构发现（list_tables + list_fields + 缓存） | 1d | P0 |
| 3 | 字段映射（自动匹配 + 手动映射 + feishu_field_mappings） | 2d | P0 |
| 4 | 比较引擎（schema diff + record diff + 冲突检测） | 3d | P0 |
| 5 | 导入引擎（全量/增量/选择性导入） | 2d | P0 |
| 6 | 同步引擎（双向 Pull/Push + 冲突解决） | 3d | P1 |
| 7 | 公式引擎（飞书公式本地解析 + 计算） | 5d | P1 |
| 8 | 链接引擎（DuplexLink/SingleLink/Lookup） | 3d | P2 |
| 9 | 前端 UI（连接管理 + 映射配置 + 比较视图 + 同步状态） | 5d | P1 |
| 10 | 通用表管理（动态创建本地表） | 2d | P2 |
| 11 | 测试 + 文档 | 2d | P0 |
| **总计** | | **~29d** | |

### 优先级说明

**P0（必须先做）**：连接管理 + 表结构发现 + 字段映射 + 比较引擎 + 导入引擎
→ 这 5 项做完就能实现"连接任意飞书表 → 比较差异 → 选择性导入"

**P1（第二阶段）**：同步引擎 + 公式引擎 + 前端 UI
→ 实现双向同步和飞书公式本地计算

**P2（第三阶段）**：链接引擎 + 通用表管理
→ 完善链接关系同步和动态建表

---

## 附录 A: 飞书 Bitable API 清单

| API | 用途 | 频率限制 |
|-----|------|----------|
| `GET /bitable/v1/apps/{app_token}/tables` | 列出所有表 | 100/min |
| `GET /bitable/v1/apps/{app_token}/tables/{table_id}/fields` | 列出所有字段 | 100/min |
| `GET /bitable/v1/apps/{app_token}/tables/{table_id}/records` | 列出记录（分页） | 100/min |
| `POST /bitable/v1/apps/{app_token}/tables/{table_id}/records` | 创建记录 | 100/min |
| `PUT /bitable/v1/apps/{app_token}/tables/{table_id}/records/{record_id}` | 更新记录 | 100/min |
| `DELETE /bitable/v1/apps/{app_token}/tables/{table_id}/records/{record_id}` | 删除记录 | 100/min |
| `POST /bitable/v1/apps/{app_token}/tables/{table_id}/records/batch_create` | 批量创建 | 100/min |
| `POST /bitable/v1/apps/{app_token}/tables/{table_id}/records/batch_update` | 批量更新 | 100/min |

## 附录 B: 飞书字段类型码

| 类型码 | 类型名 | 说明 | 可同步 |
|--------|--------|------|--------|
| 1 | Text | 多行文本 | ✅ 双向 |
| 2 | Number | 数字 | ✅ 双向 |
| 3 | SingleSelect | 单选 | ✅ 双向 |
| 4 | MultiSelect | 多选 | ✅ 双向 |
| 5 | DateTime | 日期时间 | ✅ 双向 |
| 7 | Checkbox | 复选框 | ✅ 双向 |
| 11 | User | 人员 | ⚠️ 仅拉取 |
| 13 | Phone | 电话 | ✅ 双向 |
| 15 | Url | 超链接 | ✅ 双向 |
| 17 | Attachment | 附件 | ⚠️ 需下载/上传 |
| 18 | SingleLink | 单向链接 | ⚠️ 需解析 |
| 19 | Lookup | 查找引用 | ❌ 不存储 |
| 20 | Formula | 公式 | ⚠️ 仅拉取结果 |
| 21 | DuplexLink | 双向链接 | ⚠️ 需解析 |
| 1001 | CreatedTime | 创建时间 | ❌ 自动生成 |
| 1002 | ModifiedTime | 修改时间 | ❌ 自动生成 |
| 3001 | Button | 按钮 | ❌ 忽略 |

---

> 最后更新: 2026-08-03 (v3.0: 从固定 5 表方案改为通用表格处理)
