import type { Component } from 'vue'
import { Avatar, User, Suitcase, OfficeBuilding, Postcard } from '../../shared/icons'

/** 实体类型（与后端 persons.kind CHECK 约束一致） */
export type PersonKind = 'judge' | 'client' | 'opposing_counsel' | 'court' | 'contact'

/** 与后端 PersonDto 对齐（serde camelCase） */
export interface PersonDto {
  id: string
  kind: PersonKind
  name: string
  org: string | null
  phone: string | null
  email: string | null
  /** 法官偏好 / 庭上习惯等（单一事实源核心字段） */
  preferences: string | null
  notes: string | null
  createdAt: string | null
  updatedAt: string | null
  caseCount: number | null
}

/** 案件侧：挂载的实体（list_case_persons） */
export interface CasePersonDto {
  linkId: string
  person: PersonDto
  role: string | null
}

/** 实体侧：关联的案件（list_person_cases） */
export interface PersonCaseDto {
  linkId: string
  caseId: string
  caseName: string
  caseNo: string | null
  role: string | null
}

export interface KindMeta {
  label: string
  color: string
  icon: Component
}

export const KIND_META: Record<PersonKind, KindMeta> = {
  judge: { label: '法官', color: '#6C6A9C', icon: Avatar },
  client: { label: '客户', color: '#3E5C9A', icon: User },
  opposing_counsel: { label: '对方律师', color: '#B0823A', icon: Suitcase },
  court: { label: '法院', color: '#244481', icon: OfficeBuilding },
  contact: { label: '联系人', color: '#747781', icon: Postcard },
}

export const KIND_OPTIONS: PersonKind[] = ['judge', 'client', 'opposing_counsel', 'court', 'contact']

export function kindMeta(kind: string): KindMeta {
  return (KIND_META as Record<string, KindMeta>)[kind] ?? KIND_META.contact
}

/** sqlite datetime('now','localtime') → 'YYYY-MM-DD HH:MM' */
export function formatPersonTime(ts: string | null | undefined): string {
  if (!ts) return '—'
  return ts.slice(0, 16)
}

/** 摘要截断 */
export function summarize(text: string | null | undefined, max = 60): string {
  if (!text) return ''
  const t = text.trim()
  return t.length > max ? t.slice(0, max) + '…' : t
}
