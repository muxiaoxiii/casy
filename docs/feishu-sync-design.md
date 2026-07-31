# 飞书多维表格同步设计文档

> 版本: 1.0 | 日期: 2026-07-31
> 基于 `docs/archive/feishu-base/full-schema.json` 实际数据导出分析

---

## 1. 飞书表格分析

### 1.1 表概览

| 表名 | Table ID | 记录数 | 字段数 | 主键字段 |
|------|----------|--------|--------|----------|
| 案件主表 | `tbl4fMNw2UJfXBgy` | 59 | 50 | 案件信息 (fld1LgpSfa) |
| 办案日志 | `tblAB7NpSVKXX9uN` | 67 | 9 | 事件概述 (fldydpgUuR) |
| 庭审信息 | `tblXrb7Y6c9i2o8D` | 55 | 15 | 开庭记录 (fldydpgUuR) |
| 任务管理 | `tblVzAWjugfmRqhR` | 19 | 11 | 任务名称 (fldiQqAsjJ) |
| 官方人员联系方式 | `tblYAqcFHwtehbRP` | 9 | 7 | 姓名 (fldGJQaSuU) |

### 1.2 案件主表 (tbl4fMNw2UJfXBgy) — 完整字段清单

| 字段名 | field_id | 类型 | 飞书 type | 备注 |
|--------|----------|------|-----------|------|
| 案件信息 | fld1LgpSfa | Text | 1 | **主键** |
| 案件状态 | fldIQpzbjd | **Formula** | 20 | → SingleSelect |
| 案号 | fld3rWg13c | Text | 1 | |
| 案由 | fld2wuIiuT | SingleSelect | 3 | 10 选项 |
| 内部卷号 | fldmP8IWZ1 | Text | 1 | |
| 客户名称 | fld7mTj8Bj | Text | 1 | |
| 事件记录 | fldAX330FF | **DuplexLink** | 21 | ↔ 办案日志.案件名称 |
| 未来开庭 | fld9H2phgx | **Lookup** | 19 | 从庭审信息过滤 |
| 最近已开庭 | fldDS9HLDo | **Lookup** | 19 | 从庭审信息过滤 |
| 案件进展 | fld6KMKs7x | SingleSelect | 3 | 14 选项 |
| 我方诉讼地位 | fld7Im0sn5 | Text | 1 | |
| 专利名称 | fld2ajvXbz | Text | 1 | |
| 专利申请号 | fld6XVvJgX | Text | 1 | |
| 办案人 | fldEhUlsv7 | MultiSelect | 4 | 动态选项 |
| 对方名称 | fld1L4VW0o | Text | 1 | |
| 诉讼地位 | fld7vF4iAH | SingleSelect | 3 | 10 选项 |
| 对方代理律所 | fld57aBCWi | Text | 1 | |
| 对方代理人 | fld6ErPj9l | Text | 1 | |
| 审理机关 | fld4kjHRKm | SingleSelect | 3 | 14 选项 |
| 合议庭 | fld7hd2Ppo | **Lookup** | 19 | 从庭审信息获取 |
| 书记员\|助理 | fld6g6AxBO | Text | 1 | |
| 审级 | fld6syq1PK | SingleSelect | 3 | 5 选项 |
| 金助理案号 | fldBbGWPHK | Text | 1 | |
| 立案 | fld6LYjpEf | DateTime | 5 | yyyy/MM/dd |
| 管辖异议 | fld4RB3umT | Text | 1 | |
| 开庭\|口审 | fldukZuLIj | DateTime | 5 | yyyy/MM/dd HH:mm |
| 二次开庭\|口审 | fldaK9Cexg | DateTime | 5 | yyyy/MM/dd |
| 三次开庭丨口审 | fldnI0PEZU | DateTime | 5 | yyyy/MM/dd |
| 收到判决/裁定/决定类型 | fldykBjFYs | SingleSelect | 3 | 6 选项 |
| 收到判决/裁定/决定时间 | fldzL0qOZE | DateTime | 5 | yyyy/MM/dd |
| 案件结果 | fld5UuLSj7 | SingleSelect | 3 | 7 选项 |
| 已完成 | fld5avH0sN | Text | 1 | |
| 备注 | fld2gTteIh | Text | 1 | |
| 收到起诉状时间 | fldT4czNN7 | DateTime | 5 | yyyy/MM/dd |
| 提交答辩状期间 | fldplo3Mm6 | **Formula** | 20 | → DateTime |
| 裁定中止日 | fld6z5uSqS | DateTime | 5 | yyyy/MM/dd |
| 诉讼程序 | fldFxfNPij | SingleSelect | 3 | 2 选项 |
| 预估审限 | fldB2TLEnB | **Formula** | 20 | → DateTime |
| 救济期限 | fldoQ7ONlI | Text | 1 | |
| 请求人首次无效时间 | flddg6CGhV | **Formula** | 20 | → DateTime |
| 请求人补充意见期限 | fldFqVti0u | **Formula** | 20 | → DateTime |
| 请求人提交补充意见时间 | fldAMQC7KV | DateTime | 5 | yyyy/MM/dd |
| 请求人收到专利权人意见时间 | fldVSsVv42 | DateTime | 5 | yyyy/MM/dd |
| 请求人答复意见期限 | fld49kHE9Z | **Formula** | 20 | → DateTime |
| 专利权人收到受通时间 | fldqDhmHIs | DateTime | 5 | yyyy/MM/dd |
| 专利权人陈述意见期限 | fldKvDz6IP | **Formula** | 20 | → DateTime |
| 专利权人收到补充意见时间 | fldjXAoy9B | DateTime | 5 | yyyy/MM/dd |
| 专利权人补充意见时间 | fldWnSzhO0 | **Formula** | 20 | → DateTime |
| 专利权人提交补充意见时间 | fldHHiKYGf | DateTime | 5 | yyyy/MM/dd |
| 关联案件 | fld0kRR2a8 | **SingleLink** | 18 | → 案件主表 (自引用) |
| 开庭记录 | fldHT3U9Vy | **DuplexLink** | 21 | ↔ 庭审信息.案件信息 |
| 官方人员联系方式 | fldcEeULn3 | **DuplexLink** | 21 | ↔ 官方人员.关联案件 |

### 1.3 办案日志 (tblAB7NpSVKXX9uN) — 字段清单

