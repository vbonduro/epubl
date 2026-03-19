<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";

  // ---------------------------------------------------------------------------
  // Types
  // ---------------------------------------------------------------------------

  interface Config {
    epubFolder: string;
    ereaderPath: string | null;
    bookstoreUrl: string;
    supportEmail: string;
    firstRun: boolean;
  }

  interface EReaderInfo {
    driveLetter: string;
    model: string;
    vendor: string;
  }

  interface EpubInfo {
    filename: string;
    title: string;
    author: string;
    sizeBytes: number;
  }

  // ---------------------------------------------------------------------------
  // App state
  // ---------------------------------------------------------------------------

  let config = $state<Config | null>(null);
  let showWizard = $state(false);

  // Wizard state
  let wizardFolder = $state("");
  let wizardEmail = $state("");
  let wizardBusy = $state(false);
  let wizardError = $state<string | null>(null);

  // Library
  let epubs = $state<EpubInfo[]>([]);
  let libraryError = $state<string | null>(null);

  // Device
  let device = $state<EReaderInfo | null>(null);
  let ejectError = $state<string | null>(null);

  // Sync
  let syncBusy = $state(false);

  // Update banner
  let updateVersion = $state<string | null>(null);

  // ---------------------------------------------------------------------------
  // Wizard
  // ---------------------------------------------------------------------------

  async function pickFolder() {
    const selected = await openDialog({ directory: true, multiple: false });
    if (typeof selected === "string") wizardFolder = selected;
  }

  async function completeWizard() {
    if (!wizardFolder) { wizardError = "Please select your epub folder."; return; }
    wizardBusy = true;
    wizardError = null;
    try {
      const updated: Config = {
        ...config!,
        epubFolder: wizardFolder,
        supportEmail: wizardEmail.trim(),
        firstRun: false,
      };
      await invoke("set_config", { config: updated });
      config = updated;
      showWizard = false;
      await loadLibrary();
    } catch (err) {
      wizardError = String(err);
    } finally {
      wizardBusy = false;
    }
  }

  // ---------------------------------------------------------------------------
  // Report problem
  // ---------------------------------------------------------------------------

  function reportProblem(context: string) {
    const email = config?.supportEmail;
    if (!email) return;
    const subject = encodeURIComponent("epubl problem report");
    const body = encodeURIComponent(
      `Hi,\n\nI ran into a problem with epubl:\n\n${context}\n\n` +
      `-- App info --\nOS: Windows\n`
    );
    window.open(`mailto:${email}?subject=${subject}&body=${body}`, "_self");
  }

  // ---------------------------------------------------------------------------
  // Library
  // ---------------------------------------------------------------------------

  async function loadLibrary() {
    if (!config?.epubFolder) return;
    try {
      epubs = await invoke<EpubInfo[]>("list_epubs", { folderPath: config.epubFolder });
      libraryError = null;
    } catch (err) {
      libraryError = String(err);
    }
  }

  // ---------------------------------------------------------------------------
  // Eject
  // ---------------------------------------------------------------------------

  async function handleEject() {
    if (!device) return;
    ejectError = null;
    try {
      await invoke("eject", { driveLetter: device.driveLetter });
      device = null;
    } catch (err) {
      ejectError = String(err);
    }
  }

  // ---------------------------------------------------------------------------
  // Sync
  // ---------------------------------------------------------------------------

  function handleSync() {
    syncBusy = true;
    // TODO: invoke Tauri sync command (epubl-iv7)
    setTimeout(() => (syncBusy = false), 1500);
  }

  // ---------------------------------------------------------------------------
  // Lifecycle
  // ---------------------------------------------------------------------------

  onMount(() => {
    invoke<Config>("get_config")
      .then((cfg) => {
        config = cfg;
        if (cfg.firstRun) {
          showWizard = true;
        } else {
          loadLibrary();
        }
      })
      .catch((err) => console.error("[epubl] get_config failed:", err));

    invoke<EReaderInfo[]>("get_connected_ereaders")
      .then((readers) => { device = readers[0] ?? null; })
      .catch((err) => console.error("[epubl] get_connected_ereaders failed:", err));

    const unlistenUpdate = listen<{ version: string }>("update-available", (e) => {
      updateVersion = e.payload.version;
    });
    const unlistenConnected = listen<EReaderInfo>("ereader-connected", (e) => {
      device = e.payload; ejectError = null;
    });
    const unlistenDisconnected = listen("ereader-disconnected", () => {
      device = null; ejectError = null;
    });

    return () => {
      unlistenUpdate.then((fn) => fn());
      unlistenConnected.then((fn) => fn());
      unlistenDisconnected.then((fn) => fn());
    };
  });
