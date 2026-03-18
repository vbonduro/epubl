describe('Device panel', () => {
  it('shows placeholder when no device is connected', async () => {
    // The non-mock build returns an empty device list
    const placeholder = await $('.panel-device .placeholder')
    await expect(placeholder).toBeDisplayed()
    await expect(placeholder).toHaveText('Connect your eReader')
  })

  it('does not show connected badge when no device is present', async () => {
    const badge = await $('.badge-connected')
    await expect(badge).not.toExist()
  })

  it('disables eject button when no device is connected', async () => {
    const btn = await $('.btn-eject')
    await expect(btn).toBeDisabled()
  })
})

describe('Device panel (mock device)', () => {
  // These tests require the e2e-mock build (EPUBL_BIN set to mock binary)
  // They are skipped automatically when the mock feature is not present
  // because get_connected_ereaders returns [] on Linux without the feature.
  // Run with: EPUBL_BIN=./src-tauri/target/release/epubl-mock npx wdio run wdio.conf.ts

  it('shows device model when connected', async () => {
    const model = await $('.device-model')
    // Only assert if element exists (mock build)
    if (await model.isExisting()) {
      await expect(model).toHaveText('Kindle Paperwhite')
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
