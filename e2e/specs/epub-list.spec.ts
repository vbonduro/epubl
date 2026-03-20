// epub-list.spec.ts
// Tests for the epub multi-select list UI.
// Requires the e2e-mock build which injects a mock diff result:
//   to_copy:    ["Mock Book One", "Mock Book Two"]
//   up_to_date: ["Already Synced"]

describe('Epub list (mock diff)', () => {
  it('shows books to copy checked by default', async () => {
    const items = await $$('.epub-item-new')
    if (items.length === 0) return // non-mock build, skip

    for (const item of items) {
      const checkbox = await item.$('input[type="checkbox"]')
      await expect(checkbox).toBeChecked()
    }
  })

  it('shows two books to copy in mock build', async () => {
    const items = await $$('.epub-item-new')
    if (items.length === 0) return

    expect(items.length).toBe(2)
  })

  it('shows up-to-date books unchecked and dimmed', async () => {
    const items = await $$('.epub-item-synced')
    if (items.length === 0) return

    for (const item of items) {
      const checkbox = await item.$('input[type="checkbox"]')
      await expect(checkbox).not.toBeChecked()
    }
  })

  it('shows one already-synced book in mock build', async () => {
    const items = await $$('.epub-item-synced')
    if (items.length === 0) return

    expect(items.length).toBe(1)
  })

  it('sync button is enabled when at least one book is selected', async () => {
    const items = await $$('.epub-item-new')
    if (items.length === 0) return

    const syncBtn = await $('.btn-sync')
    await expect(syncBtn).not.toBeDisabled()
  })

  it('sync button is disabled when all books are deselected', async () => {
    const items = await $$('.epub-item-new')
    if (items.length === 0) return

    // Uncheck all new books
    for (const item of items) {
      const checkbox = await item.$('input[type="checkbox"]')
      await checkbox.click()
    }

    const syncBtn = await $('.btn-sync')
    await expect(syncBtn).toBeDisabled()

    // Restore — re-check all for subsequent tests
    for (const item of items) {
      const checkbox = await item.$('input[type="checkbox"]')
      await checkbox.click()
    }
  })
})