| 字段名 | field_id | 类型 | 飞书 type | 备注 |
|--------|----------|------|-----------|------|
| 事件概述 | fldydpgUuR | Text | 1 | **主键** |
| 案件名称 | fldqqWhNtP | **DuplexLink** | 21 | ↔ 案件主表.事件记录 |
| 事件名称 | fldAns9v64 | Text | 1 | |
| 案号 | fldrglfEhA | **Lookup** | 19 | 从案件主表获取 |
| 发生时间 | fldXapQ4bm | DateTime | 5 | auto_fill=true |
| 操作内容 | fldrGi2wY0 | Text | 1 | |
| 类型 | fldDbDNUBa | SingleSelect | 3 | 4 选项: 任务/交文/收文/记录 |
| 附件 | fldF0jAMMz | Attachment | 17 | |
| 创建任务按钮 | flduSzppEK | Button | 3001 | |

### 1.4 庭审信息 (tblXrb7Y6c9i2o8D) — 字段清单

| 字段名 | field_id | 类型 | 飞书 type | 备注 |
|--------|----------|------|-----------|------|
| 开庭记录 | fldydpgUuR | Text | 1 | **主键** |
| 案件信息 | fldqqWhNtP | **DuplexLink** | 21 | ↔ 案件主表.开庭记录 |
| 案号 | fldkwqJabM | **Lookup** | 19 | 从案件主表获取 |
| 开庭名称 | fldAns9v64 | Text | 1 | |
| 开庭时间 | fldXapQ4bm | DateTime | 5 | auto_fill=true |
| 出庭人员 | fldSTAhbud | Text | 1 | |
| 审理机关 | fldLMp3Bb6 | **Lookup** | 19 | 从案件主表获取 |
| 开庭地点 | fldrGi2wY0 | Text | 1 | |
| 审判人员 | fldcsOOS31 | MultiSelect | 4 | 58 选项 (法官名单) |
| 状态 | fld49H54nQ | **Formula** | 20 | → SingleSelect |
| 联系方式 | fld8ggNevi | **Lookup** | 19 | 从官方人员获取 |
| 审级 | fldfFx7Sth | **Lookup** | 19 | 从案件主表获取 |
| 附件 | fldF0jAMMz | Attachment | 17 | |
| 发送邮件按钮 | fldJ32QWfT | Button | 3001 | |
| 实际开庭情况 | fldUFzPk5u | SingleSelect | 3 | 2 选项: 已开/未开 |

### 1.5 任务管理 (tblVzAWjugfmRqhR) — 字段清单

| 字段名 | field_id | 类型 | 飞书 type | 备注 |
|--------|----------|------|-----------|------|
| 任务名称 | fldiQqAsjJ | Text | 1 | **主键** |
| 任务详细描述 | fld1sdBTPk | Text | 1 | |
| 创建日期 | fldrQjbzGR | DateTime | 5 | auto_fill=true |
| 截止日期 | fld6Gxc8Mj | DateTime | 5 | |
| 距离截止日 | fldpkxxmhh | **Formula** | 20 | → Text |
| 优先级 | fld1xutT1t | SingleSelect | 3 | 4 选项 |
| 关联项目 | fld5JABBD7 | **SingleLink** | 18 | → 案件主表 (多选) |
| 完成状态 | fldoAS6Hfr | Checkbox | 7 | |
| 创建任务 | fld0I36h5d | Button | 3001 | |
| 任务执行人 | fldrBkE9pI | User | 11 | multiple=true |
| 完结记录 | fld3MVRiNB | Text | 1 | |

### 1.6 官方人员联系方式 (tblYAqcFHwtehbRP) — 字段清单

| 字段名 | field_id | 类型 | 飞书 type | 备注 |
|--------|----------|------|-----------|------|
| 姓名 | fldGJQaSuU | Text | 1 | **主键** |
| 身份 | fldOTnsU7g | SingleSelect | 3 | 4 选项 |
| 所属机关 | flddjKhfLC | SingleSelect | 3 | 13 选项 |
| 具体联系方式 | fldJYXiIuj | Text | 1 | |
| 联系记录 | fldSZCYTWq | Text | 1 | |
| 联系方式 | fldZPxVfh6 | Text | 1 | |
| 关联案件 | fldHEgebO3 | **DuplexLink** | 21 | ↔ 案件主表.官方人员联系方式 |

---

### 1.7 所有公式表达式（完整）

#### 案件主表 — 案件状态 (fldIQpzbjd)
```
IF(OR(
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6KMKs7x]="结案",
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6KMKs7x]="胜诉",
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6KMKs7x]="败诉",
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6KMKs7x]="对方撤案"
),"已完结",
IF(ISBLANK(bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6KMKs7x]),"未知","进行中"))
```
- **输出类型**: SingleSelect（进行中/已完结/未知）
- **逻辑**: 基于 `案件进展` 判断案件状态

#### 案件主表 — 提交答辩状期间 (fldplo3Mm6)
```
IF(OR(
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld2wuIiuT]="专利无效",
  ISBLANK(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldT4czNN7])
),"",
IF(WORKDAY(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldT4czNN7] + 14, 1)
  = bitable::$table[tbl4fMNw2UJfXBgy].$field[fldT4czNN7] + 15,
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fldT4czNN7] + 15,
  WORKDAY(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldT4czNN7] + 15, -1)))
```
- **输出类型**: DateTime (yyyy/MM/dd)
- **逻辑**: 收到起诉状后 15 天内提交答辩状（排除专利无效，顺延到工作日）

#### 案件主表 — 预估审限 (fldB2TLEnB)
```
IF(ISBLANK(bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6LYjpEf]),"",
IF(AND(
  NOT(ISBLANK(bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6z5uSqS])),
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6z5uSqS] <= IF(
    bitable::$table[tbl4fMNw2UJfXBgy].$field[fld2wuIiuT]="专利无效",
    bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6LYjpEf]+5*30,
    IF(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldFxfNPij]="简易",
      EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6LYjpEf],3),
      EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6LYjpEf],6))
  )
),"",
IF(bitable::$table[tbl4fMNw2UJfXBgy].$field[fld2wuIiuT]="专利无效",
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6LYjpEf]+5.7*30,
  IF(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldFxfNPij]="简易",
    EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6LYjpEf],3),
    EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6LYjpEf],6)))))
```
- **输出类型**: DateTime (yyyy/MM/dd)
- **逻辑**: 根据案由和程序类型计算预估审限，已中止则返回空

#### 案件主表 — 请求人首次无效时间 (flddg6CGhV)
```
IF(bitable::$table[tbl4fMNw2UJfXBgy].$field[fld2wuIiuT]="专利无效",
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6LYjpEf],"")
```
- **输出类型**: DateTime
- **逻辑**: 专利无效时取立案时间

#### 案件主表 — 请求人补充意见期限 (fldFqVti0u)
```
IF(AND(
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld2wuIiuT]="专利无效",
  NOT(ISBLANK(bitable::$table[tbl4fMNw2UJfXBgy].$field[flddg6CGhV]))),
IF(WORKDAY(EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[flddg6CGhV],1)-1,1)
  =EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[flddg6CGhV],1),
  EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[flddg6CGhV],1),
  WORKDAY(EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[flddg6CGhV],1),-1)),"")
```
- **输出类型**: DateTime
- **逻辑**: 首次无效时间 + 1 个月，顺延到工作日

