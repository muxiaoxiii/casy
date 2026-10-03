// @vitest-environment jsdom
import { expect, it } from 'vitest'
import { shallowMount } from '@vue/test-utils'
import UiDataState from '../../src/shared/ui/UiDataState.vue'
import EmptyState from '../../src/shared/components/EmptyState.vue'
import DegradedBanner from '../../src/shared/components/DegradedBanner.vue'
import SkeletonCard from '../../src/shared/components/SkeletonCard.vue'
it('separates loading, empty, filtered and failed states', async () => {
  const view = shallowMount(UiDataState, { props: { loading: true, count: 0 } })
  expect(view.findComponent(SkeletonCard).exists()).toBe(true)
  expect(view.findComponent(EmptyState).exists()).toBe(false)
  await view.setProps({ loading: false, filtered: true })
  expect(view.findComponent(EmptyState).props('title')).toBe('没有匹配结果')
  view.findComponent(EmptyState).vm.$emit('action')
  expect(view.emitted('clear')).toHaveLength(1)
  await view.setProps({ error: 'Database unavailable' })
  expect(view.findComponent(EmptyState).exists()).toBe(false)
  expect(view.findComponent(DegradedBanner).props('dismissible')).toBe(false)
  view.findComponent(DegradedBanner).vm.$emit('retry')
  expect(view.emitted('retry')).toHaveLength(1)
  view.unmount()
})
it('retains prior content during refresh and refresh failure', async () => {
  const view = shallowMount(UiDataState, { props: { loading: true, count: 2 }, slots: { default: '<p class="saved">Previous results</p>' } })
  expect(view.find('.saved').exists()).toBe(true)
  await view.setProps({ loading: false, error: 'Refresh failed' })
  expect(view.find('.saved').exists()).toBe(true)
  expect(view.findComponent(DegradedBanner).exists()).toBe(true)
  view.unmount()
})
