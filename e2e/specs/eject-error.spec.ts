describe('Eject error display (mock build)', () => {
  // Requires e2e-mock build — eject always returns "Mocked eject error"

  it('shows error message when eject fails', async () => {
    const badge = await $('.badge-connected')
    if (!await badge.isExisting()) {
      // Not a mock build — skip
      return
    }
    const btn = await $('.btn-eject')
    await btn.click()
    const error = await $('.eject-error')
    await expect(error).toBeDisplayed()
    await expect(error).toHaveText(expect.stringContaining('Mocked eject error'))
  })
})
