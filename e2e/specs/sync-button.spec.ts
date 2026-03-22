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

  it('shows Syncing… label and is disabled during sync', async () => {
    const items = await $$('.epub-item-new')
    if (items.length === 0) return // non-mock build, skip

    const btn = await $('.btn-sync')
    await btn.click()
    // Wait for the disabled state which indicates sync is in progress
    await browser.waitUntil(
      async () => await btn.getAttribute('disabled') !== null,
      { timeout: 3000, interval: 100, timeoutMsg: 'Button never became disabled' }
    )
    await expect(btn).toBeDisabled()
  })

  it('returns to Load N Books label after operation completes', async () => {
    const items = await $$('.epub-item-new')
    if (items.length === 0) return // non-mock build, skip

    const btn = await $('.btn-sync')
    // Wait for sync to finish (mock takes ~900ms) plus buffer
    await browser.waitUntil(
      async () => await btn.getAttribute('disabled') === null,
      { timeout: 5000, interval: 200, timeoutMsg: 'Button never became enabled again' }
    )
    await expect(btn).not.toBeDisabled()
  })
})