</script>

<!-- =========================================================
     First-run setup wizard
     ========================================================= -->
{#if showWizard}
  <div class="wizard-overlay" role="dialog" aria-modal="true" aria-labelledby="wizard-title">
    <div class="wizard">
      <h1 id="wizard-title" class="wizard-title">Welcome to epubl</h1>
      <p class="wizard-intro">Let's get you set up. Where are your epub files stored?</p>

      <div class="wizard-row">
        <input
          class="wizard-input"
          type="text"
          readonly
          placeholder="Select a folder…"
          value={wizardFolder}
          aria-label="epub folder path"
        />
        <button class="btn" onclick={pickFolder}>Browse…</button>
      </div>

      <p class="wizard-intro" style="margin-top: 8px;">
        Support email — where should problem reports be sent?
      </p>
      <div class="wizard-row">
        <input
          class="wizard-input"
          type="email"
          placeholder="your@email.com (optional)"
          bind:value={wizardEmail}
          aria-label="support email address"
        />
      </div>

      {#if wizardError}
        <p class="wizard-error">{wizardError}</p>
      {/if}

      <button
        class="btn btn-sync wizard-done"
        onclick={completeWizard}
        disabled={wizardBusy || !wizardFolder}
      >
        {wizardBusy ? "Saving…" : "Done"}
      </button>
    </div>
  </div>
{:else}
  <!-- =========================================================
       Main shell
       ========================================================= -->
  <div class="shell">
    {#if updateVersion}
      <div class="update-banner" role="alert">
        <span>Update available: v{updateVersion} —</span>
        <a href="https://github.com/vbonduro/epubl/releases/latest" target="_blank" rel="noopener noreferrer">
          Download
        </a>
        <button class="update-dismiss" onclick={() => (updateVersion = null)} aria-label="Dismiss">✕</button>
      </div>
    {/if}

    <div class="main-area">
      <!-- Left: epub library -->
      <aside class="panel panel-library">
        <h2 class="panel-title">Library</h2>

        {#if libraryError}
          <p class="error-text">{libraryError}</p>
          {#if config?.supportEmail}
            <button class="btn btn-report" onclick={() => reportProblem(`Library error: ${libraryError}`)}>
              Report problem
            </button>
          {/if}
        {:else if epubs.length === 0}
          <p class="placeholder">No epub files found in your library folder.</p>
        {:else}
          <ul class="epub-list">
            {#each epubs as book (book.filename)}
              <li class="epub-item">
                <span class="epub-title">{book.title}</span>
                {#if book.author}
                  <span class="epub-author">{book.author}</span>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </aside>

      <!-- Right: device status -->
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
          {#if config?.supportEmail}
            <button class="btn btn-report" onclick={() => reportProblem(`Eject error: ${ejectError}`)}>
              Report problem
            </button>
          {/if}
        {/if}

        {#if config?.bookstoreUrl}
          <a
            class="bookstore-link"
            href={config.bookstoreUrl}
            target="_blank"
            rel="noopener noreferrer"
          >Browse bookstore</a>
        {/if}
      </aside>
    </div>

    <footer class="bottom-bar">
      <button class="btn btn-sync" onclick={handleSync} disabled={syncBusy}>
        {syncBusy ? "Syncing…" : "Sync"}
      </button>
      <button class="btn btn-eject" onclick={handleEject} disabled={!device}>
        Eject
      </button>
    </footer>
  </div>
{/if}