#### 案件主表 — 请求人答复意见期限 (fld49kHE9Z)
```
IF(AND(
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld2wuIiuT]="专利无效",
  NOT(ISBLANK(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldVSsVv42]))),
IF(WORKDAY(EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldVSsVv42],1)-1,1)
  =EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldVSsVv42],1),
  EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldVSsVv42],1),
  WORKDAY(EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldVSsVv42],1),-1)),"")
```
- **输出类型**: DateTime
- **逻辑**: 请求人收到专利权人意见 + 1 个月

#### 案件主表 — 专利权人陈述意见期限 (fldKvDz6IP)
```
IF(AND(
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld2wuIiuT]="专利无效",
  NOT(ISBLANK(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldqDhmHIs]))),
IF(WORKDAY(EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldqDhmHIs],1)-1,1)
  =EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldqDhmHIs],1),
  EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldqDhmHIs],1),
  WORKDAY(EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldqDhmHIs],1),-1)),"")
```
- **输出类型**: DateTime
- **逻辑**: 专利权人收到受通 + 1 个月

#### 案件主表 — 专利权人补充意见时间 (fldWnSzhO0)
```
IF(AND(
  bitable::$table[tbl4fMNw2UJfXBgy].$field[fld2wuIiuT]="专利无效",
  NOT(ISBLANK(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldjXAoy9B]))),
IF(WORKDAY(EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldjXAoy9B],1)-1,1)
  =EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldjXAoy9B],1),
  EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldjXAoy9B],1),
  WORKDAY(EDATE(bitable::$table[tbl4fMNw2UJfXBgy].$field[fldjXAoy9B],1),-1)),"")
```
- **输出类型**: DateTime
- **逻辑**: 专利权人收到补充意见 + 1 个月

#### 庭审信息 — 状态 (fld49H54nQ)
```
IF(bitable::$table[tblXrb7Y6c9i2o8D].$field[fldXapQ4bm]<TODAY(),"已开","待开")
```
- **输出类型**: SingleSelect（已开/待开/未知）
- **逻辑**: 根据开庭时间是否已过判断

#### 任务管理 — 距离截止日 (fldpkxxmhh)
```
IF(AND(
  bitable::$table[tblVzAWjugfmRqhR].$field[fldoAS6Hfr]=0,
  ISBLAnk(bitable::$table[tblVzAWjugfmRqhR].$field[fld6Gxc8Mj])=false),
IF(TODAY()-bitable::$table[tblVzAWjugfmRqhR].$field[fld6Gxc8Mj]<=0,
  "🕑还有"&(bitable::$table[tblVzAWjugfmRqhR].$field[fld6Gxc8Mj]-TODAY())&"天到期",
  "⁉️已延期"),"")
```
- **输出类型**: Text
- **逻辑**: 未完成且有截止日期时，显示剩余天数或已延期

### 1.8 Lookup 字段公式（完整）

#### 案件主表 — 未来开庭 (fld9H2phgx)
```
bitable::$table[tblXrb7Y6c9i2o8D]
  .FILTER(CurrentValue.$column[fldXapQ4bm]>TODAY()
    &&CurrentValue.$column[fldqqWhNtP]=bitable::$table[tbl4fMNw2UJfXBgy].$field[fld1LgpSfa])
  .$column[fldydpgUuR].LISTCOMBINE()
```
- 源表: 庭审信息
- 过滤: 开庭时间 > 今天 AND 案件信息 = 当前记录
- 提取字段: 开庭记录 (fldydpgUuR)

#### 案件主表 — 最近已开庭 (fldDS9HLDo)
```
bitable::$table[tblXrb7Y6c9i2o8D]
  .FILTER(CurrentValue.$column[fldXapQ4bm]<TODAY()
    &&CurrentValue.$column[fldqqWhNtP]=bitable::$table[tbl4fMNw2UJfXBgy].$field[fld1LgpSfa]
    &&CurrentValue.$column[fldUFzPk5u]!=bitable::$table[tblXrb7Y6c9i2o8D].$column[fldUFzPk5u].$option[optL1lFlnX])
  .$column[fldydpgUuR].LISTCOMBINE()
```
- 额外过滤: 实际开庭情况 ≠ 未开

#### 案件主表 — 合议庭 (fld7hd2Ppo)
```
bitable::$table[tblXrb7Y6c9i2o8D]
  .FILTER(CurrentValue.$column[fldqqWhNtP]=bitable::$table[tbl4fMNw2UJfXBgy].$field[fld1LgpSfa])
  .$column[fldcsOOS31].LISTCOMBINE()
```
- 源表: 庭审信息
- 提取字段: 审判人员 (fldcsOOS31) — MultiSelect

#### 办案日志 — 案号 (fldrglfEhA)
```
bitable::$table[tbl4fMNw2UJfXBgy]
  .FILTER(CurrentValue.$column[fld1LgpSfa]=bitable::$table[tblAB7NpSVKXX9uN].$field[fldqqWhNtP])
  .$column[fld3rWg13c].LISTCOMBINE()
```

#### 庭审信息 — 案号 (fldkwqJabM)
```
bitable::$table[tbl4fMNw2UJfXBgy]
  .FILTER(CurrentValue.$column[fld1LgpSfa]=bitable::$table[tblXrb7Y6c9i2o8D].$field[fldqqWhNtP])
  .$column[fld3rWg13c].LISTCOMBINE()
```

#### 庭审信息 — 审理机关 (fldLMp3Bb6)
```
bitable::$table[tbl4fMNw2UJfXBgy]
  .FILTER(CurrentValue.$column[fld1LgpSfa]=bitable::$table[tblXrb7Y6c9i2o8D].$field[fldqqWhNtP])
  .$column[fld4kjHRKm].LISTCOMBINE()
```

#### 庭审信息 — 联系方式 (fld8ggNevi)
```
bitable::$table[tblYAqcFHwtehbRP]
  .FILTER(CurrentValue.$column[fldHEgebO3].CONTAIN(bitable::$table[tblXrb7Y6c9i2o8D].$field[fldqqWhNtP]))
  .$column[fldZPxVfh6].LISTCOMBINE()
```

#### 庭审信息 — 审级 (fldfFx7Sth)
```
bitable::$table[tbl4fMNw2UJfXBgy]
  .FILTER(CurrentValue.$column[fld1LgpSfa]=bitable::$table[tblXrb7Y6c9i2o8D].$field[fldqqWhNtP])
  .$column[fld6syq1PK].LISTCOMBINE()
```

