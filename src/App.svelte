<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  // epubl — main shell

  // ---------------------------------------------------------------------------
  // Update banner state
  // ---------------------------------------------------------------------------

  let updateVersion: string | null = $state(null);

  // ---------------------------------------------------------------------------
  // Device state
  // ---------------------------------------------------------------------------

  interface EReaderInfo {
    driveLetter: string;
    model: string;
    vendor: string;
  }

  let device = $state<EReaderInfo | null>(null);
  let ejectError = $state<string | null>(null);

  // ---------------------------------------------------------------------------
  // Sync state
  // ---------------------------------------------------------------------------

  let syncBusy = $state(false);

  function handleSync() {
    syncBusy = true;
    // TODO: invoke Tauri sync command
    setTimeout(() => (syncBusy = false), 1500);
  }

  // ---------------------------------------------------------------------------
  // Eject
  // ---------------------------------------------------------------------------

  async function handleEject() {
    if (!device) return;
    ejectError = null;
    try {
      await invoke("eject", { driveLetter: device.driveLetter });
      // After a successful eject the disconnected event will arrive shortly;
      // clear the device immediately so the UI feels responsive.
      device = null;
    } catch (err) {
      ejectError = String(err);
    }
  }

  // ---------------------------------------------------------------------------
  // Lifecycle: initial query + real-time event listeners
  // ---------------------------------------------------------------------------

  onMount(() => {
    // Existing update-available listener.
    const unlistenUpdate = listen<{ version: string; notes: string | null }>(
      "update-available",
      (event) => {
        updateVersion = event.payload.version;
      }
    );

    // Populate device state from the current snapshot.
    invoke<EReaderInfo[]>("get_connected_ereaders")
      .then((readers) => {
        device = readers.length > 0 ? readers[0] : null;
      })
      .catch((err) => {
        console.error("[epubl] get_connected_ereaders failed:", err);
      });

    // Real-time eReader connection events.
    const unlistenConnected = listen<EReaderInfo>("ereader-connected", (event) => {
      device = event.payload;
      ejectError = null;
    });

    const unlistenDisconnected = listen<EReaderInfo>("ereader-disconnected", () => {
      device = null;
      ejectError = null;
    });

    return () => {
      unlistenUpdate.then((fn) => fn());
      unlistenConnected.then((fn) => fn());
      unlistenDisconnected.then((fn) => fn());
    };
  });
</script>

<div class="shell">
  {#if updateVersion}
    <div class="update-banner" role="alert">
      <span>Update available: v{updateVersion} —</span>
      <a
        href="https://github.com/vbonduro/epubl/releases/latest"
        target="_blank"
        rel="noopener noreferrer"
      >Download</a>
      <button class="update-dismiss" onclick={() => (updateVersion = null)} aria-label="Dismiss">✕</button>
    </div>
  {/if}

  <div class="main-area">
    <!-- Left panel: epub library -->
    <aside class="panel panel-library">
      <h2 class="panel-title">Library</h2>
      <p class="placeholder">No epub files found. Add files to get started.</p>
    </aside>

    <!-- Right panel: device status -->
    <aside class="panel panel-device">
      <h2 class="panel-title">Device</h2>

      {#if device}
        <div class="device-info">
          <p class="device-model">{device.model}</p>
          <p class="device-drive">{device.driveLetter}</p>
          <span class="badge badge-connected">Connected</span>
        </div>
      {:else}
        <p class="placeholder muted">Connect your eReader</p>
      {/if}

      {#if ejectError}
        <p class="eject-error">{ejectError}</p>
      {/if}
    </aside>
  </div>

  <!-- Bottom bar -->
  <footer class="bottom-bar">
    <button class="btn btn-sync" onclick={handleSync} disabled={syncBusy}>
      {syncBusy ? "Syncing…" : "Sync"}
    </button>
    <button class="btn btn-eject" onclick={handleEject} disabled={!device}>
      Eject
    </button>
  </footer>
</div>
