import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router'
const origins = new Map<string, string>()
export function rememberOrigin(to: { fullPath: string; name?: unknown }, from: { fullPath: string; name?: unknown }) {
  if (!from.name || to.name === from.name || to.fullPath === from.fullPath) return
  if (origins.get(from.fullPath) === to.fullPath) return
  origins.set(to.fullPath, from.fullPath)
  if (origins.size > 100) origins.delete(origins.keys().next().value!)
}
export function useReturnOrigin(fallback: () => RouteLocationRaw) {
  const route = useRoute(), router = useRouter()
  return () => router.push(origins.get(route.fullPath) || fallback())
}