### 1.9 链接关系全景图

```
案件主表 ◄══════════════════════════════════════════════╗
  │                                                      ║
  ├── DuplexLink ──► 办案日志.案件名称                    ║
  │   (fldAX330FF → fldqqWhNtP, back_field)              ║
  │                                                      ║
  ├── DuplexLink ──► 庭审信息.案件信息                    ║
  │   (fldHT3U9Vy → fldqqWhNtP, back_field)              ║
  │                                                      ║
  ├── DuplexLink ──► 官方人员联系方式.关联案件            ║
  │   (fldcEeULn3 → fldHEgebO3, back_field)              ║
  │                                                      ║
  ├── SingleLink ──► 案件主表 (自引用)                    ║
  │   (fld0kRR2a8 → tbl4fMNw2UJfXBgy, 关联案件)          ║
  │                                                      ║
  ├── Lookup ◄──── 庭审信息 (未来开庭/最近已开庭/合议庭)  ║
  │                                                      ║
  └══════════════════════════════════════════════════════╝

任务管理 ── SingleLink ──► 案件主表 (关联项目)
```

**DuplexLink（双向链接）详情**：

| 链接名 | 源字段 | 目标表 | 目标字段 | 关系 |
|--------|--------|--------|----------|------|
| 事件记录 | 案件主表.fldAX330FF | 办案日志 | fldqqWhNtP | 1:N |
| 开庭记录 | 案件主表.fldHT3U9Vy | 庭审信息 | fldqqWhNtP | 1:N |
| 官方人员联系方式 | 案件主表.fldcEeULn3 | 官方人员 | fldHEgebO3 | N:N |
| 案件名称 | 办案日志.fldqqWhNtP | 案件主表 | fldAX330FF | N:1 (反向) |
| 案件信息 | 庭审信息.fldqqWhNtP | 案件主表 | fldHT3U9Vy | N:1 (反向) |
| 关联案件 | 官方人员.fldHEgebO3 | 案件主表 | fldcEeULn3 | N:N (反向) |

---

## 2. 能力映射

### 2.1 Formula（公式字段）

| 维度 | 飞书 | Casy SQLite + Rust |
|------|------|---------------------|
| 工作原理 | `formula_expression` 存储公式，引擎实时计算并显示结果 | **当前**: `DeadlineEngine` 仅处理期限规则（trigger_field + offset），通过 SQL 触发器计算 `case_status` |
| 语法 | `IF`, `OR`, `AND`, `NOT`, `ISBLANK`, `EDATE`, `WORKDAY`, `TODAY()` | **缺失**: 无通用公式解析器 |
| 跨表引用 | `bitable::$table[xxx].$field[yyy]` | **缺失**: 无跨表引用机制 |
| 输出类型 | 可返回 Text/SingleSelect/DateTime | **部分**: case_status 触发器返回文本 |
| **已实现** | — | `case_status` 自动计算（SQL trigger）；`DeadlineEngine` 期限计算（Rust） |
| **缺失** | — | 通用公式引擎；`EDATE`/`WORKDAY`/`TODAY` 函数；`ISBLANK`/`OR`/`AND` 运算符；公式元数据存储 |

### 2.2 DuplexLink（双向链接）

| 维度 | 飞书 | Casy SQLite + Rust |
|------|------|---------------------|
| 工作原理 | `back_field_id` 声明反向关系，A↔B 自动同步 | **当前**: `case_officials`（M:N 关联表）、`case_relations`（自引用）、FK `case_id` 在 logs/hearings/tasks |
| 维护 | 添加/删除任一侧自动更新对侧 | **手动**: 需在两侧分别维护 |
| 数据格式 | `[{record_ids:[], table_id, text}]` | TEXT FK 或关联表 |
| **已实现** | — | `case_logs.case_id` FK、`hearings.case_id` FK、`case_officials` M:N |
| **缺失** | — | 飞书 record_id ↔ 本地 ID 映射；双向链接自动维护；链接元数据存储 |

### 2.3 SingleLink（单向链接）

| 维度 | 飞书 | Casy SQLite + Rust |
|------|------|---------------------|
| 工作原理 | 指向另一表记录，不创建反向字段 | **当前**: `case_relations` 自引用表；`tasks.case_id` FK |
| 数据格式 | `[{record_ids:[], table_id}]` | TEXT FK |
| **已实现** | — | `tasks.case_id`、`case_relations` |
| **缺失** | — | 任务→案件的 SingleLink 未完整映射（`multiple:true` 允许多选） |

### 2.4 Lookup（查找引用）

| 维度 | 飞书 | Casy SQLite + Rust |
|------|------|---------------------|
| 工作原理 | 通过链接关系从关联表提取字段值，支持 FILTER + LISTCOMBINE | **当前**: `import_hearing` 中手动查询 `lookup_case_court_level()`；SQL JOIN |
| 公式 | `bitable::$table[xxx].FILTER(...).$column[yyy].LISTCOMBINE()` | **缺失**: 无通用 Lookup 引擎 |
| **已实现** | — | 手动 SQL JOIN 查询 |
| **缺失** | — | 通用 Lookup 引擎；Lookup 公式存储与执行 |

### 2.5 SingleSelect / MultiSelect（单选/多选）

| 维度 | 飞书 | Casy SQLite + Rust |
|------|------|---------------------|
| 工作原理 | options 数组带 color/id/name；SingleSelect 存文本或 option_id | **当前**: TEXT 字段存储字符串值；MultiSelect 存 JSON 数组 |
| 选项管理 | 选项定义在 field property 中 | **缺失**: 无选项元数据表 |
| 颜色 | color 属性 (0-51) | **缺失**: 无颜色存储 |
| **已实现** | — | SingleSelect → TEXT；MultiSelect → JSON TEXT |
| **缺失** | — | 选项定义/颜色存储；选项 ID ↔ 名称映射；动态选项管理 |

### 2.6 Button（按钮）

| 维度 | 飞书 | Casy SQLite + Rust |
|------|------|---------------------|
| 工作原理 | 触发自动化操作（如创建关联记录、发送邮件） | **无等价物** |
| 实例 | 创建任务按钮、发送邮件按钮 | **缺失**: 可用 Tauri command 替代 |
| **建议** | — | 不需存储按钮字段，实现为 UI 层的 Tauri command |

### 2.7 Attachment（附件）

| 维度 | 飞书 | Casy SQLite + Rust |
|------|------|---------------------|
| 工作原理 | 文件上传到飞书云存储，字段存储文件元数据 | **当前**: `case_files` 表、`files_json` 字段 |
| 数据格式 | `[{file_token, name, size, type, url}]` | `[{name, path, size}]` JSON |
| **已实现** | — | `case_files` 表完整 |
| **缺失** | — | 飞书文件 token ↔ 本地路径映射 |

