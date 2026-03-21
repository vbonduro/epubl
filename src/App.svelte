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

  interface DiffResult {
    toCopy: EpubInfo[];
    upToDate: EpubInfo[];
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

  // Library / diff
  let diff = $state<DiffResult | null>(null);
  let selected = $state<Set<string>>(new Set());
  let libraryError = $state<string | null>(null);

  // Device
  let device = $state<EReaderInfo | null>(null);
  let ejectError = $state<string | null>(null);

  // Sync
  let syncBusy = $state(false);
  let syncError = $state<string | null>(null);
  let syncProgress = $state<{ filename: string; filesDone: number; filesTotal: number; bytesCopied: number; bytesTotal: number } | null>(null);

  // Update banner
  let updateVersion = $state<string | null>(null);

  // Dev mode — true when running in browser without Tauri
  let devMode = $state(false);

  // ---------------------------------------------------------------------------
  // Wizard
  // ---------------------------------------------------------------------------

  async function pickFolder() {
    try {
      const selected = await openDialog({ directory: true, multiple: false });
      if (typeof selected === "string") wizardFolder = selected;
    } catch {
      // Running in browser without Tauri — let user type the path directly
      wizardFolder = prompt("Enter your epub folder path:") ?? wizardFolder;
    }
  }

  async function completeWizard() {
    if (!wizardFolder) { wizardError = "Please select your epub folder."; return; }
    wizardBusy = true;
    wizardError = null;
    try {
      const updated: Config = {
        epubFolder: wizardFolder,
        ereaderPath: config?.ereaderPath ?? null,
        bookstoreUrl: config?.bookstoreUrl ?? "https://www.amazon.com/ebooks",
        supportEmail: wizardEmail.trim(),
        firstRun: false,
      };
      try { await invoke("set_config", { config: updated }); } catch { /* browser mode */ }
      config = updated;
      showWizard = false;
      await loadDiff();
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
  // Library / diff
  // ---------------------------------------------------------------------------

  async function loadDiff() {
    if (!config?.epubFolder || !device) return;
    try {
      const result = await invoke<DiffResult>("diff_epubs", {
        localFolder: config.epubFolder,
        deviceFolder: device.driveLetter + "/documents",
      });
      diff = result;
      // Select all new books by default
      selected = new Set(result.toCopy.map((b) => b.filename));
      libraryError = null;
    } catch (err) {
      libraryError = String(err);
    }
  }

  function toggleBook(filename: string) {
    const next = new Set(selected);
    if (next.has(filename)) {
      next.delete(filename);
    } else {
      next.add(filename);
    }
    selected = next;
  }

  // ---------------------------------------------------------------------------
  // Eject
  // ---------------------------------------------------------------------------

  async function handleEject() {
    if (!device) return;
    if (devMode) { mockDisconnect(); return; }
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

  async function handleSync() {
    if (selected.size === 0 || !device) return;
    if (devMode) { mockTransfer(); return; }
    if (!config?.epubFolder) return;
    syncBusy = true;
    syncError = null;
    syncProgress = null;

    const unlistenProgress = listen<{ filename: string; filesDone: number; filesTotal: number; bytesCopied: number; bytesTotal: number }>(
      "copy-progress",
      (e) => { syncProgress = e.payload; }
    );
    const unlistenComplete = listen("copy-complete", () => {
      syncBusy = false;
      syncProgress = null;
      loadDiff();
    });

    try {
      await invoke("copy_epubs", {
        filenames: [...selected],
        localFolder: config.epubFolder,
        deviceFolder: device.driveLetter + "/documents",
      });
    } catch (err) {
      syncError = String(err);
      syncBusy = false;
      syncProgress = null;
    } finally {
      unlistenProgress.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
    }
  }

  // ---------------------------------------------------------------------------
  // Select all / deselect all
  // ---------------------------------------------------------------------------

  function toggleSelectAll() {
    if (!diff) return;
    if (selected.size === diff.toCopy.length) {
      selected = new Set();
    } else {
      selected = new Set(diff.toCopy.map((b) => b.filename));
    }
  }

  // ---------------------------------------------------------------------------
  // Helpers
  // ---------------------------------------------------------------------------

  function formatSize(bytes: number): string {
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function progressPct(): number {
    if (!syncProgress || syncProgress.bytesTotal === 0) return 0;
    return Math.round((syncProgress.bytesCopied / syncProgress.bytesTotal) * 100);
  }

  // ---------------------------------------------------------------------------
  // Mock controls (dev mode only)
  // ---------------------------------------------------------------------------

  function mockConnect() {
    device = { driveLetter: "E:", model: "Kobo Clara HD", vendor: "Kobo" };
    ejectError = null;
    mockLoadBooks();
  }

  function mockDisconnect() {
    device = null;
    ejectError = null;
    diff = null;
    selected = new Set();
  }

  function mockLoadBooks() {
    const mockBooks: EpubInfo[] = [
      { filename: "great-gatsby.epub", title: "The Great Gatsby", author: "F. Scott Fitzgerald", sizeBytes: 1024000 },
      { filename: "pride-prejudice.epub", title: "Pride and Prejudice", author: "Jane Austen", sizeBytes: 1280000 },
      { filename: "mockingbird.epub", title: "To Kill a Mockingbird", author: "Harper Lee", sizeBytes: 1536000 },
      { filename: "1984.epub", title: "1984", author: "George Orwell", sizeBytes: 768000 },
      { filename: "catcher-rye.epub", title: "The Catcher in the Rye", author: "J.D. Salinger", sizeBytes: 896000 },
    ];
    diff = {
      toCopy: mockBooks,
      upToDate: [
        { filename: "already-synced.epub", title: "Already Synced Book", author: "Some Author", sizeBytes: 512000 },
      ],
    };
    selected = new Set(mockBooks.map((b) => b.filename));
    libraryError = null;
  }

  function mockClearBooks() {
    diff = { toCopy: [], upToDate: [] };
    selected = new Set();
  }

  function mockTransfer() {
    if (selected.size === 0 || !diff) return;
    syncBusy = true;
    syncError = null;
    const filenames = [...selected];
    const total = filenames.length;
    let done = 0;

    const interval = setInterval(() => {
      done++;
      syncProgress = {
        filename: filenames[done - 1],
        filesDone: done,
        filesTotal: total,
        bytesCopied: done * 1000000,
        bytesTotal: total * 1000000,
      };
      if (done >= total) {
        clearInterval(interval);
        setTimeout(() => {
          syncBusy = false;
          syncProgress = null;
          // Move copied books to upToDate
          if (diff) {
            const copied = diff.toCopy.filter((b) => selected.has(b.filename));
            diff = {
              toCopy: diff.toCopy.filter((b) => !selected.has(b.filename)),
              upToDate: [...diff.upToDate, ...copied],
            };
          }
          selected = new Set();
        }, 500);
      }
    }, 800);
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
          loadDiff();
        }
      })
      .catch((err) => {
        console.error("[epubl] get_config failed:", err);
        devMode = true;
        showWizard = true;
      });

    invoke<EReaderInfo[]>("get_connected_ereaders")
      .then((readers) => {
        device = readers[0] ?? null;
        if (device) loadDiff();
      })
      .catch((err) => console.error("[epubl] get_connected_ereaders failed:", err));

    const unlistenUpdate = listen<{ version: string }>("update-available", (e) => {
      updateVersion = e.payload.version;
    });
    const unlistenConnected = listen<EReaderInfo>("ereader-connected", (e) => {
      device = e.payload; ejectError = null; loadDiff();
    });
    const unlistenDisconnected = listen("ereader-disconnected", () => {
      device = null; ejectError = null; diff = null; selected = new Set();
    });

    return () => {
      unlistenUpdate.then((fn) => fn());
      unlistenConnected.then((fn) => fn());
      unlistenDisconnected.then((fn) => fn());
    };
  });
</script>

<!-- =========================================================
     SVG icons (inline to avoid dependency)
     ========================================================= -->
{#snippet bookOpenIcon(cls: string)}
  <svg class={cls} xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
       stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"/><path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"/>
  </svg>
{/snippet}

{#snippet usbIcon(cls: string)}
  <svg class={cls} xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
       stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <circle cx="10" cy="7" r="1"/><circle cx="4" cy="20" r="1"/><path d="M4.7 19.3 19 5"/><path d="m21 3-3 1 2 2Z"/>
    <circle cx="10" cy="7" r="1"/><path d="M10 7v4"/><path d="M12 12l-3 5"/>
  </svg>
{/snippet}

{#snippet uploadIcon(cls: string)}
  <svg class={cls} xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
       stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/>
  </svg>
{/snippet}

{#snippet externalLinkIcon(cls: string)}
  <svg class={cls} xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
       stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/>
    <line x1="10" y1="14" x2="21" y2="3"/>
  </svg>
{/snippet}

{#snippet alertCircleIcon(cls: string)}
  <svg class={cls} xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
       stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
  </svg>
{/snippet}

{#snippet checkCircleIcon(cls: string)}
  <svg class={cls} xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
       stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/>
  </svg>
{/snippet}

{#snippet wrenchIcon(cls: string)}
  <svg class={cls} xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
       stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>
  </svg>
{/snippet}

{#snippet trashIcon(cls: string)}
  <svg class={cls} xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
       stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
  </svg>
{/snippet}

{#snippet plusIcon(cls: string)}
  <svg class={cls} xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
       stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
  </svg>
{/snippet}

{#snippet unplugIcon(cls: string)}
  <svg class={cls} xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
       stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="m19 5 3-3"/><path d="m2 22 3-3"/><path d="M6.3 20.3a2.4 2.4 0 0 0 3.4 0L12 18l-6-6-2.3 2.3a2.4 2.4 0 0 0 0 3.4Z"/>
    <path d="M7.5 13.5 10 11"/><path d="M10.5 16.5 13 14"/>
    <path d="m12 6 6 6 2.3-2.3a2.4 2.4 0 0 0 0-3.4l-2.6-2.6a2.4 2.4 0 0 0-3.4 0Z"/>
  </svg>
{/snippet}

<!-- =========================================================
     First-run wizard
     ========================================================= -->
{#if showWizard}
  <div class="wizard-overlay" role="dialog" aria-modal="true" aria-labelledby="wizard-title">
    <header class="wizard-header">
      <div class="wizard-header-inner">
        {@render bookOpenIcon("header-icon")}
        <div>
          <h1>Old People Loadin'</h1>
          <p class="header-sub">This will help ya load your books bud.</p>
        </div>
      </div>
    </header>

    <div class="wizard">
      <div class="card">
        <div class="card-header">
          <div class="step-number">1</div>
          <div class="card-header-text">
            <h2 id="wizard-title">Configuration</h2>
            <p>Set up your ebook folder and support email</p>
          </div>
        </div>
        <div class="card-body" style="display: flex; flex-direction: column; gap: 16px;">
          <div>
            <label for="wizard-folder" style="font-size: 14px; font-weight: 500; color: #78350f;">
              Epub Folder
            </label>
            <div class="wizard-row" style="margin-top: 6px;">
              <input
                id="wizard-folder"
                class="wizard-input"
                type="text"
                placeholder="Select a folder…"
                bind:value={wizardFolder}
              />
              <button class="btn" onclick={pickFolder}>Browse…</button>
            </div>
            <p style="font-size: 12px; color: #92400e; margin-top: 4px;">
              The folder where your epub files are stored
            </p>
          </div>

          <div>
            <label for="wizard-email" style="font-size: 14px; font-weight: 500; color: #78350f;">
              Support Email
            </label>
            <input
              id="wizard-email"
              class="wizard-input"
              type="email"
              placeholder="your@email.com (optional)"
              bind:value={wizardEmail}
              style="margin-top: 6px;"
            />
            <p style="font-size: 12px; color: #92400e; margin-top: 4px;">
              Where problem reports will be sent
            </p>
          </div>

          {#if wizardError}
            <p class="wizard-error">{wizardError}</p>
          {/if}

          <button
            class="btn btn-primary btn-sync wizard-done"
            onclick={completeWizard}
            disabled={wizardBusy || !wizardFolder}
            style="width: auto;"
          >
            {wizardBusy ? "Saving…" : "Save Configuration"}
          </button>
        </div>
      </div>
    </div>
  </div>

{:else}
  <!-- =========================================================
       Main app
       ========================================================= -->
  <header class="header">
    <div class="header-inner">
      {@render bookOpenIcon("header-icon")}
      <div>
        <h1>Old People Loadin'</h1>
        <p class="header-sub">This will help ya load your books bud.</p>
      </div>
    </div>
  </header>

  <main class="main">
    {#if updateVersion}
      <div class="update-banner" role="alert">
        <span>Update available: v{updateVersion} —</span>
        <a href="https://github.com/vbonduro/epubl/releases/latest" target="_blank" rel="noopener noreferrer">
          Download
        </a>
        <button class="update-dismiss" onclick={() => (updateVersion = null)} aria-label="Dismiss">✕</button>
      </div>
    {/if}

    <!-- Mock controls (dev mode only) -->
    {#if devMode}
      <div class="mock-controls">
        <div class="mock-header">
          {@render wrenchIcon("mock-icon")}
          <span>Testing Controls</span>
        </div>
        <div class="mock-buttons">
          {#if !device}
            <button class="btn-mock" onclick={mockConnect}>
              {@render usbIcon("mock-btn-icon")}
              Mock: Connect eReader
            </button>
          {:else}
            <button class="btn-mock btn-mock-outline" onclick={mockDisconnect}>
              {@render unplugIcon("mock-btn-icon")}
              Mock: Disconnect
            </button>
          {/if}

          {#if !diff || (diff.toCopy.length === 0 && diff.upToDate.length === 0)}
            <button class="btn-mock" onclick={mockLoadBooks} disabled={!device}>
              {@render plusIcon("mock-btn-icon")}
              Mock: Add ePubs
            </button>
          {:else}
            <button class="btn-mock btn-mock-outline" onclick={mockClearBooks}>
              {@render trashIcon("mock-btn-icon")}
              Mock: Clear ePubs
            </button>
          {/if}

          {#if device && diff && diff.toCopy.length > 0 && selected.size > 0 && !syncBusy}
            <button class="btn-mock" onclick={mockTransfer}>
              {@render uploadIcon("mock-btn-icon")}
              Mock: Simulate Transfer
            </button>
          {/if}
        </div>
      </div>
    {/if}

    <!-- =====================================================
         Step 1: Download Books
         ===================================================== -->
    <div class="card">
      <div class="card-header">
        <div class="step-number">1</div>
        <div class="card-header-text">
          <h2>Download Books</h2>
          <p>Download books from the server so that they can be loaded onto your eReader below.</p>
        </div>
      </div>
      <div class="card-body">
        {#if config?.bookstoreUrl}
          <div class="bookstore-box">
            <p>Download books at the link below by clicking on the book you want then choosing Download: EPUB</p>
            <a class="bookstore-link" href={config.bookstoreUrl} target="_blank" rel="noopener noreferrer">
              {@render externalLinkIcon("alert-icon")}
              Visit Ebook Store
            </a>
          </div>
        {:else}
          <p class="placeholder">No bookstore URL configured.</p>
        {/if}
      </div>
    </div>

    <!-- =====================================================
         Step 2: Load Books onto eReader
         ===================================================== -->
    <div class="card">
      <div class="card-header">
        <div class="step-number">2</div>
        <div class="card-header-text">
          <h2>Load Books onto eReader</h2>
          <p>Connect your eReader and transfer your downloaded books</p>
        </div>
      </div>
      <div class="card-body">
        <!-- eReader connection status -->
        {#if !device}
          <div class="alert alert-waiting">
            {@render alertCircleIcon("alert-icon")}
            <span>Waiting for eReader… Please plug in your eReader via USB, then look at your eReader and press the connect button.</span>
          </div>
        {:else}
          <div class="alert alert-connected">
            {@render usbIcon("alert-icon")}
            <span>eReader connected successfully!</span>
            <!-- Hidden badge for E2E tests -->
            <span class="badge-connected">Connected</span>
          </div>
          <div class="device-info">
            <p class="device-model">{device.model}</p>
            <p class="device-drive">{device.driveLetter}</p>
          </div>
          <div class="eject-row">
            <button class="btn btn-eject" onclick={handleEject}>
              {@render unplugIcon("alert-icon")}
              Eject eReader
            </button>
          </div>

          {#if ejectError}
            <p class="eject-error">{ejectError}</p>
            {#if config?.supportEmail}
              <button class="btn btn-report" onclick={() => reportProblem(`Eject error: ${ejectError}`)}>
                Report problem
              </button>
            {/if}
          {/if}
          <!-- Book loading UI (only when connected) -->
          {#if syncBusy}
            <!-- Transfer in progress -->
            <div class="transfer-progress">
              {@render uploadIcon("transfer-icon")}
              <p class="transfer-title">Loading books onto your eReader…</p>
              <p class="transfer-sub">
                {syncProgress ? `${syncProgress.filesDone} of ${syncProgress.filesTotal}` : ''} book{selected.size !== 1 ? 's' : ''} being copied
              </p>
              <div class="progress-bar">
                <div class="progress-fill" style="width: {progressPct()}%"></div>
              </div>
              <p class="progress-pct">{progressPct()}%</p>
            </div>

          {:else if libraryError}
            <p class="error-text">{libraryError}</p>
            {#if config?.supportEmail}
              <button class="btn btn-report" onclick={() => reportProblem(`Library error: ${libraryError}`)}>
                Report problem
              </button>
            {/if}

          {:else if syncError}
            <div class="alert" style="background: #fef2f2; border: 1px solid #fca5a5; color: #dc2626;">
              {@render alertCircleIcon("alert-icon")}
              <span>{syncError}</span>
            </div>
            {#if config?.supportEmail}
              <button class="btn btn-report" style="margin-top: 12px;" onclick={() => reportProblem(`Sync error: ${syncError}`)}>
                Report problem
              </button>
            {/if}

          {:else if !diff}
            <p class="placeholder">Loading…</p>

          {:else if diff.toCopy.length === 0 && diff.upToDate.length === 0}
            <p class="placeholder">No epub files found in your library folder.</p>

          {:else if diff.toCopy.length === 0 && diff.upToDate.length > 0}
            <!-- All synced -->
            <div class="alert alert-done">
              {@render checkCircleIcon("alert-icon")}
              <span>All books have been loaded! Download more from your ebook store. Or hit the eject button above if done loading, then unplug the USB cable.</span>
            </div>

          {:else}
            <!-- Select all row -->
            <div class="select-all-row">
              <label class="select-all-label">
                <input
                  type="checkbox"
                  checked={selected.size === diff.toCopy.length && diff.toCopy.length > 0}
                  onchange={toggleSelectAll}
                />
                Select All ({diff.toCopy.length} book{diff.toCopy.length !== 1 ? 's' : ''})
              </label>
              <span class="selected-count">{selected.size} selected</span>
            </div>

            <!-- Book list -->
            <ul class="epub-list">
              {#each diff.toCopy as book (book.filename)}
                <li class="epub-item epub-item-new" class:is-selected={selected.has(book.filename)}>
                  <label class="epub-label">
                    <input
                      type="checkbox"
                      checked={selected.has(book.filename)}
                      onchange={() => toggleBook(book.filename)}
                    />
                    {@render bookOpenIcon("epub-book-icon")}
                    <div class="epub-text">
                      <span class="epub-title">{book.title}</span>
                      <p class="epub-meta">
                        {formatSize(book.sizeBytes)}{#if book.author} · {book.author}{/if}
                      </p>
                    </div>
                  </label>
                </li>
              {/each}
              {#each diff.upToDate as book (book.filename)}
                <li class="epub-item epub-item-synced">
                  <label class="epub-label">
                    <input type="checkbox" checked disabled />
                    {@render bookOpenIcon("epub-book-icon")}
                    <div class="epub-text">
                      <span class="epub-title">{book.title}</span>
                      <p class="epub-meta">
                        {formatSize(book.sizeBytes)}{#if book.author} · {book.author}{/if}
                      </p>
                    </div>
                  </label>
                  <span class="synced-badge">Synced</span>
                </li>
              {/each}
            </ul>

            <!-- Load button -->
            <button
              class="btn btn-sync"
              style="margin-top: 16px;"
              onclick={handleSync}
              disabled={syncBusy || selected.size === 0}
            >
              {@render uploadIcon("alert-icon")}
              {syncBusy ? "Syncing…" : `Load ${selected.size > 0 ? selected.size + ' ' : ''}Book${selected.size !== 1 ? 's' : ''}`}
            </button>
          {/if}
        {/if}
      </div>
    </div>
  </main>

  <footer class="footer">
    <p>Made with care for easy reading</p>
  </footer>
{/if}
