import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { readFileSync } from "node:fs";
import istanbul from "vite-plugin-istanbul";

const pkg = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf-8")) as { version: string };

// E2E 覆盖率采集：只有显式设置 E2E_COVERAGE=1 时才注入 istanbul 插桩，
// 生产构建完全不受影响（不增加包体积、不拖慢编译）。
const e2eCoverage = process.env.E2E_COVERAGE === "1";

export default defineConfig(async () => ({
  plugins: [
    vue(),
    ...(e2eCoverage
      ? [
          istanbul({
            include: ["src/**/*.{ts,vue}"],
            exclude: ["src/**/*.test.ts", "src/test-utils/**", "src/api/mock.ts", "src/types/**"],
            extension: [".ts", ".vue"],
            requireEnv: false,
            forceBuildInstrument: true,
          }),
        ]
      : []),
  ],
  clearScreen: false,
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
    // 演示构建要把 mock 后端打进去；生产桌面构建保持 false，mock 不进包。
    __DEMO_MODE__: JSON.stringify(process.env.VITE_DEMO === "1"),
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/target/**", "**/node_modules/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  // 在线演示：构建产物放在 /digilegacy/demo/ 下，静态托管即可点开体验。
  base: process.env.VITE_DEMO === "1" ? "/digilegacy/demo/" : "/",
  build: {
    target: process.env.TAURI_PLATFORM == "windows" ? "chrome105" : "safari13",
    minify: process.env.TAURI_DEBUG ? false : "oxc",
    sourcemap: !!process.env.TAURI_DEBUG || e2eCoverage,
  },
}));