### 2.8 Checkbox / User

| 类型 | 飞书 | Casy |
|------|------|------|
| Checkbox | boolean (true/false) | `tasks.completed` INTEGER (0/1) ✅ 已实现 |
| User | `{id, name, email, avatar}` | `tasks.assignee` TEXT — 仅存名字，缺失 ID 映射 |

---

## 3. 公式引擎设计

### 3.1 架构概览

```
┌─────────────────────────────────────────────────────┐
│                   Formula Engine                      │
│                                                       │
│  ┌───────────┐   ┌──────────┐   ┌──────────────────┐ │
│  │  Parser   │──►│  AST     │──►│  Evaluator       │ │
│  │  (nom)    │   │          │   │  (Rust)          │ │
│  └───────────┘   └──────────┘   └──────────────────┘ │
│                                      │                │
│  ┌───────────────────────────────────┘                │
│  │                                                    │
│  ▼                                                    │
│  ┌──────────────────┐   ┌────────────────────────┐   │
│  │ Function Registry│   │  Cross-Table Resolver   │   │
│  │ IF/AND/OR/NOT    │   │  bitable::$table[...]   │   │
│  │ ISBLANK/TODAY    │   │  → SQL subquery         │   │
│  │ EDATE/WORKDAY    │   └────────────────────────┘   │
│  └──────────────────┘                                 │
└─────────────────────────────────────────────────────┘
```

### 3.2 AST 节点类型

```rust
enum Expr {
    // 字面量
    Literal(Value),          // "string", 123, true, null

    // 当前记录字段引用
    FieldRef { field_id: String },  // 当前表的字段

    // 跨表引用
    CrossTableRef {
        table_id: String,
        field_id: String,
    },  // bitable::$table[xxx].$field[yyy]

    // 函数调用
    Call {
        name: String,        // IF, AND, OR, NOT, ISBLANK, EDATE, WORKDAY, TODAY
        args: Vec<Expr>,
    },

    // 比较运算
    Compare {
        op: CmpOp,           // ==, !=, <, >, <=, >=
        left: Box<Expr>,
        right: Box<Expr>,
    },

    // 逻辑运算
    Logic {
        op: LogicOp,         // &&, ||
        left: Box<Expr>,
        right: Box<Expr>,
    },

    // 字符串拼接
    Concat(Vec<Expr>),       // & 运算符
}

enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    Array(Vec<Value>),
}
```

### 3.3 内置函数注册表

| 函数 | 签名 | 实现方式 |
|------|------|----------|
| `IF(cond, then, else)` | `bool, T, T → T` | Rust `match` |
| `AND(a, b, ...)` | `bool... → bool` | 短路求值 |
| `OR(a, b, ...)` | `bool... → bool` | 短路求值 |
| `NOT(a)` | `bool → bool` | 取反 |
| `ISBLANK(v)` | `T → bool` | `matches!(v, Null \| String("") \| Array([]))` |
| `TODAY()` | `→ Date` | `chrono::Local::now().naive_local().date()` |
| `EDATE(date, n)` | `Date, int → Date` | `add_months_clamp()` — 已在 holidays.rs 实现 |
| `WORKDAY(date, n)` | `Date, int → Date` | 使用 `HolidayCalendar` 前进/后退 n 个工作日 |
| `LISTCOMBINE()` | `Array → Array` | 合并去重 |
| `FILTER(cond)` | `Array, bool → Array` | 过滤 |

### 3.4 跨表引用解析

**飞书公式引用格式**:
```
bitable::$table[tbl4fMNw2UJfXBgy].$field[fld6KMKs7x]
```

**解析策略**:
1. 解析出 `table_id` 和 `field_id`
2. 查 `feishu_field_meta` 表获取 field_name 和所在表
3. 查 `sync_map` 将飞书 field_id 映射到本地表/列
4. 如果引用的是同表字段 → 直接读取当前记录的对应列值
5. 如果引用的是跨表字段 → 通过链接关系查找关联记录

**Lookup 公式解析**:
```
bitable::$table[tblXrb7Y6c9i2o8D]
  .FILTER(CurrentValue.$column[fldXapQ4bm]>TODAY()
    &&CurrentValue.$column[fldqqWhNtP]=bitable::$table[tbl4fMNw2UJfXBgy].$field[fld1LgpSfa])
  .$column[fldydpgUuR].LISTCOMBINE()
```

→ 转换为等价 SQL:
```sql
SELECT hearing_record FROM hearings
WHERE case_id = :current_case_id
  AND hearing_date > date('now')
```

### 3.5 自动重算机制

```rust
/// 依赖图：记录哪些公式字段依赖哪些数据字段
struct DependencyGraph {
    /// field_id → Vec<依赖的 field_id>
    dependents: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    /// 当 field_id 的值变化时，返回需要重算的所有字段（拓扑排序）
    fn get_recalc_order(&self, changed_field: &str) -> Vec<String> {
        // BFS 拓扑排序，防止循环依赖
    }
}
```

**触发时机**:
1. 记录更新后，检查变更字段是否在依赖图中
2. 递归计算所有下游公式字段
3. 结果写入 SQLite 的公式缓存列

### 3.6 公式 → SQLite 映射策略

| 公式复杂度 | SQLite 映射 | 适用场景 |
|-----------|-------------|----------|
| 简单（仅引用同表字段） | **GENERATED COLUMN** 或 VIEW | `案件状态`、`庭审状态` |
| 中等（跨表单值查询） | **SQL Trigger + subquery** | `请求人首次无效时间` |
| 复杂（FILTER + LISTCOMBINE） | **应用层计算 + 缓存列** | Lookup 字段 |

**推荐方案**: 混合策略
- 简单公式 → SQLite `GENERATED ALWAYS AS` 列（SQLite 3.31+）
- 中等公式 → AFTER UPDATE 触发器调用应用层
- 复杂公式 → Rust 公式引擎 + 结果缓存列

---

## 4. 同步架构设计

### 4.1 同步拓扑

```
┌──────────────┐              ┌──────────────┐
│  Casy SQLite │              │  飞书多维表格  │
│              │   Pull API   │              │
│  cases ──────│◄────────────│  案件主表     │
│  case_logs ──│◄────────────│  办案日志     │
│  hearings ───│◄────────────│  庭审信息     │
│  tasks ──────│◄────────────│  任务管理     │
│  officials ──│◄────────────│  官方人员     │
│              │              │              │
│  sync_map    │   Push API   │              │
│  sync_queue │──────────────►│              │
└──────────────┘              └──────────────┘
```

