<script lang="ts">
  // epubl — main shell
  let syncBusy = $state(false);

  function handleSync() {
    syncBusy = true;
    // TODO: invoke Tauri sync command
    setTimeout(() => (syncBusy = false), 1500);
  }

  function handleEject() {
    // TODO: invoke Tauri eject command
    console.log("eject requested");
  }
</script>

<div class="shell">
  <div class="main-area">
    <!-- Left panel: epub library -->
    <aside class="panel panel-library">
      <h2 class="panel-title">Library</h2>
      <p class="placeholder">No epub files found. Add files to get started.</p>
    </aside>

    <!-- Right panel: device status -->
    <aside class="panel panel-device">
      <h2 class="panel-title">Device</h2>
      <p class="placeholder">No device connected.</p>
    </aside>
  </div>

  <!-- Bottom bar -->
  <footer class="bottom-bar">
    <button class="btn btn-sync" onclick={handleSync} disabled={syncBusy}>
      {syncBusy ? "Syncing…" : "Sync"}
    </button>
    <button class="btn btn-eject" onclick={handleEject}>
      Eject
    </button>
  </footer>
</div>
