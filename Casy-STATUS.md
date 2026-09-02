# Casy 项目状态

> **记录时间**: 2026-09-01 · **状态**: 🚀 **RC（发布候选）· 待 Dogfooding**
> **规划**: `docs/refactoring-plan.md` v1.7 · **上位哲学**: casy-design-philosophy.md v2.3
> **基线**: main 工作区干净 · cargo test 119+7 ✓ · vue-tsc 0 ✓ · build ✓
> **最新批次**: 全球对标灵感落地（2026-09-01）—— Schema v20 · 10 项灵感全量实装 · bindings 115 类型 · CommandMap +45 契约
> 详见 `docs/global-benchmark-inspirations-plan-2026-09-01.md` 与 `docs/global-benchmark-inspirations-walkthrough-2026-09-01.md`

---

## 一、里程碑全景（自 2026-08-21 接手以来）

| 里程碑 | 状态 | 关键产出 |
|---|---|---|
| B0 仓库止血 | ✅ | rebase 事故恢复；门禁体系建立 |
| M-GTD-1 不打断 | ✅ | 一键完成/乐观更新/⌘Z/系统通知/parseWhen |
| M-GTD-2 组织得起来 | ✅ | A1-1~A1-7 全量：项目建模(绞杀式阶段一)/Areas/拖拽排序/子任务/重复任务/⌘K/Review 修复 |
| M-CAL-1 日历完整化 | ✅ | D-7 calendar_events 实体；年/月/周/日四视图；NL 直建日程；双向拖拽改期 |
| A-UI 动效地基+优化轮 | ✅ | Motion Tokens；六表面自绘；令牌化 514 处；空状态/对话框节奏统一；KeyboardCenter |
| B3/B1 内核收口 | ✅ | 确认策略上收；类型双轨合一；事件层激活；bindings 83 类型；错误码全域 |
| B4 生产化 | 🔶 | R-1 身份/updater ✅ · R-3 CI ✅ · R-4 CSP/审计 ✅ · R-5 备份恢复 ✅ · R-6 崩溃日志 ✅ · R-7 合规文本 ✅ · R-2 签名 ⏸️ 待证书 |
| 对标灵感落地（豁免冻结） | ✅ | AI Proposal Diff+@引用 · Defer Date · 通知中心 · 期限规则自定义+留痕 · 双链 · OCR+SmartRules · persons 对象 · 事实白板 |

## 二、当前形态

- **架构**：cordis 式内核（Context/Service/Fiber）+ 单一数据通路 + AI 无特权通道（executeTool 强制确认 + audit_events 归因）
- **Schema v20**：tasks(+parent/recurrence/defer_until) / projects + case_legal_details / calendar_events / notifications / links / persons+case_persons / smart_rules / whiteboards+fact_nodes / deadline_rule_audit
- **类型安全**：specta 绑定 58 类型 + commandMap 契约注册表 + tauriCallSafe 双重载；动态命令余量 ~86（杂项盘点 63 个已分桶：可定型31/三态2/键袋18/核验2，随 DomainCommand 化消化）
- **CI**：.github/workflows/ci.yml 三平台矩阵 + bindings 漂移校验 + tag 发布草稿

## 三、已知问题 / 待办

### 边界项（需用户）
- R-2 证书采购（Apple $99/年 + Windows Authenticode）→ scripts/signing/ 已就绪
- Dogfooding 第一周 → docs/dogfooding-round1.md 清单就绪

### 技术备忘
- quick-xml 升级需跨 minor 适配 calamine/docsy_engine（本地输入面，接受风险，见 security-audit-r4.md）
- create/update_case 三态语义待 B1 DomainCommand 设计时定型（现保持 Record 有决策记录）
- CalendarView forecast 的任务投影依赖 get_calendar_events 月度窗口，年视图已单独处理
- docsy_engine 改名建议挂 C 线解冻首批

## 四、验证方式

```bash
npm run test     # vue-tsc + cargo test (104)
npm run build    # 含类型门禁
.github/workflows/ci.yml   # push 自动三平台矩阵 + bindings 漂移校验
```
