// 演示站构建后补一个 robots.txt：Vite 的 publicDir 没有被用于演示构建，
// 直接写入 dist 比在仓库里维护一份与产物不同步的副本更可靠。
import { writeFileSync } from 'node:fs';
import path from 'node:path';

const target = path.join(process.cwd(), 'dist', 'robots.txt');
writeFileSync(target, 'User-agent: *\nAllow: /\n', 'utf8');
console.log(`wrote ${path.relative(process.cwd(), target)}`);
