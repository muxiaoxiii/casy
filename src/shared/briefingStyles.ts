export type BriefingKind = 'daily' | 'weekly'
export type BriefingSealTone = 'none' | 'oxblood' | 'gold'
export type BriefingTone = 'light' | 'paper' | 'dark' | 'vivid'

export interface BriefingStyleOption {
  id: string
  kind: BriefingKind
  name: string
  label: string
  tagline: string
  tags: string[]
  desc: string
  family: string
  seal: BriefingSealTone
  tone: BriefingTone
  themeColor: string
}

export const briefingStyleOptions: BriefingStyleOption[] = [
  { id: 'gazette', kind: 'daily', name: 'Casy Dispatch', label: '法务晨刊', tagline: '经典报刊 · 克制朱红', tags: ['Editorial', 'Serif', 'Paper'], desc: '以报刊双线、社论式留白与清晰分栏组织当天的焦点、期限和排期。', family: 'editorial', seal: 'none', tone: 'paper', themeColor: '#82252a' },
  { id: 'typewriter', kind: 'daily', name: 'Counsel Memo', label: '办案备忘', tagline: '等宽公文 · 黑白高对比', tags: ['Monospace', 'Memo', 'Print'], desc: '机械打字质感与规则基线，适合追求极简、严谨和可打印性的办案简报。', family: 'document', seal: 'none', tone: 'light', themeColor: '#151515' },
  { id: 'swiss-grid', kind: 'daily', name: 'Swiss Brief', label: '网格晨报', tagline: '国际主义网格 · 数字优先', tags: ['Grid', 'Sans', 'Metrics'], desc: '用严格网格、大号标题和红黑对比突出关键指标，适合高信息密度晨读。', family: 'grid', seal: 'none', tone: 'light', themeColor: '#c62f24' },
  { id: 'vogue', kind: 'daily', name: 'Editorial No. 1', label: '编辑精选', tagline: '杂志封面 · 优雅留白', tags: ['Editorial', 'Serif', 'Whitespace'], desc: '居中封面式标题与高比例留白，让日常办案信息呈现为精致的编辑成品。', family: 'editorial', seal: 'none', tone: 'light', themeColor: '#111111' },
  { id: 'narrative-air', kind: 'daily', name: 'Quiet Brief', label: '清风晨读', tagline: '低认知负担 · 柔和纸面', tags: ['Calm', 'Prose', 'Soft'], desc: '柔和青绿色、舒展行距和轻量信息块，适合不希望被视觉噪音打断的晨读。', family: 'atmospheric', seal: 'none', tone: 'light', themeColor: '#318875' },
  { id: 'glassmorphism', kind: 'daily', name: 'Midnight Glass', label: '夜光简报', tagline: '深夜玻璃 · 冷静青蓝', tags: ['Dark', 'Glass', 'Cyan'], desc: '深蓝底色与低透明面板构成安静的夜间简报，保留足够文字对比度。', family: 'atmospheric', seal: 'none', tone: 'dark', themeColor: '#75e6da' },
  { id: 'action-board', kind: 'daily', name: 'Action Sheet', label: '行动清单', tagline: '便签纸感 · 行动优先', tags: ['Action', 'Notes', 'Warm'], desc: '用纸张叠放和重点便签的触感突出首要行动，同时保留完整事实结构。', family: 'action', seal: 'none', tone: 'paper', themeColor: '#be6b17' },
  { id: 'executive', kind: 'daily', name: 'Executive Memo', label: '合伙人简报', tagline: '海军蓝 · 香槟金', tags: ['Executive', 'Gold', 'Memo'], desc: '海军蓝文字、细金边和稳健比例，面向更正式的管理层晨间简报。', family: 'executive', seal: 'gold', tone: 'paper', themeColor: '#a97728' },
  { id: 'magic-prophet', kind: 'daily', name: 'The Daily Prophet', label: '预言家日报', tagline: '猫头鹰邮递 · 魔法号外', tags: ['Magic', 'Engraving', 'Gazette'], desc: '猫头鹰铜版画、古典大报头、羊皮纸与酒红火漆。平凡的一天，也值得登上魔法世界的头版。', family: 'heritage', seal: 'oxblood', tone: 'paper', themeColor: '#6f271f' },
  { id: 'receipt', kind: 'daily', name: 'Little Things Store', label: '今日小票', tagline: '热敏纸 · 日常值得结账', tags: ['Receipt', 'Mono', 'Keepsake'], desc: '窄幅热敏纸、虚线明细与收藏条码，把今天的认真打印成一张值得留下的小票，不虚构金额或成绩。', family: 'receipt', seal: 'none', tone: 'light', themeColor: '#45483f' },
  { id: 'herbarium', kind: 'daily', name: 'A Day in Bloom', label: '植物标本笺', tagline: '鼠尾草绿 · 慢慢生长', tags: ['Botanical', 'Linen', 'Quiet'], desc: '植物线描、亚麻纸与标本标签，让任务像叶片一样被轻轻收好。生长不必急于被看见。', family: 'botanical', seal: 'none', tone: 'paper', themeColor: '#49634a' },
  { id: 'lunar-log', kind: 'weekly', name: 'Letters from the Moon', label: '月球观测日志', tagline: '月面蚀刻 · 深蓝银墨', tags: ['Moon', 'Observatory', 'Night'], desc: '月面插画、轨道细线与观测站编号，像在安静的月球基地回望一周。给忙碌留一点宇宙的距离。', family: 'observatory', seal: 'none', tone: 'dark', themeColor: '#b6cfdf' },
  { id: 'airmail', kind: 'weekly', name: 'Postcards to Myself', label: '远方来信', tagline: '航空信封 · 寄给自己', tags: ['Airmail', 'Postmark', 'Letter'], desc: '红蓝航空信封边、山野邮票与邮戳，把一周折成寄给自己的信。辛苦了，下一站慢慢来。', family: 'postal', seal: 'none', tone: 'paper', themeColor: '#a64c3d' },
  { id: 'telegraph', kind: 'daily', name: 'Priority Wire', label: '优先电报', tagline: '旧式电报 · 紧凑等宽', tags: ['Telegram', 'Mono', 'Urgent'], desc: '黄褐电报纸与等宽排版带来明确的时效感，但不制造虚假紧急信号。', family: 'document', seal: 'none', tone: 'paper', themeColor: '#a03e28' },
  { id: 'bulletin', kind: 'daily', name: 'Case Bulletin', label: '案件通报', tagline: '档案布告 · 粗重边框', tags: ['Bulletin', 'Archive', 'Slab'], desc: '粗边框、居中标题与旧纸材质，呈现正式案件通报而不是娱乐化通缉令。', family: 'heritage', seal: 'oxblood', tone: 'paper', themeColor: '#8d281f' },
  { id: 'blueprint', kind: 'daily', name: 'Case Blueprint', label: '案件蓝图', tagline: '工程蓝图 · 青白网格', tags: ['Blueprint', 'Grid', 'Technical'], desc: '深蓝制图底、细密网格与青白线条，适合强调结构和执行路径的简报。', family: 'technical', seal: 'none', tone: 'dark', themeColor: '#7fe9f0' },
  { id: 'terminal', kind: 'daily', name: 'System Brief', label: '系统晨报', tagline: '复古终端 · 荧光绿字', tags: ['Terminal', 'Mono', 'Dark'], desc: '终端式等宽字体与扫描线底纹，视觉鲜明但保持静态导出的清晰度。', family: 'technical', seal: 'none', tone: 'dark', themeColor: '#d7ff5b' },
  { id: 'polaroid', kind: 'daily', name: 'Field Notes', label: '现场札记', tagline: '拍立得构图 · 现场记录', tags: ['Photo', 'Notes', 'Collectible'], desc: '以拍立得相纸和现场色块构成封面，适合更具个人情绪价值的晨报。', family: 'collectible', seal: 'none', tone: 'light', themeColor: '#466985' },
  { id: 'ticket', kind: 'daily', name: 'Agenda Pass', label: '议程票据', tagline: '票根结构 · 酒红编号', tags: ['Ticket', 'Perforation', 'Agenda'], desc: '撕线、票据编号和酒红配色带来收藏感，同时完整呈现日程与期限。', family: 'collectible', seal: 'none', tone: 'paper', themeColor: '#9c2348' },
  { id: 'scroll', kind: 'daily', name: 'Ink Review', label: '墨卷晨报', tagline: '宣纸纤维 · 朱砂印色', tags: ['Ink', 'Scroll', 'Wax'], desc: '东方书卷比例、宣纸纤维与朱砂色印章，克制地加入仪式感。', family: 'heritage', seal: 'oxblood', tone: 'paper', themeColor: '#a12c27' },
  { id: 'dossier', kind: 'weekly', name: 'Executive Dossier', label: '执行卷宗', tagline: '牛皮卷宗 · 深红归档', tags: ['Dossier', 'Archive', 'Wax'], desc: '牛皮纸、归档边框和火漆章组合为稳重的周度卷宗，适合正式复盘。', family: 'document', seal: 'oxblood', tone: 'paper', themeColor: '#8f252b' },
  { id: 'analytics', kind: 'weekly', name: 'Signal Review', label: '数据复盘', tagline: '深色分析 · 冷静蓝光', tags: ['Dark', 'Metrics', 'Signal'], desc: '深色分析底与冷蓝指标构成专业数据复盘，不使用虚构比例或战力指数。', family: 'technical', seal: 'none', tone: 'dark', themeColor: '#55b6ff' },
  { id: 'ledger', kind: 'weekly', name: 'Black Ledger', label: '黑金台账', tagline: '黑金账册 · 高对比', tags: ['Ledger', 'Gold', 'Dark'], desc: '哑黑纸面与细金线构成正式台账，适合重点案件与管理指标汇总。', family: 'executive', seal: 'gold', tone: 'dark', themeColor: '#d4a84e' },
  { id: 'milestones', kind: 'weekly', name: 'Milestone Review', label: '里程碑周报', tagline: '柔粉流线 · 轻量复盘', tags: ['Milestone', 'Soft', 'Flow'], desc: '柔粉与浅青的低对比流线，为周度节点复盘提供更轻盈的情绪价值。', family: 'atmospheric', seal: 'none', tone: 'light', themeColor: '#b94080' },
  { id: 'partner-brief', kind: 'weekly', name: 'Partner Letter', label: '合伙人复盘', tagline: '正式信函 · 暖棕墨色', tags: ['Letter', 'Partner', 'Serif'], desc: '暖棕纸面与书信式衬线排版，适合严肃但不冰冷的合伙人复盘。', family: 'executive', seal: 'gold', tone: 'paper', themeColor: '#8e6335' },
  { id: 'focus-matrix', kind: 'weekly', name: 'Focus Matrix', label: '焦点矩阵', tagline: '粗野网格 · 高饱和对比', tags: ['Grid', 'Brutalist', 'Bold'], desc: '粗黑边框、黄绿底和粉色强调形成强视觉矩阵，事实结构仍保持一致。', family: 'grid', seal: 'none', tone: 'vivid', themeColor: '#ed3e98' },
  { id: 'chronicle', kind: 'weekly', name: 'Weekly Chronicle', label: '周度编年', tagline: '档案期刊 · 蓝灰墨色', tags: ['Chronicle', 'Editorial', 'Archive'], desc: '期刊式时间感与蓝灰档案色适合沉淀周度办案记录和工作摘要。', family: 'editorial', seal: 'none', tone: 'paper', themeColor: '#36546c' },
  { id: 'cyber-matrix', kind: 'weekly', name: 'Control Grid', label: '控制矩阵', tagline: '青色 HUD · 技术网格', tags: ['HUD', 'Grid', 'Cyan'], desc: '低亮度青色网格与等宽标题突出控制感，不再展示无来源的性能指数。', family: 'technical', seal: 'none', tone: 'dark', themeColor: '#33e6d7' },
  { id: 'hogwarts-letter', kind: 'weekly', name: 'Emerald Letter', label: '翡翠信笺', tagline: '翡翠墨色 · 深红火漆', tags: ['Letter', 'Emerald', 'Wax'], desc: '厚重档案纸、翡翠绿文字和真实深红火漆，保留仪式感而不过度幻想化。', family: 'heritage', seal: 'oxblood', tone: 'paper', themeColor: '#174b3c' },
  { id: 'classified-file', kind: 'weekly', name: 'Confidential File', label: '机密卷宗', tagline: '档案袋色 · 红色标记', tags: ['Confidential', 'Folder', 'Stamp'], desc: '档案袋色纸张和红色机密标记形成保密感，导出时不会擅自涂改真实内容。', family: 'document', seal: 'oxblood', tone: 'paper', themeColor: '#a1121a' },
  { id: 'vinyl-record', kind: 'weekly', name: 'Side A / Week', label: '黑胶周记', tagline: '唱片封套 · 暖橙棕色', tags: ['Vinyl', 'Retro', 'Collectible'], desc: '黑胶唱片纹理与暖橙封套带来音乐感，适合作为更有个性的周度纪念。', family: 'collectible', seal: 'none', tone: 'vivid', themeColor: '#d45b32' },
  { id: 'tarot', kind: 'weekly', name: 'Constellation Review', label: '星图复盘', tagline: '深靛星图 · 哑金边饰', tags: ['Constellation', 'Gold', 'Dark'], desc: '深靛底与对称金线形成典礼卡牌感，内容仍以真实复盘数据为中心。', family: 'ceremonial', seal: 'gold', tone: 'dark', themeColor: '#d2ac54' },
  { id: 'bank-note', kind: 'weekly', name: 'Assurance Note', label: '鉴证票据', tagline: '防伪纹理 · 墨绿票据', tags: ['Guilloche', 'Note', 'Green'], desc: '墨绿双线和细密扭索纹构成鉴证票据质感，适合正式的周度归档图片。', family: 'ceremonial', seal: 'gold', tone: 'paper', themeColor: '#174c36' },
  { id: 'passport', kind: 'weekly', name: 'Case Passport', label: '案件护照', tagline: '护照蓝 · 机读排版', tags: ['Passport', 'Mono', 'Archive'], desc: '护照蓝装订边与机读式小字，适合呈现跨周期案件工作的行程感。', family: 'collectible', seal: 'none', tone: 'light', themeColor: '#204d83' },
  { id: 'steampunk', kind: 'weekly', name: 'Brass Chronicle', label: '黄铜纪事', tagline: '哑黑纸面 · 黄铜机械', tags: ['Brass', 'Archive', 'Dark'], desc: '暗纸、黄铜边框和克制齿轮线条组合成稳重的机械纪事。', family: 'ceremonial', seal: 'gold', tone: 'dark', themeColor: '#c58434' },
  { id: 'wax-sealed-parchment', kind: 'weekly', name: 'Casy Decree', label: '火漆诏书', tagline: '棉纸底纹 · 暖金火漆', tags: ['Parchment', 'Decree', 'Wax'], desc: '棉纸纤维、双线金边和真实暖金火漆素材，形成最具仪式感的周报成品。', family: 'ceremonial', seal: 'gold', tone: 'paper', themeColor: '#a46e1f' },
]

export const dailyBriefingStyles = briefingStyleOptions.filter((style) => style.kind === 'daily')
export const weeklyBriefingStyles = briefingStyleOptions.filter((style) => style.kind === 'weekly')
export const briefingStyleMeta = Object.fromEntries(briefingStyleOptions.map((style) => [style.id, style])) as Record<string, BriefingStyleOption>
