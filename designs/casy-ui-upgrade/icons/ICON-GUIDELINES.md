# Casy Judicial Docket 图标系统

> 完整一致的法务线性图标语言。与 UI 升级（卷宗墨卷 · 现代化配色）同一套骨架。

## 设计锚点

- **产品**：Casy — 专利律师全流程秩序引擎
- **系统名**：Judicial Docket（卷宗墨卷）
- **性格**：卷宗秩序、法庭文书的克制精确；现代线性，而非复古雕版
- **形态**：线性描边、几何法务隐喻、统一光学重量

## 构造规则

| 项 | 规范 |
|----|------|
| 画板 | `viewBox="0 0 24 24"` |
| 安全区 | 2–22（20px 视觉盒） |
| 描边 | `stroke-width="1.5"` |
| 端点 | `stroke-linecap="round"` `stroke-linejoin="round"` |
| 填充 | `fill="none"`（默认） |
| 颜色 | `stroke="currentColor"`，跟随文字色 / 语义色 |
| 小尺寸 | 16px 时描边可增至约 2.25 视觉等效（预览已标） |
| 禁止 | emoji、多套图标混用、不同描边混重、写实隐喻、多余装饰 |

## 隐喻约定

- **今日**：日轮 + 案卷横线（今日卷）
- **收件箱**：托盘（大口袋，先丢后厘清）
- **案件**：律师箱（统一容器）
- **卷宗**：双层卷宗夹（12 阶段归档）
- **等待**：沙漏（等待中，不是延期）
- **风险**：三角警示（R1–R4 / 逾期）
- **智伴**：星芒节点（主动智伴）
- **品牌印章**：双环法务印（可用作 App 图标 / 角标）

## 语义色（与主题一致）

| Token | Hex | 用途 |
|-------|-----|------|
| ink | `#0C1526` | 默认图标 |
| accent | `#1A4FD6` | 选中 / 主操作 |
| risk | `#D64545` | 逾期 / R3–R4 |
| warning | `#C4890A` | 将到期 / R2 |
| waiting | `#2F7A8C` | 等待中 |
| secondary | `#5B6B7F` | 弱化图标 |

## 交付文件

| 文件 | 说明 |
|------|------|
| `svg/*.svg` | 单图标源文件（31 枚） |
| `casy-icons.sprite.svg` | `<symbol>` 雪碧图 |
| `preview.html` | 规范预览页（浏览器打开） |
| `build_icons.py` | 生成脚本（改几何请改这里再重生成） |

## 在 Vue / Element Plus 中使用

内联 SVG（推荐，随文字色着色）：

```html
<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor"
     stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <!-- 将 svg/*.svg 中的 path/circle/rect 直接粘贴于此 -->
</svg>
```

或使用雪碧图：

```html
<svg width="20" height="20"><use href="/icons/casy-icons.sprite.svg#i-today"></use></svg>
```

## 图标清单

| id | 中文 | 说明 |
|----|------|------|
| `today` | 今日 | 日晷式今日：天穹圆弧 + 案卷横线 |
| `inbox` | 收件箱 | 托盘收件：大口袋 |
| `cases` | 案件 | 律师箱：案件容器 |
| `projects` | 项目 | 叠层项目板 |
| `tasks` | 任务 | 勾选清单 |
| `calendar` | 日历 | 装订日历页 |
| `knowledge` | 知识库 | 开卷 + 知识节点 |
| `drafting` | 文书 | 钢笔尖：文书工坊 |
| `dossier` | 卷宗 | 双层卷宗夹 |
| `dashboard` | 看板 | 四格数据看板 |
| `search` | 搜索 | 全局检索 |
| `capture` | 捕获 | 取景框捕获 |
| `reminder` | 提醒 | 分级预警铃 |
| `settings` | 设置 | 机械齿轮 |
| `waiting` | 等待 | 沙漏：等待中 |
| `risk` | 风险 | 期限风险 / 逾期 |
| `link` | 关联 | 双向链接 |
| `file` | 文件 | 文书页 |
| `sync` | 同步 | 双向同步 |
| `filter` | 筛选 | 漏斗筛选 |
| `check` | 完成 | 完成勾 |
| `clock` | 时间 | 时点 / 双轨时间 |
| `ai` | 智伴 | 主动智伴 AI |
| `seal` | 品牌印章 | Casy 法务印章标 |
| `relation` | 关系网 | 案件关系网络 |
| `brief` | 早报 | 今日早报 |
| `star` | 重点 | 今日重点 ★ |
| `plus` | 新建 | 新建 |
| `chevron-right` | 展开 | 侧栏/行展开 |
| `archive` | 归档 | 归档入库 |
| `ocr` | OCR | 页级识别 |

## 质量自检

- [x] 全部 24 网格对齐，安全区 2–22
- [x] 描边 1.5 / round / currentColor 一致
- [x] 无填充混用、无 emoji
- [x] 同类隐喻同构（卷宗线、三角风险、沙漏等待）
- [x] 16 / 20 / 24 / 32 尺寸试作通过
- [x] 墨底反白可用