### 4.2 多表同步流程

**当前状态**: 仅同步案件主表 → `cases`

**目标**: 同步全部 5 张表

```rust
pub async fn sync_feishu_full_pull(app_token: &str) -> Result<FullSyncReport> {
    let tables = vec![
        ("tbl4fMNw2UJfXBgy", "cases", sync_case_record),
        ("tblAB7NpSVKXX9uN", "case_logs", sync_log_record),
        ("tblXrb7Y6c9i2o8D", "hearings", sync_hearing_record),
        ("tblVzAWjugfmRqhR", "tasks", sync_task_record),
        ("tblYAqcFHwtehbRP", "officials", sync_official_record),
    ];

    // Phase 1: 同步基础表（无外键依赖）
    sync_table(app_token, "tblYAqcFHwtehbRP", "officials").await?;
    sync_table(app_token, "tbl4fMNw2UJfXBgy", "cases").await?;

    // Phase 2: 同步依赖表（需要先解析链接关系）
    sync_table(app_token, "tblAB7NpSVKXX9uN", "case_logs").await?;
    sync_table(app_token, "tblXrb7Y6c9i2o8D", "hearings").await?;
    sync_table(app_token, "tblVzAWjugfmRqhR", "tasks").await?;

    // Phase 3: 同步链接关系
    sync_link_relationships(app_token).await?;

    // Phase 4: 重算公式字段
    recalculate_formulas().await?;

    Ok(report)
}
```

### 4.3 字段类型转换矩阵

| 飞书类型 | Pull (飞书→SQLite) | Push (SQLite→飞书) |
|----------|-------------------|-------------------|
| Text (1) | `.as_str()` → TEXT | `json!(string)` |
| Formula (20) | **Pull**: 读取计算结果，存缓存列 | **Push**: 不推送（飞书自行计算） |
| SingleSelect (3) | `.as_str()` → TEXT | `json!(string)` |
| MultiSelect (4) | `[arr]` → JSON TEXT | `json!([arr])` |
| DateTime (5) | `ms/1000` → `%Y-%m-%d` TEXT | `date * 1000` → ms |
| Checkbox (7) | `.as_bool()` → INTEGER 0/1 | `json!(bool)` |
| DuplexLink (21) | 解析 `record_ids` → 关联表 INSERT | Push 时通过 link API 维护 |
| SingleLink (18) | 解析 `record_ids` → FK 更新 | Push 时通过 link API |
| Lookup (19) | **不存储**（由公式引擎本地计算） | **不推送** |
| Attachment (17) | 下载文件 → 本地路径 | 上传文件 → 飞书 token |
| Button (3001) | **忽略**（纯 UI 操作） | **忽略** |
| User (11) | `.name` → TEXT | N/A（飞书用户系统） |

### 4.4 Formula 字段同步策略

```
Pull 方向:
  飞书 formula 结果值 → 本地缓存列 (e.g., case_status, deadline_xxx)
  飞书 formula_expression → feishu_field_meta 表存储
  → 本地公式引擎可选择: (a) 使用飞书公式本地重算, 或 (b) 直接使用拉取的值

Push 方向:
  Formula 字段不推送（飞书有自己的公式引擎）
  但 Push 前需确保依赖的输入字段已推送
  → 需要计算推送顺序: 先推输入字段，再拉公式结果
```

### 4.5 Link 字段同步策略

**Pull**:
```rust
fn sync_duplex_link(conn: &Connection, record: &Value, field_name: &str) -> Result<()> {
    let link_data = &record["fields"][field_name];
    if let Some(arr) = link_data.as_array() {
        for link in arr {
            let record_ids = link["record_ids"].as_array();
            let table_id = link["table_id"].as_str();
            if let (Some(ids), Some(tid)) = (record_ids, table_id) {
                for rid in ids {
                    // 查 sync_map 获取本地 ID
                    let local_id = resolve_feishu_record_id(conn, rid.as_str().unwrap())?;
                    // 插入关联
                    insert_link(conn, current_local_id, local_id, tid)?;
                }
            }
        }
    }
}
```

**Push**:
```rust
async fn push_link_field(client: &Client, auth: &mut FeishuAuth, 
                          record_id: &str, field_name: &str, 
                          linked_ids: Vec<String>) -> Result<()> {
    // 使用飞书 API 更新链接字段
    // PUT /bitable/v1/apps/{app}/tables/{table}/records/{record}
    // body: { "fields": { "field_name": [{ "record_id": "xxx" }] } }
}
```

### 4.6 Lookup 字段处理

Lookup 字段 **不直接存储**，而是通过本地公式引擎或 SQL 视图实现:

```sql
-- 示例: 庭审信息中的 "案号" Lookup
CREATE VIEW v_hearing_with_lookup AS
SELECT h.*, c.case_no AS looked_up_case_no,
       c.court AS looked_up_court,
       c.case_level AS looked_up_case_level
FROM hearings h
LEFT JOIN cases c ON h.case_id = c.id;
```

### 4.7 冲突解决策略

```
┌─────────────┐    ┌─────────────┐    ┌─────────────────┐
│ 本地较新     │    │ 远端较新     │    │ 双方都改过       │
│ (local_newer)│    │(remote_newer)│    │   (conflict)     │
├─────────────┤    ├─────────────┤    ├─────────────────┤
│ 直接 Push    │    │ 直接 Pull    │    │ 字段级合并:       │
│              │    │              │    │  - 非重叠字段:合并 │
│              │    │              │    │  - 重叠字段:      │
│              │    │              │    │    飞书优先 (默认)│
│              │    │              │    │    或弹窗让用户选 │
└─────────────┘    └─────────────┘    └─────────────────┘
```

**时间戳比较**:
- `sync_map.local_updated` vs `sync_map.remote_updated`
- 飞书端: `last_modified_time` 字段
- 本地端: `updated_at` 列

**冲突字段检测**:
```rust
struct ConflictInfo {
    field_name: String,
    local_value: String,
    remote_value: String,
    local_updated_at: String,
    remote_updated_at: String,
}
```

---

## 5. 数据模型变更

### 5.1 新增表: `feishu_field_meta`（飞书字段元数据）

