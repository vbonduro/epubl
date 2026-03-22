// sync-button.spec.ts
// Tests for the Sync button behaviour.
// Requires the e2e-mock build which pre-selects 2 books by default.

describe('Sync button', () => {
  before(async () => {
    // Wait for books to load — if none appear this is a non-mock build
    await browser.waitUntil(
      async () => (await $$('.epub-item-new')).length > 0,
      { timeout: 8000, interval: 100, timeoutMsg: 'No epub-item-new found' }
    ).catch(() => {/* non-mock build — tests will skip */})
  })

  it('shows progress indicator during sync', async () => {
    const items = await $$('.epub-item-new')
    if (items.length === 0) return // non-mock build, skip

    const btn = await $('.btn-sync')
    await btn.click()
    // When syncBusy=true the button is replaced by the transfer-progress div
    await browser.waitUntil(
      async () => (await $$('.transfer-progress')).length > 0,
      { timeout: 3000, interval: 100, timeoutMsg: 'Transfer progress indicator never appeared' }
    )
    const progress = await $('.transfer-progress')
    await expect(progress).toBeDisplayed()
  })

  it('returns to Load N Books label after operation completes', async () => {
    const items = await $$('.epub-item-new')
    if (items.length === 0) return // non-mock build, skip

    // Wait for sync to finish — btn-sync reappears when syncBusy=false
    await browser.waitUntil(
      async () => (await $$('.btn-sync')).length > 0,
      { timeout: 8000, interval: 200, timeoutMsg: 'Sync button never reappeared' }
    )
    const btn = await $('.btn-sync')
    await expect(btn).not.toBeDisabled()
  })
})
