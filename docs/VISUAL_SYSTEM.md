# Judicial Docket（卷宗墨卷）视觉系统

版本：0.1.3。原稿位于 `designs/casy-ui-upgrade`，保留图标规范、SVG、品牌标、预览页与界面原型，设计文件不进入应用 runtime。

## 资产与生成

- 32 枚原始业务图标：24 × 24 网格、20 安全区、1.5 描边、round 端点、currentColor。
- 52 枚基础操作扩展：位于 `src/shared/icons/geometry.json`，遵循相同几何规范；共 84 个产品图形。
- `aliases.json` 将既有组件名映射到统一图形，`index.ts` 是生成的 Vue 组件。116 个兼容命名不等于 116 个独立图形。
- 品牌为双环法务印与卷线；窗口侧栏使用矢量印章，macOS 应用图标由同一品牌 SVG 生成。

```bash
python3 scripts/generate-icons.py
npm run tauri -- icon designs/casy-ui-upgrade/icons/brand/app-icon-512.svg --output src-tauri/icons
```

修改源 SVG 或 geometry 后重新生成，不手改生成物。Element Plus 内部控件保留其内置图标；应用自行引入的图标统一从 `shared/icons` 使用。

## 色彩与布局

`src/assets/docket.css` 在公共样式后加载。明亮主题以冷灰底、白色工作面、墨色正文、蓝色焦点组成；墨色主题提高正文和语义色对比。红色代表风险，琥珀代表提醒，青色代表等待，绿色代表完成。危险、等待、卷宗分别使用三角、沙漏、夹层隐喻。

今日、案件、任务、日历、卷宗、知识库、收件箱沿用真实数据与原有业务组件，按原型整理侧栏、搜索栏、面板、分隔线、留白和字号。未照搬原型中的示例数字、虚构进度或生成图水印。今日日期使用系统宋体，正文使用系统无衬线字体。支持石墨蓝、暖砂大地、日光浅色、日光深色、暗夜深色五个原有主题标识；卷宗明亮、卷宗墨色使用独立标识，另新增黄宣纸。跟随系统继续在石墨蓝与暗夜深色之间切换。

卷宗增加独立导航入口和案件选择器；未选案件时禁用上传、检索等依赖案件的动作。已有任务编辑、案件详情、日历排期、知识编辑和收件澄清保留实际服务接线。

## 验证范围

浏览器 mock 用于检查布局、导航和表单入口；不作为持久化验收。检查默认 1024 × 720、最小 800 × 600 的布局及明暗主题，原生功能在独立资料库验证。具体结果见 `RELEASE_0.1.3.md`。

## 2026-09-23 主题回归修订

首包中 docket.css 的全局 `:root` 配色覆盖了既有主题，且复用了 `slate` / `dark` 标识；此前“保留其他主题”的描述未经全量切换验证。修订后卷宗配色只匹配 `docket-light` / `docket-dark`，不改写旧设置含义。通用几何样式仍共享。

黄宣纸（`rice-paper`）使用淡米色 `#F3EAD7`、纸面 `#FCF6E8` 和朱砂重点色，叠加 `rice-paper-grain.svg` 的固定细纤维噪声纹理。纹理仅作用于应用工作面和面板，不叠在文本上，不改变导出文档。日光浅色按用户反馈改为暖白 `#FAF7ED`、纸面 `#FFFDF7`，使用蓝色重点且不加纹理。

浏览器逐一点击八种显式主题，核对根节点、工作面颜色、深浅色控件模式和纸纹只在黄宣纸启用；CSS 级联回归测试覆盖全部配色。截图及实际计算颜色记录位于 `outputs/release-0.1.3-themes-webdav/ui/`。

## 日历

视图顺序为日、周、月、年、时间线、排期。年度采用 12 个月历缩略图，日任务数五档（0 / 1 / 2–3 / 4–6 / 7+），月份背景按平均日密度轻微着色，均使用当前主题主色。法定休/班使用实色徽标，个人自休/自班使用虚线框；同一天两类均保留，不能只靠颜色区分。

排期内部提供负荷列表与横向甘特切换。甘特使用主题主色计划条、休息日浅底纹、法定实色/个人虚线标记和截止菱形；任务名称列固定，日期区域内部横向滚动。按日 4 周/按周 12 周，修改经确认后保存，窄幅减少标签列宽。截图与交互验证见[甘特归档](Archive/2026-09-23/gantt-planning/README.md)。

个人休息表单提供全天、上午、下午、自定义时段。部分休息紧凑徽标为「时休」，完整时间通过非紧凑标签、提示和详情呈现；日/周时间轴按起止显示主题色斜纹，甘特仅提示该日部分时间不可用。任务及案件列表的计划摘要明确与截止日期区分。

## Desktop workspace layout contract (2026-10-02)

- Preserve the current theme tokens, docket surfaces, icon family and Element Plus.
- Compact: below 1100px, reduce secondary panels and allow toolbars to wrap.
- Narrow: below 900px, ensure usability at the native 800x600 minimum; primary
  actions and error/retry controls must remain accessible without horizontal scroll.
- Wide: 1360px and above, optional secondary panes may expand.
- Media queries use literal widths: CSS custom properties cannot be used directly
  as media-query breakpoints. Smaller browser/container breakpoints remain where
  their actual embedded surface requires them; do not mass-delete them.
- New list pages should compose UiDataState. A failed request is never empty data;
  refresh failures retain previous content with a non-dismissible error/retry notice.
- Shared UI primitives live in src/shared/ui and must be adopted by views, not
  merely added to a component inventory. Current adopters: persons, inbox, projects.
- Skeletons use semantic theme colors and honor prefers-reduced-motion.
- Screenshot acceptance and live database/IPC acceptance are separate evidence.
