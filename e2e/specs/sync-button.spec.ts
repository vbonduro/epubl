describe('Sync button', () => {
  it('shows Syncing… label and is disabled during sync', async () => {
    const btn = await $('.btn-sync')
    await btn.click()
    await expect(btn).toHaveText('Syncing…')
    await expect(btn).toBeDisabled()
  })

  it('returns to Sync label after operation completes', async () => {
    const btn = await $('.btn-sync')
    // Wait for the setTimeout stub to complete (1500ms + buffer)
    await browser.pause(2000)
    await expect(btn).toHaveText('Sync')
    await expect(btn).not.toBeDisabled()
  })
})
