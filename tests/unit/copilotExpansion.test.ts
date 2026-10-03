// @vitest-environment jsdom
import { expect, it } from 'vitest'
import { shallowMount } from '@vue/test-utils'
import CopilotSidebar from '../../src/modules/docs/components/CopilotSidebar.vue'

it('knowledge card clicks emit the selected id to the parent', async () => {
  const view = shallowMount(CopilotSidebar, { props: {
    searchQuery: 'evidence',
    searchResults: { paragraphs: [{ id: 'p', title: 'Evidence', content: 'Text' }], laws: [], cases: [] },
  } })
  await view.find('.card-header').trigger('click')
  expect(view.emitted('toggle-expand')).toEqual([['p']])
  view.unmount()
})
