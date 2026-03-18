import type { Options } from '@wdio/types'
import { spawn, type ChildProcess } from 'child_process'
import { setTimeout as sleep } from 'timers/promises'

const appBin = process.env.EPUBL_BIN ?? './src-tauri/target/release/epubl'

let tauriDriver: ChildProcess

export const config: Options.Testrunner = {
  runner: 'local',
  autoCompileOpts: {
    autoCompile: true,
    tsNodeOpts: { project: './tsconfig.e2e.json', esm: false },
  },
  specs: ['./e2e/specs/**/*.spec.ts'],
  exclude: [],
  maxInstances: 1,

  capabilities: [{
    browserName: 'wry',
    // Disable wdio v9 BiDi negotiation — tauri-driver rejects webSocketUrl:true
    'wdio:enforceWebDriverClassic': true,
    'webkitgtk:browserOptions': { binary: appBin },
  }],

  logLevel: 'info',
  bail: 0,
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,

  // Launch tauri-driver before the test session and shut it down after.
  // tauri-driver is a WebDriver server that wraps WebKitWebDriver on Linux.
  onPrepare: async () => {
    // tauri-driver is on PATH when installed via cargo install
    tauriDriver = spawn(
      'tauri-driver',
      [],
      { stdio: [null, process.stdout, process.stderr] },
    )
    // Wait for tauri-driver to be ready before workers start connecting
    await sleep(2000)
  },

  onComplete: () => {
    tauriDriver?.kill()
  },

  // tauri-driver listens on localhost:4444 by default
  hostname: 'localhost',
  port: 4444,
  path: '/',

  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 60000 },
}
