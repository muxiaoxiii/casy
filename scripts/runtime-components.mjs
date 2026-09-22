// Component boundaries are metadata only: every component remains bundled.
export const runtimeComponent = path => {
  if (path.startsWith('models/embedding-e5-base/')) return 'embedding-e5-base'
  if (path.startsWith('models/ppocrv6-medium/')) return 'ocr-ppocrv6-medium'
  if (path.startsWith('models/korean-ppocrv5-mobile/')) return 'ocr-korean-ppocrv5-mobile'
  if (path.startsWith('models/layout/')) return 'layout'
  if (path.startsWith('licenses/')) return 'compliance'
  if (path.startsWith('fonts/')) return 'fonts'
  if (path.startsWith('zvec/')) return 'vector-runtime'
  return 'document-runtime'
}
export function componentInventory(files) {
  const groups = new Map()
  for (const file of files) {
    const id = runtimeComponent(file.path)
    const component = groups.get(id) || { id, delivery: 'bundled', bytes: 0, files: [] }
    component.bytes += file.bytes
    component.files.push(file.path)
    groups.set(id, component)
  }
  return [...groups.values()].sort((a, b) => a.id.localeCompare(b.id))
}
