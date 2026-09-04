/**
 * CommandMap 契约全覆盖门禁（防回退）
 *
 * 静态提取三类来源，做三方双向子集校验：
 *   1. 前端调用命令：src 下 tauriCallSafe / tauriCall / 直接 invoke 的字符串字面量
 *   2. CommandMap 键：src/types/commandMap.ts 中 `name: Cmd<...>` 的命令名
 *   3. Rust 注册命令：src-tauri/src/commands/mod.rs 的 tauri::generate_handler![...]
 *
 * 断言（任一失败即报差集）：
 *   - frontend ⊆ CommandMap（前端调用的命令必须已登记契约，禁止漏项）
 *   - frontend ⊆ Rust（前端调用的命令后端必须已实现）
 *   - CommandMap ⊆ Rust（契约表不允许登记后端不存在的命令）
 *
 * 说明：本门禁只做静态词法提取，不解析 TS 类型；命令名均为 snake_case 字符串字面量。
 */
import { describe, it, expect } from 'vitest'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

const ROOT = process.cwd()
const SRC_DIR = join(ROOT, 'src')
const COMMAND_MAP_FILE = join(SRC_DIR, 'types', 'commandMap.ts')
const RUST_COMMANDS_FILE = join(ROOT, 'src-tauri', 'src', 'commands', 'mod.rs')

/** 递归收集 src 下所有 .ts / .vue 文件 */
function collectSrcFiles(dir: string, acc: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name)
    if (statSync(full).isDirectory()) {
      // 跳过会污染提取结果的元数据目录（本仓库 src 下无此类，保守跳过）
      if (['node_modules', 'dist', '.git'].includes(name)) continue
      collectSrcFiles(full, acc)
    } else if (name.endsWith('.ts') || name.endsWith('.vue')) {
      acc.push(full)
    }
  }
  return acc
}

/** 提取前端调用命令：tauriCallSafe / tauriCall / 直接 invoke 的字符串字面量 */
function extractFrontendCommands(): Set<string> {
  const commands = new Set<string>()
  // 匹配调用函数名后紧跟的第一个字符串字面量参数（命令名）
  const callRe = /\b(?:tauriCallSafe|tauriCall)\s*\(\s*['"]([^'"\n]+)['"]/g
  // 直接 invoke 仅限从 @tauri-apps/api/core 导入的文件，避免误捕其它同名函数
  const invokeRe = /\binvoke\s*\(\s*['"]([^'"\n]+)['"]/g
  for (const file of collectSrcFiles(SRC_DIR)) {
    const content = readFileSync(file, 'utf8')
    let m: RegExpExecArray | null
    while ((m = callRe.exec(content)) !== null) commands.add(m[1])
    if (/from\s+['"]@tauri-apps\/api(\/core)?['"]/.test(content)) {
      invokeRe.lastIndex = 0
      while ((m = invokeRe.exec(content)) !== null) commands.add(m[1])
    }
  }
  return commands
}

/** 提取 CommandMap 键 */
function extractCommandMapKeys(): Set<string> {
  const text = readFileSync(COMMAND_MAP_FILE, 'utf8')
  const keys = new Set<string>()
  const keyRe = /^\s{0,3}([a-zA-Z_][a-zA-Z0-9_]*):\s*Cmd</gm
  let m: RegExpExecArray | null
  while ((m = keyRe.exec(text)) !== null) keys.add(m[1])
  return keys
}

/** 提取 Rust generate_handler 注册名（取每条最后一个路径段） */
function extractRustCommands(): Set<string> {
  const text = readFileSync(RUST_COMMANDS_FILE, 'utf8')
  const start = text.indexOf('tauri::generate_handler![')
  if (start < 0) return new Set()
  const end = text.indexOf(']', start)
  let block = text.slice(start, end)
  // 去除注释，避免注释文本被误识别为命令名
  block = block.replace(/\/\/.*$/gm, '').replace(/\/\*[\s\S]*?\*\//g, '')
  const names = new Set<string>()
  for (const part of block.split(',')) {
    const seg = part.trim()
    if (!seg) continue
    const m = seg.match(/([a-zA-Z_][a-zA-Z0-9_]*)\s*$/)
    if (m) names.add(m[1])
  }
  return names
}

/** 断言 subset ⊆ superset，否则抛出带差集的错误 */
function assertSubset(subset: Set<string>, superset: Set<string>, label: string): void {
  const missing = [...subset].filter((x) => !superset.has(x)).sort()
  if (missing.length) {
    throw new Error(
      `${label} 不成立：缺少 ${missing.length} 项\n${missing.map((x) => `  - ${x}`).join('\n')}`
    )
  }
}

describe('CommandMap 契约全覆盖', () => {
  const frontend = extractFrontendCommands()
  const commandMap = extractCommandMapKeys()
  const rust = extractRustCommands()

  it('静态提取非空（防提取逻辑退化）', () => {
    expect(frontend.size).toBeGreaterThan(0)
    expect(commandMap.size).toBeGreaterThan(0)
    expect(rust.size).toBeGreaterThan(0)
  })

  it('frontend ⊆ CommandMap：前端调用命令必须登记到 CommandMap', () => {
    assertSubset(frontend, commandMap, 'frontend ⊆ CommandMap')
  })

  it('frontend ⊆ Rust：前端调用命令后端必须已实现', () => {
    assertSubset(frontend, rust, 'frontend ⊆ Rust')
  })

  it('CommandMap ⊆ Rust：契约表不允许登记后端不存在的命令', () => {
    assertSubset(commandMap, rust, 'CommandMap ⊆ Rust')
  })
})
