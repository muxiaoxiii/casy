import { computed, ref } from 'vue'
import { useUnsavedForm } from './useUnsavedForm'
export function useFormBaseline(label: string, read: () => unknown, busy: () => boolean = () => false, restore?: (value: any) => void) {
  const original = ref(JSON.stringify(read()))
  const dirty = computed(() => JSON.stringify(read()) !== original.value)
  const markSaved = (snapshot: unknown = read()) => { original.value = JSON.stringify(snapshot) }
  const guard = useUnsavedForm(label, () => dirty.value, busy, () => restore?.(JSON.parse(original.value)))
  return { dirty, markSaved, ...guard }
}