```sql
-- 飞书字段元数据存储
CREATE TABLE IF NOT EXISTS feishu_field_meta (
  id              TEXT PRIMARY KEY,
  table_id        TEXT NOT NULL,           -- 飞书 table_id (e.g., tbl4fMNw2UJfXBgy)
  table_name      TEXT NOT NULL,           -- 飞书表名 (e.g., 案件主表)
  field_id        TEXT NOT NULL,           -- 飞书 field_id (e.g., fldIQpzbjd)
  field_name      TEXT NOT NULL,           -- 飞书字段名 (e.g., 案件状态)
  field_type      INTEGER NOT NULL,        -- 飞书类型码 (1=Text, 20=Formula, etc.)
  ui_type         TEXT NOT NULL,           -- 飞书 UI 类型 (Text, Formula, etc.)
  is_primary      INTEGER DEFAULT 0,       -- 是否主键
  formula_expression TEXT,                 -- 公式表达式 (Formula 类型)
  property_json   TEXT,                    -- 完整 property JSON (选项、链接配置等)
  local_table     TEXT,                    -- 映射到的本地表名
  local_column    TEXT,                    -- 映射到的本地列名
  created_at      TEXT DEFAULT (datetime('now','localtime')),
  updated_at      TEXT DEFAULT (datetime('now','localtime')),
  UNIQUE(table_id, field_id)
);

CREATE INDEX IF NOT EXISTS idx_feishu_meta_table ON feishu_field_meta(table_id);
CREATE INDEX IF NOT EXISTS idx_feishu_meta_type ON feishu_field_meta(field_type);
```

### 5.2 新增表: `feishu_record_map`（飞书记录 ID 映射）

当前 `sync_map` 表已有此功能，但需扩展支持多表:

```sql
-- 扩展 sync_map 支持所有表（当前仅 cases）
-- 无需新建表，sync_map 已有 local_table 列
-- 但需确保所有 5 张表都使用 sync_map

-- 新增: 飞书链接关系缓存
CREATE TABLE IF NOT EXISTS feishu_link_cache (
  id              TEXT PRIMARY KEY,
  source_table    TEXT NOT NULL,            -- 源飞书表 ID
  source_field    TEXT NOT NULL,            -- 源飞书字段 ID
  source_record   TEXT NOT NULL,            -- 源飞书记录 ID
  target_table    TEXT NOT NULL,            -- 目标飞书表 ID
  target_record   TEXT NOT NULL,            -- 目标飞书记录 ID
  link_type       TEXT NOT NULL DEFAULT 'duplex'
                  CHECK(link_type IN ('duplex','single')),
  synced_at       TEXT DEFAULT (datetime('now','localtime')),
  UNIQUE(source_table, source_field, source_record, target_record)
);
```

### 5.3 新增列: `formula_cache`（公式结果缓存）

在 `cases` 表新增公式缓存列:

```sql
-- cases 表新增公式缓存列
ALTER TABLE cases ADD COLUMN formula_case_status TEXT;       -- 案件状态（公式）
ALTER TABLE cases ADD COLUMN formula_defense_deadline TEXT;  -- 提交答辩状期间
ALTER TABLE cases ADD COLUMN formula_estimated_trial_limit TEXT; -- 预估审限
ALTER TABLE cases ADD COLUMN formula_petitioner_first TEXT;  -- 请求人首次无效时间
ALTER TABLE cases ADD COLUMN formula_petitioner_supp TEXT;   -- 请求人补充意见期限
ALTER TABLE cases ADD COLUMN formula_petitioner_reply TEXT;  -- 请求人答复意见期限
ALTER TABLE cases ADD COLUMN formula_patentee_statement TEXT; -- 专利权人陈述意见期限
ALTER TABLE cases ADD COLUMN formula_patentee_supp TEXT;     -- 专利权人补充意见时间

-- hearings 表新增公式缓存列
ALTER TABLE hearings ADD COLUMN formula_status TEXT;         -- 状态（公式）

-- tasks 表新增公式缓存列
ALTER TABLE tasks ADD COLUMN formula_days_until_deadline TEXT; -- 距离截止日
```

### 5.4 扩展 `sync_map` 支持多表同步

当前 `sync_map` 已有 `local_table` 列，但 `sync_feishu_pull_inner` 仅同步 `cases`。需扩展:

```sql
-- 确保 sync_map 支持所有表类型
-- 当前: local_table 仅用 'cases'
-- 目标: 'cases', 'case_logs', 'hearings', 'tasks', 'officials'

-- 同步队列扩展（已有表，确保使用）
-- sync_queue 已支持 local_table 列
```

### 5.5 新增列: 链接关系存储

```sql
-- cases 表: 存储关联案件 ID（SingleLink 自引用）
ALTER TABLE cases ADD COLUMN related_case_ids TEXT;  -- JSON array of case IDs

-- case_logs / hearings: 已有 case_id FK（DuplexLink 的反向实现）
-- officials ↔ cases: 已有 case_officials 关联表
```

### 5.6 完整迁移 SQL (v2)

```sql
-- Migration v2: 飞书同步增强

-- 1. 飞书字段元数据表
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

-- 2. 飞书链接缓存
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

-- 3. cases 表公式缓存列
ALTER TABLE cases ADD COLUMN formula_case_status TEXT;
ALTER TABLE cases ADD COLUMN formula_defense_deadline TEXT;
ALTER TABLE cases ADD COLUMN formula_estimated_trial_limit TEXT;
ALTER TABLE cases ADD COLUMN formula_petitioner_first TEXT;
ALTER TABLE cases ADD COLUMN formula_petitioner_supp TEXT;
ALTER TABLE cases ADD COLUMN formula_petitioner_reply TEXT;
ALTER TABLE cases ADD COLUMN formula_patentee_statement TEXT;
ALTER TABLE cases ADD COLUMN formula_patentee_supp TEXT;

-- 4. hearings 表公式缓存列
ALTER TABLE hearings ADD COLUMN formula_status TEXT;

-- 5. tasks 表公式缓存列
ALTER TABLE tasks ADD COLUMN formula_days_until_deadline TEXT;

-- 6. 关联案件 ID 存储
ALTER TABLE cases ADD COLUMN related_case_ids TEXT;

-- 7. 飞书配置表扩展
CREATE TABLE IF NOT EXISTS feishu_base_config (
  id              TEXT PRIMARY KEY,
  app_token       TEXT NOT NULL,
  base_name       TEXT,
  table_mappings  TEXT NOT NULL,  -- JSON: {feishu_table_id: local_table_name}
  sync_direction  TEXT DEFAULT 'bidirectional'
                  CHECK(sync_direction IN ('pull_only','push_only','bidirectional')),
  last_full_sync  TEXT,
  created_at      TEXT DEFAULT (datetime('now','localtime'))
);
```

---

## 6. 实施计划

### Phase 1: 基础设施 (1-2 周)

| 任务 | 优先级 | 工作量 | 描述 |
|------|--------|--------|------|
| 1.1 | P0 | 2d | 数据库迁移 v2: 创建 `feishu_field_meta`、`feishu_link_cache`、`feishu_base_config` 表 |
| 1.2 | P0 | 1d | cases/hearings/tasks 表新增公式缓存列 |
| 1.3 | P0 | 3d | 多表同步: 扩展 `sync_feishu_pull_inner` 支持全部 5 张表 |
| 1.4 | P1 | 2d | 字段元数据导入: 从 full-schema.json 导入所有字段定义到 `feishu_field_meta` |
| 1.5 | P1 | 2d | 同步映射表扩展: 确保 sync_map 正确支持 cases/case_logs/hearings/tasks/officials |

