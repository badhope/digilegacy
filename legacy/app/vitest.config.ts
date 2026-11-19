import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
import { readFileSync } from 'node:fs';

const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf-8')) as { version: string };

// 前端单元测试配置（vitest 5 · jsdom）
// 说明：测试文件内显式 import { describe, it, expect } from 'vitest'，
// 不依赖 tsconfig 的 globals 类型注入，避免影响 vue-tsc 对业务代码的检查。
export default defineConfig({
  plugins: [vue()],
  // 与 vite.config.ts 对齐：业务代码直接读构建期注入的 __APP_VERSION__ / __DEMO_MODE__，
  // 测试环境缺这些 define 会让任何挂载 App.vue 的用例直接 ReferenceError。
  define: { __APP_VERSION__: JSON.stringify(pkg.version), __DEMO_MODE__: 'false' },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.{test,spec}.ts'],
    // 每个测试文件都会新建一次 jsdom（~1.5s/实例）。并发过高在内存受限的机器上
    // 会让 worker 进程 OOM 崩溃（表现为 "Worker forks emitted error"），
    // 限制并发换取稳定性；CI runner 内存更小，同样受益。
    maxWorkers: 2,
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json-summary'],
      // 关键：不设 include 时 v8 只统计「被测试 import 过的文件」，
      // 从没被加载的视图在报表里根本不存在 —— 分母变小，门禁变成假绿灯。
      // 这里显式包含全部源码，让未覆盖的文件以 0% 计入分母。
      include: ['src/**/*.{ts,vue}'],
      exclude: [
        'src/**/*.test.ts',
        'src/**/*.d.ts',
        'src/main.ts',
        'src/types/**',
        // 浏览器预览专用后端，不进生产包（见 README「Test coverage」），
        // 计入分母会掩盖真实产品覆盖率。
        'src/api/mock.ts',
        'src/api/mock-platforms.json',
        // 测试辅助代码同样不属于产品代码
        'src/test-utils/**',
      ],
      // 阈值按「真实产品口径」设定，并预留逐步提升空间。
      thresholds: {
        // 补完 App / ReportView / AssetDetail 后的实测：79.94 / 79.75 / 73.41 / 81.11
        statements: 78,
        branches: 78,
        functions: 71,
        lines: 79,
      },
    },
  },
});
