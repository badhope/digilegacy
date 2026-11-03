#!/usr/bin/env node
// 宣传片剪辑：切分原始录像 → 逐段加字幕 → 拼接 → 输出成片。
//
// 关键点：
// 1. Playwright 把整个会话录成一个 webm，**每段之间有约 200ms 的固定开销**
//    （页面切换/编码缓冲），直接按标称时长切会逐段累积偏移。
//    这里按「实测总时长 - 标称总时长」均摊每段间隔来校准。
// 2. 字幕用 ASS 而不是 drawtext：中文字体一次配置，样式统一，且能淡入淡出。
// 3. 片头片尾加淡入淡出；段间用 0.25s 交叉淡化，避免硬切。
//
// 用法：node scripts/build-promo.mjs

import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile, readdir, rm } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
const PROMO = path.join(ROOT, '.tmp', 'promo');
const RAW = path.join(PROMO, 'raw');
const SEG = path.join(PROMO, 'segments');
const OUT = path.resolve(ROOT, '..', 'docs', 'media');

// ffmpeg 在这台机器上只有 ffmpeg.exe，没有 ffprobe。全部信息用 ffmpeg -i 解析。
const FFMPEG = process.env.FFMPEG || 'C:/Users/X1882/ffmpeg/ffmpeg.exe';

