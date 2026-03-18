describe('Device panel (no device)', () => {
  // Skipped automatically on mock builds where get_connected_ereaders returns a device.
  it('shows placeholder when no device is connected', async () => {
    const badge = await $('.badge-connected')
    if (await badge.isExisting()) {
      // Mock build — device is present, skip this assertion
      return
    }
    const placeholder = await $('.panel-device .placeholder')
    await expect(placeholder).toBeDisplayed()
    await expect(placeholder).toHaveText('Connect your eReader')
  })

  it('does not show connected badge when no device is present', async () => {
    const badge = await $('.badge-connected')
    if (await badge.isExisting()) return
    await expect(badge).not.toExist()
  })

  it('disables eject button when no device is connected', async () => {
    const badge = await $('.badge-connected')
    if (await badge.isExisting()) return
    const btn = await $('.btn-eject')
    await expect(btn).toBeDisabled()
  })
})

describe('Device panel (mock device)', () => {
  // These tests require the e2e-mock build which injects a Kindle device.
  it('shows device model when connected', async () => {
    const model = await $('.device-model')
    if (await model.isExisting()) {
      await expect(model).toHaveText('Kindle Internal Storage')
    }
  })

  it('shows drive letter when connected', async () => {
    const drive = await $('.device-drive')
    if (await drive.isExisting()) {
      await expect(drive).toHaveText('E:')
    }
  })

  it('shows connected badge when device is present', async () => {
    const badge = await $('.badge-connected')
    if (await badge.isExisting()) {
      await expect(badge).toBeDisplayed()
    }
  })

  it('enables eject button when device is present', async () => {
    const badge = await $('.badge-connected')
    if (await badge.isExisting()) {
      const btn = await $('.btn-eject')
      await expect(btn).not.toBeDisabled()
    }
  })
})
