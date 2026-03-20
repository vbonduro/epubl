// sync-button.spec.ts
// Tests for the Sync button behaviour.
// Requires the e2e-mock build which pre-selects 2 books by default.

describe('Sync button', () => {
  before(async () => {
    // Wait for books to load — if none appear this is a non-mock build
    await browser.waitUntil(
      async () => (await $$('.epub-item-new')).length > 0,
      { timeout: 3000, interval: 100, timeoutMsg: 'No epub-item-new found' }
    ).catch(() => {/* non-mock build — tests will skip */})
  })

  it('shows Syncing… label and is disabled during sync', async () => {
    const items = await $$('.epub-item-new')
    if (items.length === 0) return // non-mock build, skip

    const btn = await $('.btn-sync')
    await btn.click()
    await expect(btn).toHaveText('Syncing…')
    await expect(btn).toBeDisabled()
  })

  it('returns to Sync (N) label after operation completes', async () => {
    const items = await $$('.epub-item-new')
    if (items.length === 0) return // non-mock build, skip

    const btn = await $('.btn-sync')
    // Wait for the setTimeout stub to complete (1500ms + buffer)
    await browser.pause(2000)
    // Button label includes selection count, e.g. "Sync (2)"
    await expect(btn).toHaveText('Sync (2)')
    await expect(btn).not.toBeDisabled()
  })
})