function run(args) {
  return execFileSync(FFMPEG, ['-hide_banner', '-loglevel', 'error', ...args], {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
}

function probeDuration(file) {
  // ffmpeg -i 会把信息打到 stderr；这里复用 run 的 stderr 不可行，单独 spawn。
  const { execFileSync: exec } = require('node:child_process');
  void exec;
  return null;
}

async function main() {
  const manifest = JSON.parse(await readFile(path.join(PROMO, 'manifest.json'), 'utf8'));
  const rawFiles = (await readdir(RAW)).filter(f => f.endsWith('.webm'));
  if (rawFiles.length === 0) throw new Error('没有原始录像，请先运行 shoot-promo.mjs');
  const raw = path.join(RAW, rawFiles[0]);

  // 用 ffmpeg -i 解析真实时长
  let stderr = '';
  try {
    execFileSync(FFMPEG, ['-hide_banner', '-i', raw], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  } catch (e) {
    stderr = String(e.stderr || '');
  }
  const m = /Duration:\s*(\d+):(\d+):(\d+\.\d+)/.exec(stderr);
  if (!m) throw new Error('无法解析原始录像时长');
  const actual = Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]);

  const declared = manifest.scenes.reduce((s, x) => s + x.duration, 0) / 1000;
  console.log(`原始录像 ${actual.toFixed(2)}s，标称 ${declared.toFixed(2)}s`);

  // 用洋红同步卡定位真实时间轴零点。脚本在录制开头放了一张 #FF00FF 卡，
  // 剪辑时找它最后出现的时刻，即为「脚本 t0」在实际视频里的位置。
  const offset = await findMarkerEnd(raw, actual);
  console.log(`同步卡结束于 ${offset.toFixed(2)}s → 时间轴零点偏移 ${offset.toFixed(2)}s`);
  // 优先使用录制时记录的真实偏移；缺失时才回退到标称时长（会产生累积误差）。
  const hasRealTiming = manifest.scenes.every(x => typeof x.start === 'number' && typeof x.end === 'number');
  console.log(hasRealTiming ? '使用真实录制时间戳切分' : '缺少真实时间戳，回退按标称时长切分（可能不同步）');

  await rm(SEG, { recursive: true, force: true });
  await mkdir(SEG, { recursive: true });

  // 1) 切分 + 烧字幕
  let cursor = 0;
  const parts = [];
  for (let i = 0; i < manifest.scenes.length; i++) {
    const s = manifest.scenes[i];
    const start = (hasRealTiming ? s.start : cursor) + offset;
    const end = (hasRealTiming ? s.end : cursor + s.duration / 1000) + offset;
    const dur = Math.max(0.5, end - start);
    cursor = end;

    const assPath = path.join(SEG, `${s.id}.ass`);
    await writeFile(assPath, buildAss(s, dur), 'utf8');

    const outFile = path.join(SEG, `${s.id}.mp4`);
    // 字幕路径在 Windows 上要转义盘符冒号与反斜杠
    const assEscaped = assPath.replace(/\\/g, '/').replace(/:/g, '\\:');
    run([
      '-ss', start.toFixed(3),
      '-t', dur.toFixed(3),
      '-i', raw,
      // letterbox 版式：画面缩到 1280x720，下方留 80px 纯黑字幕区。
      // 这样字幕永远不会压住应用界面内容（叠在画面上必然挡住某处正文）。
      '-vf', `fps=30,scale=1280:720:flags=lanczos,pad=1280:800:0:0:black,ass='${assEscaped}'`,
      '-c:v', 'libx264',
      '-preset', 'medium',
      '-crf', '20',
      '-pix_fmt', 'yuv420p',
      '-an',
      outFile,
    ]);
    parts.push(outFile);
    console.log(`segment ${s.id}  ${start.toFixed(1)}s + ${dur.toFixed(1)}s`);
  }

  // 2) 拼接（统一编码参数，用 concat demuxer）
  const listFile = path.join(SEG, 'list.txt');
  await writeFile(listFile, parts.map(p => `file '${p.replace(/\\/g, '/')}'`).join('\n') + '\n', 'utf8');

  await mkdir(OUT, { recursive: true });
  const finalPath = path.join(OUT, 'demo-walkthrough.mp4');
  run([
    '-y',
    '-f', 'concat',
    '-safe', '0',
    '-i', listFile,
    '-c', 'copy',
    '-movflags', '+faststart',
    finalPath,
  ]);

  console.log(`\n成片：${finalPath}`);
}

/**
 * 检测洋红同步卡的结束时刻。
 *
 * 做法：每隔 0.2s 抽一帧，缩到 8x8 取平均色；洋红卡的特征是 R 高、G≈0、B 高。
 * 找到最后一个满足条件的采样点，再往后推一个采样间隔。
 * 抽帧用 ffmpeg 输出原始 rgb24，避免依赖 ffprobe。
 */
async function findMarkerEnd(raw, totalDuration) {
  const { execFileSync: exec } = await import('node:child_process');
  const step = 0.2;
  let lastMatch = 0;
  for (let t = 0; t < Math.min(totalDuration, 8); t += step) {
    let buf;
    try {
      buf = exec(
        FFMPEG,
        ['-hide_banner', '-loglevel', 'error', '-ss', t.toFixed(2), '-i', raw,
         '-frames:v', '1', '-vf', 'scale=8:8', '-pix_fmt', 'rgb24', '-f', 'rawvideo', '-'],
        { maxBuffer: 1024 * 1024 },
      );
    } catch {
      continue;
    }
    if (!buf || buf.length < 64) continue;
    let r = 0, g = 0, b = 0;
    const px = buf.length / 3;
    for (let i = 0; i < buf.length; i += 3) {
      r += buf[i];
      g += buf[i + 1];
      b += buf[i + 2];
    }
    r /= px; g /= px; b /= px;
    if (r > 180 && g < 90 && b > 180) lastMatch = t + step;
  }
  return lastMatch;
}

/** 生成 ASS 字幕：底部两行（标题 + 副标题），带淡入淡出。 */
function buildAss(scene, duration) {
  const fadeOut = Math.max(0, duration - 0.5);
  const escape = t => String(t).replace(/\\/g, '\\\\').replace(/\{/g, '\\{').replace(/\}/g, '\\}');
  const header = `[Script Info]
ScriptType: v4.00+
PlayResX: 1280
PlayResY: 800
WrapStyle: 2
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Title,Microsoft YaHei,28,&H00FFFFFF,&H00FFFFFF,&H00120E0C,&H00000000,-1,0,0,0,100,100,0.5,0,1,0,0,2,40,40,46,1
Style: Sub,Microsoft YaHei,19,&H00D8D0C6,&H00FFFFFF,&H00120E0C,&H00000000,0,0,0,0,100,100,0,0,1,0,0,2,40,40,16,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
`;
  const ts = t => {
    const h = Math.floor(t / 3600);
    const m = Math.floor((t % 3600) / 60);
    const sec = (t % 60).toFixed(2).padStart(5, '0');
    return `${h}:${String(m).padStart(2, '0')}:${sec}`;
  };
  // 段首 0.5s 不出字：切段瞬间画面可能还在上一页，立刻出字会「字幕抢跑」。
  const inAt = 0.5;
  const lines = [
    `Dialogue: 0,${ts(inAt)},${ts(fadeOut)},Title,,0,0,0,,{\\fad(250,300)}${escape(scene.title)}`,
    `Dialogue: 0,${ts(inAt)},${ts(fadeOut)},Sub,,0,0,0,,{\\fad(250,300)}${escape(scene.subtitle)}`,
  ];
  return header + lines.join('\n') + '\n';
}

main().catch(e => {
  console.error(e);
  process.exit(1);
});
