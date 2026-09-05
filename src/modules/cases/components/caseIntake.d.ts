export interface IntakeDraft extends Record<string, unknown> {
  caseName: string
  notes: string
  track: string
  caseRoute: string
  thirdParties: Array<Record<string, string>> | string
  attorneys: string[] | string
}
export function newIntake(): IntakeDraft
export function parseIntakeText(text: string): { fields: Record<string, unknown>; unmatched: string[] }
export function setIntakeRoute(form: IntakeDraft, route: string): void
export function intakePayload(form: IntakeDraft): Record<string, unknown>
export const roleOptions: string[]
export const routeOptions: Array<[string, string]>
export const intakeSections: Array<{ key: string; label: string; fields: Array<{ key: string; label: string; type: string; options: string[] }> }>
export const statusGroups: Array<{ route: string; key: string; label: string; options: Array<[string, string]> }>
