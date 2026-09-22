import type { ToolParameterSchema } from './types'

/** Validate our supported JSON Schema subset at the tool boundary, before effects. */
export function validateParams(schema: ToolParameterSchema, value: unknown, path = 'params'): string | null {
  if (schema.type === 'object') {
    if (value === null || typeof value !== 'object' || Array.isArray(value)) return `${path} 必须是对象`
    const record = value as Record<string, unknown>
    for (const key of schema.required || []) {
      if (record[key] === undefined || record[key] === null || record[key] === '') return `${path}.${key} 必填`
    }
    for (const [key, field] of Object.entries(schema.properties || {})) {
      if (record[key] === undefined) continue
      const error = validateParams(field, record[key], `${path}.${key}`)
      if (error) return error
    }
  } else if (schema.type === 'array') {
    if (!Array.isArray(value)) return `${path} 必须是数组`
    if (schema.items) {
      for (let i = 0; i < value.length; i++) {
        const error = validateParams(schema.items, value[i], `${path}[${i}]`)
        if (error) return error
      }
    }
  } else if (schema.type === 'number' || schema.type === 'integer') {
    if (typeof value !== 'number' || !Number.isFinite(value) || (schema.type === 'integer' && !Number.isInteger(value))) return `${path} 必须是有效${schema.type === 'integer' ? '整数' : '数字'}`
  } else if ((schema.type === 'string' || schema.type === 'boolean') && typeof value !== schema.type) {
    return `${path} 必须是${schema.type === 'string' ? '文本' : '布尔值'}`
  }
  if (schema.enum && !schema.enum.includes(String(value))) return `${path} 必须是 ${schema.enum.join(' / ')}`
  return null
}
