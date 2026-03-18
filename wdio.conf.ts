import type { Options } from '@wdio/types'

export const config: Options.Testrunner = {
  runner: 'local',
  autoCompileOpts: { autoCompile: true, tsNodeOpts: { project: './tsconfig.e2e.json' } },
  specs: ['./e2e/specs/**/*.spec.ts'],
  exclude: [],
  maxInstances: 1,
  capabilities: [{
    'tauri:options': {
      application: process.env.EPUBL_BIN ?? './src-tauri/target/release/epubl',
    },
  }],
  logLevel: 'info',
  bail: 0,
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,
  services: ['tauri'],
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 60000 },
}