### Phase 2: 公式引擎 (2-3 周)

| 任务 | 优先级 | 工作量 | 描述 |
|------|--------|--------|------|
| 2.1 | P0 | 3d | 公式解析器: 使用 `nom` 实现 Feishu 公式语法解析 → AST |
| 2.2 | P0 | 2d | 内置函数: 实现 IF/AND/OR/NOT/ISBLANK/TODAY/EDATE/WORKDAY |
| 2.3 | P1 | 2d | 字段引用解析: 当前记录字段值查找 |
| 2.4 | P1 | 3d | 跨表引用: `bitable::$table[xxx].$field[yyy]` → SQL 查询 |
| 2.5 | P2 | 3d | Lookup 引擎: FILTER + LISTCOMBINE 转换为 SQL |
| 2.6 | P1 | 2d | 依赖图: 字段依赖关系追踪 + 拓扑排序重算 |
| 2.7 | P2 | 2d | 公式缓存: 计算结果写入 formula_ 缓存列 |

### Phase 3: 链接关系同步 (1-2 周)

| 任务 | 优先级 | 工作量 | 描述 |
|------|--------|--------|------|
| 3.1 | P0 | 2d | DuplexLink Pull: 解析飞书链接数据，建立本地关联 |
| 3.2 | P1 | 2d | DuplexLink Push: 本地关联变更 → 飞书链接 API 更新 |
| 3.3 | P1 | 1d | SingleLink: 关联案件、关联项目的同步 |
| 3.4 | P2 | 2d | 链接完整性检查: 定期校验双向链接一致性 |

### Phase 4: 选项与附件 (1 周)

| 任务 | 优先级 | 工作量 | 描述 |
|------|--------|--------|------|
| 4.1 | P2 | 2d | 选项同步: SingleSelect/MultiSelect 选项定义 + 颜色存储 |
| 4.2 | P2 | 2d | 附件下载: 飞书附件 → 本地 case_files 表 |
| 4.3 | P3 | 1d | 附件上传: 本地文件 → 飞书附件字段 |

### Phase 5: Push 增强与冲突处理 (1-2 周)

| 任务 | 优先级 | 工作量 | 描述 |
|------|--------|--------|------|
| 5.1 | P1 | 3d | 多表 Push: 所有 5 张表的本地变更 → 飞书 |
| 5.2 | P1 | 2d | 推送顺序: 先推基础表，再推依赖表，最后拉公式结果 |
| 5.3 | P2 | 2d | 冲突检测: 字粒度冲突识别 |
| 5.4 | P2 | 2d | 冲突解决 UI: 弹窗让用户选择冲突字段的取值 |

### Phase 6: 测试与优化 (1 周)

| 任务 | 优先级 | 工作量 | 描述 |
|------|--------|--------|------|
| 6.1 | P0 | 2d | 全量同步测试: 使用 archive/feishu-base/ 数据验证 |
| 6.2 | P1 | 2d | 公式引擎测试: 所有 10 个公式表达式的单元测试 |
| 6.3 | P2 | 1d | 性能优化: 批量 API 调用、缓存策略 |

### 工作量总计

| Phase | 工作量 | 累计 |
|-------|--------|------|
| Phase 1: 基础设施 | 10d | 10d |
| Phase 2: 公式引擎 | 17d | 27d |
| Phase 3: 链接关系 | 7d | 34d |
| Phase 4: 选项与附件 | 5d | 39d |
| Phase 5: Push 增强 | 9d | 48d |
| Phase 6: 测试优化 | 5d | 53d |
| **总计** | **~11 周** | |

### 依赖关系

```
Phase 1 (基础设施)
    │
    ├──► Phase 2 (公式引擎) ──► Phase 6 (测试)
    │
    ├──► Phase 3 (链接关系) ──► Phase 5 (Push 增强)
    │
    └──► Phase 4 (选项与附件)
```

### 关键技术决策

1. **公式引擎**: 使用 `nom` 解析器组合子库（已在 Rust 生态成熟），不引入 DSL
2. **跨表引用**: 优先使用 SQL 视图/触发器，复杂场景用 Rust 引擎 + 缓存
3. **同步方向**: 默认飞书优先（pull），本地编辑后自动 push
4. **Lookup 字段**: 不存储，通过 SQL VIEW 实时查询（性能足够，数据一致性好）
5. **Button 字段**: 不同步，仅作为 UI 快捷操作在 Casy 本地实现
6. **User 字段**: 仅存储姓名字符串，不同步飞书用户系统

---

## 附录: 当前 Casy 已有能力清单

| 能力 | 状态 | 位置 |
|------|------|------|
| SQLite 完整 schema | ✅ 已实现 | `db/schema.rs` |
| 案件 CRUD | ✅ 已实现 | `db/cases.rs` |
| 飞书 Pull（案件主表） | ✅ 已实现 | `sync/feishu.rs:sync_feishu_pull_inner` |
| 飞书 Push（案件主表） | ✅ 已实现 | `sync/feishu.rs:sync_feishu_push_inner` |
| JSON Dump 导入（全部 5 表） | ✅ 已实现 | `commands/import_feishu.rs` |
| Token 管理 + 限流 | ✅ 已实现 | `sync/feishu.rs:FeishuAuth/RateLimiter` |
| 自动推送（5s 防抖） | ✅ 已实现 | `sync/feishu.rs:AutoPushManager` |
| case_status 自动计算 | ✅ 已实现 | SQL trigger `trg_cases_status_*` |
| 期限规则引擎 | ✅ 已实现 | `formula/engine.rs:DeadlineEngine` |
| 节假日日历 | ✅ 已实现 | `formula/holidays.rs:HolidayCalendar` |
| sync_map 映射表 | ✅ 已实现 | `db/schema.rs` |
| sync_queue 队列 | ✅ 已实现 | `db/schema.rs` |
| 多表 Pull | ❌ 未实现 | — |
| 多表 Push | ❌ 未实现 | — |
| 通用公式引擎 | ❌ 未实现 | — |
| Lookup 字段 | ❌ 未实现 | — |
| DuplexLink 同步 | ❌ 未实现 | — |
| 附件同步 | ❌ 未实现 | — |
| 飞书字段元数据 | ❌ 未实现 | — |
| 选项颜色管理 | ❌ 未实现 | — |
| 冲突解决 UI | ❌ 未实现 | — |
