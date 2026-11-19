import { defineConfig, devices } from '@playwright/test';

// E2E：跑在浏览器预览模式（vite dev + src/api/mock.ts）。
//
// 为什么不跑真实 Tauri 窗口：CI 是 Linux 无窗口环境，且 WebView2 无法在
// GitHub Actions 里启动。预览模式复用了与生产完全相同的 Vue 组件、store
// 与路由；唯一替换的是 IPC 后端（mock 实现了同一套命令契约）。
//
// 覆盖目标：7 个此前单测 0% 的视图的真实交互链路。
export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  // 采集覆盖率时每页都跑插桩代码，并发过高会让 Chromium 进程 OOM
  // （表现为 worker exited unexpectedly / code=3221226505）。
  // 本机 17GB 内存 + 单进程 Edge 通道，4 worker 并发时 Chromium 会直接 OOM
  // （worker exited code=134，不是断言失败）。CI 仍用 1；本地压到 2，稳定性优先。
  workers: process.env.CI || process.env.E2E_COVERAGE === '1' ? 1 : 2,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  timeout: 30_000,
  expect: { timeout: 8_000 },

  use: {
    baseURL: 'http://localhost:1420',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    viewport: { width: 1200, height: 800 },
  },

  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        // 本机/受限网络环境可用系统 Edge（同为 Chromium 内核）跑 E2E：
        //   PW_CHANNEL=msedge npx playwright test
        // 不设置时保持默认（CI 用 playwright install 下载的 chromium）。
        ...(process.env.PW_CHANNEL ? { channel: process.env.PW_CHANNEL } : {}),
      },
    },
  ],

  // E2E 覆盖率：设 E2E_COVERAGE=1 时启用（vite 侧同步插桩）
  globalTeardown: process.env.E2E_COVERAGE === '1' ? './e2e/coverage-teardown.mjs' : undefined,

  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
