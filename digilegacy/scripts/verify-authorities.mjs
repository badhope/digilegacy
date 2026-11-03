#!/usr/bin/env node
// 权威来源核验：逐条实测 URL，并区分两种不同的事实。
//
// 这是本项目引用体系的核心纪律，因为历史上踩过两个坑：
//   1. 只有公众号出处的条目被当成「有来源」；
//   2. 手写的 gov.cn 链接实际 404，但没人发现。
//
// 因此每条来源记录两个**互相独立**的结论：
//   - reachable       ：HTTP 可达（200/403）。只证明站点/页面存在。
//   - content_matched ：在返回的 HTML 里真的找到了要引用的关键短语。
//                       **这才是「这条链接支持这条引用」的证据。**
//
// 政务站点常见情况：页面可达、但正文在 iframe/PDF/JS 里，抓不到关键短语。
// 此时 reachable=true、content_matched=false —— 必须如实标注为
// 「入口可达，内容需人工核对」，绝不能当成已验证。
//
// 用法：
//   node scripts/verify-authorities.mjs            # 只检查
//   node scripts/verify-authorities.mjs --write    # 把结果写回 JSON

import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { WEB } from './doc-facts-lib.mjs';

const FILE = path.join(WEB, 'src', 'data', 'authorities.json');
const write = process.argv.includes('--write');
const TIMEOUT_MS = 20000;
const RETRIES = 2;

const list = JSON.parse(readFileSync(FILE, 'utf8'));

function stripHtml(html) {
  return html
    .replace(/<script[\s\S]*?<\/script>/gi, ' ')
    .replace(/<style[\s\S]*?<\/style>/gi, ' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/g, ' ')
    .replace(/\s+/g, ' ');
}

function extractTitle(html) {
  const m = /<title[^>]*>([\s\S]*?)<\/title>/i.exec(html);
  return m ? stripHtml(m[1]).trim().slice(0, 80) : '';
}

async function probe(url) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    const res = await fetch(url, {
      redirect: 'follow',
      signal: controller.signal,
      headers: {
        'User-Agent':
          'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36',
        Accept: 'text/html,application/xhtml+xml',
      },
    });
    const html = res.status === 200 ? await res.text() : '';
    return {
      status: res.status,
      finalUrl: res.url,
      title: extractTitle(html),
      text: res.status === 200 ? stripHtml(html) : '',
      error: '',
    };
  } catch (e) {
    return { status: 0, finalUrl: url, title: '', text: '', error: String(e).slice(0, 120) };
  } finally {
    clearTimeout(timer);
  }
}

function reachable(status) {
  // 403 表示站点存在但拒绝脚本抓取，链接本身仍有效
  return status === 200 || status === 403;
}

/** 错误页识别：政务站点出错时常常仍返回 200，标题是「哎呀，出错了」之类。 */
const ERROR_PAGE_MARKERS = ['出错了', '页面不存在', '404', 'Not Found', '访问出错', '系统繁忙'];

function looksLikeErrorPage(title, text) {
  const head = `${title} ${text.slice(0, 200)}`;
  return ERROR_PAGE_MARKERS.some(m => head.includes(m));
}

async function main() {
  const results = [];
  for (const a of list) {
    // 政务站点常偶发超时；单次失败不足以判「死链」，重试后再下结论。
    let r = await probe(a.url);
    for (let i = 0; i < RETRIES && !reachable(r.status); i++) {
      await new Promise(res => setTimeout(res, 800));
      r = await probe(a.url);
    }
    // 「200 + 错误页」必须当成不可用：这是政务站点最常见的假可达。
    const ok = reachable(r.status) && !looksLikeErrorPage(r.title, r.text);
    // 关键短语：由每条来源自己声明，脚本只负责「找得到 / 找不到」
    const needles = a.content_needles ?? [];
    // 内容匹配只认「正文里出现该短语」；错误页一律不算匹配。
    const matched =
      needles.length > 0 && !looksLikeErrorPage(r.title, r.text) && needles.every(n => r.text.includes(n));
    results.push({ id: a.id, ok, matched, needles, ...r });
    if (write) {
      a.reachable = ok;
      a.content_matched = matched;
      a.last_checked = new Date().toISOString().slice(0, 10);
      a.http_status = r.status;
      a.observed_title = r.title;
    }
  }

  const okCount = results.filter(r => r.ok).length;
  const matchedCount = results.filter(r => r.matched).length;
  // 全部不可达通常意味着本机没网，而不是所有来源同时下线。
  // 这时必须说清楚，否则用户会去逐个怀疑来源。
  const allDown = okCount === 0;
  if (allDown) {
    console.log('权威来源核验：全部无法连接 —— 本机网络不可用或所有站点同时不可达。');
    console.log('  这不是「来源失效」的结论，请联网后重跑；本次结果不写入 JSON。');
    process.exit(2);
  }
  console.log(`权威来源核验：可达 ${okCount}/${results.length} · 内容匹配 ${matchedCount}/${results.length}`);
  for (const r of results) {
    const reach = r.ok ? 'OK  ' : 'FAIL';
    const match = r.matched ? '内容匹配' : `内容未匹配(需人工核对)`;
    console.log(`  ${reach} ${String(r.status).padStart(3)} ${r.id.padEnd(20)} ${match}  ${r.title || r.error || ''}`);
  }

  if (write) {
    writeFileSync(FILE, JSON.stringify(list, null, 2) + '\n', 'utf8');
    console.log('\n已写回 authorities.json');
  }

  // 只把「不可达」当失败；「内容未匹配」不算失败，但会打印出来要求人工确认。
  process.exit(okCount === results.length ? 0 : 1);
}

main();
