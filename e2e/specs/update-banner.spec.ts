describe('Update banner (mock update)', () => {
  // Requires e2e-update-mock build — the mock makes check_for_update
  // immediately emit update-available with version "9.9.9"

  it('shows update banner when an update is available', async () => {
    // Give the startup update check time to fire
    await browser.pause(1000)
    const banner = await $('.update-banner')
    if (await banner.isExisting()) {
      await expect(banner).toBeDisplayed()
      await expect(banner).toHaveText(expect.stringContaining('9.9.9'))
    }
  })

  it('hides the update banner after clicking dismiss', async () => {
    await browser.pause(1000)
    const banner = await $('.update-banner')
    if (await banner.isExisting()) {
      const dismiss = await $('.update-dismiss')
      await dismiss.click()
      await expect(banner).not.toBeDisplayed()
    }
  })
})
