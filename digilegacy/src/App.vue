<template>
  <div class="app-container">
    <!-- 侧边栏 -->
    <aside class="sidebar">
      <div class="sidebar-header">
        <div class="sidebar-logo">
          <!-- 品牌标：双环 + 圆心，与 app icon / favicon 保持同一套几何 -->
          <svg class="logo-mark" viewBox="0 0 40 40" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
            <rect x="1" y="1" width="38" height="38" rx="9" fill="url(#digilegacy-logo)"/>
            <circle cx="20" cy="20" r="11.6" stroke="#fff" stroke-width="2.1" opacity="0.95"/>
            <circle cx="20" cy="20" r="7.2" stroke="#fff" stroke-width="1.2" opacity="0.6"/>
            <circle cx="20" cy="20" r="2.9" fill="#fff"/>
            <defs>
              <linearGradient id="digilegacy-logo" x1="20" y1="1" x2="20" y2="39" gradientUnits="userSpaceOnUse">
                <stop stop-color="#c46a5c"/>
                <stop offset="1" stop-color="#9a473b"/>
              </linearGradient>
            </defs>
          </svg>
          <div class="logo-wordmark">
            <span class="logo-name">归零</span>
            <span class="logo-sub">DigiLegacy</span>
          </div>
        </div>
      </div>
      
      <nav class="sidebar-nav">
        <!-- 按「家人交付旅程」分组：用户不需要先理解 9 个功能，只需要知道自己在哪一段 -->
        <div v-for="group in navGroups" :key="group.id" class="nav-group">
          <div class="nav-group-label">{{ t(group.labelKey) }}</div>
          <router-link
            v-for="route in group.routes"
            :key="route.path"
            :to="route.path"
            class="nav-item"
            :class="{ active: currentRoute === route.path }"
            :data-tour="'nav-' + route.path.slice(1)"
          >
            <span class="nav-icon" aria-hidden="true"><component :is="route.icon" /></span>
            <span>{{ t(route.titleKey) }}</span>
          </router-link>
        </div>
      </nav>
      
      <div class="sidebar-footer">
        <div class="sidebar-security">
          <router-link v-if="!security.hasMasterPassword" to="/profile" style="color: var(--warning); text-decoration: none;"><component :is="WarningOutline" aria-hidden="true" /> {{ t('app.noEncryption') }}</router-link>
          <span v-else>{{ t('app.encryptedLocal') }}</span>
        </div>
        <div class="sidebar-brandline">
          <span>v{{ appVersion }} · {{ t('common.openSource') }}</span>
        </div>
      </div>
    </aside>

    <!-- 主内容区 -->
    <div class="main-layout">
      <header class="main-header">
        <div class="header-title">{{ currentPageTitle }}</div>
        <div class="header-actions">
          <!-- 命令面板入口：把 ⌘K 这条隐形快捷键变成可见入口，
               否则只有读过文档的用户才知道它存在。 -->
          <button
            class="btn btn-sm btn-secondary cmdk-trigger"
            data-testid="cmdk-open"
            :aria-label="t('cmdk.openHint')"
            :title="t('cmdk.openHint')"
            @click="showPalette = true"
          >
            <component :is="SearchOutline" aria-hidden="true" />
            <span class="cmdk-trigger-text">{{ t('cmdk.placeholder') }}</span>
            <kbd class="cmdk-kbd cmdk-trigger-kbd">{{ isMac ? '⌘' : 'Ctrl' }}K</kbd>
          </button>
          <button class="btn btn-sm btn-secondary" data-testid="theme-switch" :aria-label="themeLabel" :title="themeLabel" @click="toggleTheme">
            <component :is="themeIcon" aria-hidden="true" />
          </button>
          <button class="btn btn-sm btn-secondary" data-testid="lang-switch" :title="locale === 'zh-CN' ? 'Switch to English' : '切换到中文'" @click="toggleLocale">
            <component :is="LanguageOutline" aria-hidden="true" /> {{ locale === 'zh-CN' ? 'EN' : '中文' }}
          </button>
          <button class="btn btn-sm btn-secondary" data-tour="help" @click="showHelp = true"><component :is="HelpCircleOutline" aria-hidden="true" /> {{ t('app.help') }}</button>
          <button class="btn btn-sm btn-secondary" v-if="security.hasMasterPassword && security.unlocked" @click="handleLock"><component :is="LockClosedOutline" aria-hidden="true" /> {{ t('app.lock') }}</button>
          <button class="btn btn-sm" data-tour="checkin" :class="checkBtnClass" @click="handleCheckIn">
            <component v-if="checkBtnIcon" :is="checkBtnIcon" aria-hidden="true" /> {{ checkBtnText }}
          </button>
        </div>
      </header>
      
      <main class="main-content">
        <div class="content-wrap">
          <!-- 浏览器演示横幅：明确告知这是演示数据，不是真实保险库 -->
          <div v-if="demoMode" class="demo-banner" role="note">
            <component :is="FlaskOutline" aria-hidden="true" />
            <span>
              {{ locale === 'zh-CN' ? t('app.demoBannerZh') : t('app.demoBannerEn') }}<br>
              <span :lang="locale === 'zh-CN' ? 'en' : 'zh'">{{ locale === 'zh-CN' ? t('app.demoBannerEn') : t('app.demoBannerZh') }}</span>
            </span>
          </div>
          <!--
            路由切换刻意**不用** <transition mode="out-in">。
            原因：每个视图根节点自带 `fade-in` 动画（animation: fadeIn ... forwards），
            与 Vue 的 enter/leave 钩子会在同一元素上抢同一条 opacity 通道。在
            `prefers-reduced-motion: reduce` 下所有 transition-duration 被压到
            0.01ms，Vue 的 after-leave/after-enter 回调与实际帧不同步，
            `fade-enter-from` 会被永久留在元素上（opacity: 0）——对开了「减少动画」
            的用户，页面内导航直接变成白屏。视图自己的 fade-in 已经提供了入场效果，
            这里只需要一个稳定的组件切换点。 -->
          <router-view v-slot="{ Component }">
            <component :is="Component" />
          </router-view>
        </div>
      </main>
    </div>

    <!-- 锁定遮罩：模态对话框语义 + 焦点困留，键盘/读屏用户不得穿透到遮罩后的功能 -->
    <div v-if="security.hasMasterPassword && !security.unlocked" class="lock-overlay">
      <div class="lock-card" role="dialog" aria-modal="true" aria-labelledby="lock-title" ref="lockCardRef">
        <template v-if="!recoveryMode">
          <div class="lock-hero-icon" aria-hidden="true"><component :is="LockClosedOutline" /></div>
          <h2 id="lock-title" style="font-size: 18px; font-weight: 700; margin-top: 8px;">{{ t('lock.title') }}</h2>
          <p class="text-muted" style="font-size: 13px; margin-top: 8px;">{{ t('lock.desc') }}</p>
          <input class="form-input" type="password" v-model="unlockPassword" :aria-label="t('lock.passwordLabel')" :placeholder="t('lock.passwordPlaceholder')" style="margin-top: 16px;" @keyup.enter="handleUnlock" />
          <p v-if="unlockError" role="alert" style="color: var(--danger); font-size: 13px; margin-top: 8px;">{{ unlockError }}</p>
          <button class="btn btn-primary" style="margin-top: 16px; width: 100%;" :disabled="security.busy" @click="handleUnlock">{{ security.busy ? t('lock.verifying') : t('app.unlock') }}</button>
          <button v-if="!demoMode" class="btn btn-secondary btn-sm" style="margin-top: 10px; width: 100%;" @click="recoveryMode = true">{{ t('lock.forgot') }}</button>
          <button v-if="!demoMode" class="btn btn-secondary btn-sm" style="margin-top: 8px; width: 100%;" @click="shareMode = true; recoveryMode = false">{{ t('kit.lockEntry') }}</button>
          <p class="text-muted" style="font-size: 12px; margin-top: 12px;">{{ t('lock.hint') }}</p>
        </template>
        <template v-else>
          <div class="lock-hero-icon" aria-hidden="true"><component :is="KeyOutline" /></div>
          <h2 id="lock-title" style="font-size: 18px; font-weight: 700; margin-top: 8px;">{{ t('recovery.title') }}</h2>
          <p class="text-muted" style="font-size: 13px; margin-top: 8px;">{{ t('recovery.desc') }}</p>
          <input class="form-input" v-model="recoveryCode" :aria-label="t('recovery.codeLabel')" :placeholder="t('recovery.codePlaceholder')" style="margin-top: 16px;" @keyup.enter="handleStartRecovery" />
          <button v-if="!recoveryUnlocked" class="btn btn-primary" style="margin-top: 12px; width: 100%;" :disabled="recoveryBusy" @click="handleStartRecovery">{{ recoveryBusy ? t('lock.verifying') : t('recovery.verify') }}</button>
          <template v-if="recoveryUnlocked">
            <input class="form-input" type="password" v-model="recoveryNewPassword" data-autofocus-pw :aria-label="t('recovery.newPasswordLabel')" :placeholder="t('recovery.newPasswordPlaceholder')" style="margin-top: 12px;" />
            <input class="form-input" type="password" v-model="recoveryNewPassword2" :aria-label="t('recovery.confirmPasswordLabel')" :placeholder="t('recovery.confirmPasswordPlaceholder')" style="margin-top: 12px;" />
            <p v-if="recoveryMessage" role="alert" style="font-size: 13px; margin-top: 8px;" :style="{ color: recoveryOk ? 'var(--success)' : 'var(--danger)' }" class="recovery-msg">{{ recoveryMessage }}</p>
            <button class="btn btn-primary" style="margin-top: 12px; width: 100%;" :disabled="recoveryBusy" @click="handleCompleteRecovery">{{ recoveryBusy ? t('recovery.processing') : t('recovery.reset') }}</button>
          </template>
          <!-- 新恢复密钥必须在重设成功后仍然可见：旧码一次性失效，
               若此处随 recoveryUnlocked 一起隐藏，用户将永远拿不到新码。
               并且只能由用户显式确认关闭——24 字符密钥不可能在倒计时内抄完。 -->
          <template v-else-if="recoveryNewCode">
            <p class="recovery-code-box" role="alert">{{ t('recovery.newCode', { code: recoveryNewCode }) }}</p>
            <label class="recovery-ack">
              <input type="checkbox" v-model="recoveryNewCodeAck" />
              {{ t('recovery.ack') }}
            </label>
            <button
              class="btn btn-primary"
              style="margin-top: 12px; width: 100%;"
              :disabled="!recoveryNewCodeAck"
              @click="finishRecovery"
            >{{ t('recovery.savedClose') }}</button>
          </template>
          <p v-else-if="recoveryError" role="alert" style="color: var(--danger); font-size: 13px; margin-top: 8px;">{{ recoveryError }}</p>
          <!-- 新恢复密钥展示期间隐藏「返回」：该按钮会离开恢复流程且不清空 recoveryNewCode，
               用户点一下就把唯一一份新码永久丢掉。只能用下方的显式确认按钮退出。 -->
          <button v-if="!recoveryNewCode" class="btn btn-secondary btn-sm" style="margin-top: 10px; width: 100%;" @click="backToUnlock">{{ t('recovery.backToUnlock') }}</button>
        </template>

        <!-- 继承人恢复份额解锁：失能场景下家人走这条路径 -->
        <template v-if="shareMode">
          <div class="lock-hero-icon" aria-hidden="true"><component :is="KeyOutline" /></div>
          <h2 id="lock-title" style="font-size: 18px; font-weight: 700; margin-top: 8px;">{{ t('kit.lockTitle') }}</h2>
          <p class="text-muted" style="font-size: 13px; margin-top: 8px;">{{ t('kit.lockDesc') }}</p>
          <textarea
            v-for="(_, i) in shareInputs"
            :key="i"
            class="form-input"
            rows="2"
            v-model="shareInputs[i]"
            :aria-label="t('kit.lockPlaceholder')"
            :placeholder="t('kit.lockPlaceholder')"
            style="margin-top: 10px; font-family: ui-monospace, Consolas, monospace; font-size: 12px;"
          ></textarea>
          <button class="btn btn-secondary btn-sm" style="margin-top: 8px; width: 100%;" @click="shareInputs.push('')">{{ t('kit.addShare') }}</button>
          <input class="form-input" type="password" v-model="sharePassphrase" :placeholder="t('kit.passphrase')" style="margin-top: 10px;" autocomplete="off" />
          <p v-if="shareError" role="alert" style="color: var(--danger); font-size: 13px; margin-top: 8px;">{{ shareError }}</p>
          <p v-if="shareMessage" role="status" style="color: var(--success); font-size: 13px; margin-top: 8px;">{{ shareMessage }}</p>
          <button class="btn btn-primary" style="margin-top: 12px; width: 100%;" :disabled="shareBusy" @click="handleShareUnlock">{{ shareBusy ? t('kit.unlocking') : t('kit.unlockShare') }}</button>
          <button class="btn btn-secondary btn-sm" style="margin-top: 10px; width: 100%;" @click="shareMode = false; shareError = ''">{{ t('recovery.backToUnlock') }}</button>
        </template>
      </div>
    </div>

    <!-- 新手引导与帮助中心 -->
    <OnboardingTour v-if="showTour" @close="showTour = false" />
    <HelpDrawer v-if="showHelp" @close="showHelp = false" @replay-tour="startTour" />

    <!-- 全局反馈层：Toast + 确认框（替代原生 alert/confirm） -->
    <FeedbackLayer />
    <CommandPalette v-model:open="showPalette" />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import BookOutline from '@vicons/ionicons5/es/BookOutline';
import CheckmarkOutline from '@vicons/ionicons5/es/CheckmarkOutline';
import DiamondOutline from '@vicons/ionicons5/es/DiamondOutline';
import DocumentTextOutline from '@vicons/ionicons5/es/DocumentTextOutline';
import FlaskOutline from '@vicons/ionicons5/es/FlaskOutline';
import GridOutline from '@vicons/ionicons5/es/GridOutline';
import HelpCircleOutline from '@vicons/ionicons5/es/HelpCircleOutline';
import KeyOutline from '@vicons/ionicons5/es/KeyOutline';
import LanguageOutline from '@vicons/ionicons5/es/LanguageOutline';
import LockClosedOutline from '@vicons/ionicons5/es/LockClosedOutline';
import MoonOutline from '@vicons/ionicons5/es/MoonOutline';
import PeopleOutline from '@vicons/ionicons5/es/PeopleOutline';
import PersonOutline from '@vicons/ionicons5/es/PersonOutline';
import SearchOutline from '@vicons/ionicons5/es/SearchOutline';
import SunnyOutline from '@vicons/ionicons5/es/SunnyOutline';
import TimeOutline from '@vicons/ionicons5/es/TimeOutline';
import WarningOutline from '@vicons/ionicons5/es/WarningOutline';
import type { Component } from 'vue';
import { useRoute } from 'vue-router';
import { invoke, isTauriEnv } from './api/invoke';

import { useDashboardStore } from './stores/dashboard';
import { useSecurityStore } from './stores/security';
import { useAssetsStore } from './stores/assets';
import { startLocalReminderLoop } from './api/reminders';
import { initTheme, toggleTheme, useTheme } from './composables/theme';
import { useI18n } from './i18n';
import OnboardingTour from './components/OnboardingTour.vue';
import HelpDrawer from './components/HelpDrawer.vue';
import CommandPalette from './components/CommandPalette.vue';
import FeedbackLayer from './components/FeedbackLayer.vue';
import type { ApiResponse, SecurityStatus } from './types';

interface NavRoute {
  path: string;
  titleKey: string;
  icon: Component;
}

interface NavGroup {
  id: string;
  labelKey: string;
  /** 一句话说明这一段在做什么；侧栏空间有限，只在 tooltip 里给出 */
  routes: NavRoute[];
}

const route = useRoute();
const dashboard = useDashboardStore();
const security = useSecurityStore();
const { t, locale, toggleLocale } = useI18n();

// 版本号：构建期由 Vite 从 package.json 注入（见 vite.config.ts define）
const appVersion = __APP_VERSION__;
// 浏览器演示构建（VITE_DEMO=1）时显示醒目的「演示数据」横幅
const demoMode = __DEMO_MODE__;

const unlockPassword = ref('');
const unlockError = ref('');
const showTour = ref(false);
const showHelp = ref(false);
const showPalette = ref(false);

/** 快捷键提示用哪个修饰键：macOS 显示 ⌘，其余显示 Ctrl。 */
const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || '');
const { resolved: themeResolved } = useTheme();
const themeIcon = computed(() => (themeResolved.value === 'dark' ? SunnyOutline : MoonOutline));
const themeLabel = computed(() => (themeResolved.value === 'dark' ? t('app.themeToLight') : t('app.themeToDark')));
const lockCardRef = ref<HTMLElement | null>(null);
let stopReminders: (() => void) | null = null;
let stopTrayEvents: (() => void) | null = null;

// 焦点困留（document 捕获阶段拦截 Tab）：即使焦点因点击遮罩逃到对话框之外，
// 下一次 Tab 也会被拉回对话框，禁止穿透到被遮罩的侧边栏与主内容
function trapLockFocus(e: KeyboardEvent) {
  if (e.key !== 'Tab') return;
  const card = lockCardRef.value;
  if (!card) return;
  const focusables = Array.from(
    card.querySelectorAll<HTMLElement>('button:not([disabled]), input, [href], textarea, select')
  );
  if (focusables.length === 0) return;
  const active = document.activeElement as HTMLElement | null;
  const inside = !!active && card.contains(active);
  if (!inside) {
    e.preventDefault();
    focusables[0].focus();
    return;
  }
  const idx = focusables.indexOf(active);
  let next = idx + (e.shiftKey ? -1 : 1);
  if (next < 0) next = focusables.length - 1;
  if (next >= focusables.length) next = 0;
  e.preventDefault();
  focusables[next].focus();
}

function startTour() {
  showHelp.value = false;
  showTour.value = true;
}

const IDLE_LIMIT_MS = 10 * 60 * 1000;
let lastActivity = Date.now();
let idleTimer: number | undefined;

function bumpActivity() {
  lastActivity = Date.now();
}

function registerActivityWatch() {
  ['mousemove', 'keydown', 'click', 'wheel', 'touchstart'].forEach(ev =>
    window.addEventListener(ev, bumpActivity, { passive: true })
  );
  idleTimer = window.setInterval(() => {
    if (security.hasMasterPassword && security.unlocked && Date.now() - lastActivity > IDLE_LIMIT_MS) {
      handleLock();
    }
  }, 30_000);
}

function unregisterActivityWatch() {
  ['mousemove', 'keydown', 'click', 'wheel', 'touchstart'].forEach(ev =>
    window.removeEventListener(ev, bumpActivity)
  );
  if (idleTimer !== undefined) {
    window.clearInterval(idleTimer);
  }
}

async function handleUnlock() {
  unlockError.value = '';
  if (!unlockPassword.value) {
    unlockError.value = t('lock.enterPassword');
    return;
  }
  const err = await security.unlockWith(unlockPassword.value);
  if (err) {
    unlockError.value = err;
  } else {
    unlockPassword.value = '';
    bumpActivity();
    await refreshSensitiveData();
  }
}

async function refreshSensitiveData() {
  const assetsStore = useAssetsStore();
  await Promise.all([assetsStore.loadAssets(), dashboard.loadUser()]);
}

function wipeSensitiveData() {
  const assetsStore = useAssetsStore();
  assetsStore.assets = [];
  assetsStore.currentAsset = null;
  dashboard.user = null;
}

async function handleLock() {
  await security.lockNow();
  wipeSensitiveData();
  bumpActivity();
}

const recoveryMode = ref(false);
const recoveryCode = ref('');
const recoveryError = ref('');
const recoveryUnlocked = ref(false);
const recoveryNewPassword = ref('');
const recoveryNewPassword2 = ref('');
const recoveryMessage = ref('');
const recoveryOk = ref(false);
const recoveryBusy = ref(false);
const recoveryNewCode = ref('');
const recoveryNewCodeAck = ref(false);

// ── 继承人恢复份额解锁 ──
const shareMode = ref(false);
const shareInputs = ref<string[]>(['']);
const sharePassphrase = ref('');
const shareError = ref('');
const shareMessage = ref('');
const shareBusy = ref(false);

async function handleShareUnlock() {
  shareError.value = '';
  shareMessage.value = '';
  const shares = shareInputs.value.map(s => s.trim()).filter(Boolean);
  if (shares.length === 0) {
    shareError.value = t('kit.shareRequired');
    return;
  }
  shareBusy.value = true;
  try {
    const res = await invoke<ApiResponse<{ has_master_password: boolean; unlocked: boolean }>>(
      'unlock_with_recovery_shares',
      { shares, passphrase: sharePassphrase.value.trim() ? sharePassphrase.value : null },
    );
    if (res.success) {
      shareMessage.value = t('kit.unlocked');
      shareInputs.value = [''];
      sharePassphrase.value = '';
      shareMode.value = false;
      await security.refresh();
      // 份额解出的是旧密钥；数据已可读，但必须让用户立刻设新密码
      recoveryMode.value = true;
      recoveryUnlocked.value = true;
      recoveryMessage.value = t('kit.unlocked');
      recoveryOk.value = true;
    } else {
      shareError.value = res.message;
    }
  } catch (e) {
    shareError.value = String(e);
  } finally {
    shareBusy.value = false;
  }
}

/** 用户确认已抄写新恢复密钥后才关闭恢复流程。 */
function finishRecovery() {
  if (!recoveryNewCodeAck.value) return;
  security.refresh();
  recoveryMode.value = false;
  recoveryUnlocked.value = false;
  recoveryNewCode.value = '';
  recoveryNewCodeAck.value = false;
  recoveryCode.value = '';
  recoveryMessage.value = '';
  recoveryOk.value = false;
  refreshSensitiveData();
}

// 锁定遮罩出现 / 内部状态切换（主密码↔恢复模式↔设置新密码）时：
// 1. 注册全文档捕获级 Tab 困留（焦点逃出对话框也能被拉回）
// 2. 焦点移入对应输入框（恢复设置新密码时聚焦新密码框，而非第一个输入框）
watch(
  () => [security.hasMasterPassword, security.unlocked, recoveryMode.value, recoveryUnlocked.value] as const,
  async ([hasMasterPassword, unlocked, recMode, recUnlocked]) => {
    const locked = hasMasterPassword && !unlocked;
    if (locked) {
      document.addEventListener('keydown', trapLockFocus, true);
      await nextTick();
      if (recMode && recUnlocked) {
        lockCardRef.value?.querySelector<HTMLElement>('[data-autofocus-pw]')?.focus();
      } else {
        lockCardRef.value?.querySelector<HTMLElement>('input')?.focus();
      }
    } else {
      document.removeEventListener('keydown', trapLockFocus, true);
    }
  }
);

async function handleStartRecovery() {
  recoveryError.value = '';
  if (!recoveryCode.value.trim()) {
    recoveryError.value = t('recovery.enterCode');
    return;
  }
  try {
    const res = await invoke<ApiResponse<SecurityStatus>>('start_recovery', { recoveryCode: recoveryCode.value });
    if (res.success) {
      recoveryUnlocked.value = true;
      recoveryMessage.value = '';
      security.unlocked = false;
    } else {
      recoveryError.value = res.message;
    }
  } catch (e) {
    recoveryError.value = String(e);
  }
}

async function handleCompleteRecovery() {
  recoveryMessage.value = '';
  recoveryBusy.value = true;
  try {
    const res = await invoke<ApiResponse<string>>('complete_recovery', {
      newPassword: recoveryNewPassword.value,
      confirmPassword: recoveryNewPassword2.value,
    });
    recoveryOk.value = res.success;
    recoveryMessage.value = res.message;
    if (res.success && res.data) {
      recoveryNewCode.value = res.data;
      recoveryNewCodeAck.value = false;
      recoveryUnlocked.value = false;
      recoveryCode.value = '';
      recoveryNewPassword.value = '';
      recoveryNewPassword2.value = '';
    }
  } catch (e) {
    recoveryOk.value = false;
    recoveryMessage.value = String(e);
  } finally {
    recoveryBusy.value = false;
  }
}

function backToUnlock() {
  recoveryMode.value = false;
  recoveryUnlocked.value = false;
  recoveryError.value = '';
  recoveryMessage.value = '';
}

/**
 * 导航按「你现在要完成什么」分组，而不是按功能模块平铺。
 * 定位是「给家人留下能照着做的工具包」：持有者先盘点、再规划、再交付、最后维护。
 * 旧路由全部保留（深链与测试不受影响），只是不再平铺成 9 项。
 */
const navGroups: NavGroup[] = [
  {
    id: 'start',
    labelKey: 'nav.groupStart',
    routes: [{ path: '/dashboard', titleKey: 'nav.dashboard', icon: GridOutline }],
  },
  {
    id: 'inventory',
    labelKey: 'nav.groupInventory',
    routes: [
      { path: '/discover', titleKey: 'nav.discover', icon: SearchOutline },
      { path: '/assets', titleKey: 'nav.assets', icon: DiamondOutline },
    ],
  },
  {
    id: 'plan',
    labelKey: 'nav.groupPlan',
    routes: [
      { path: '/profile', titleKey: 'nav.profile', icon: PersonOutline },
      { path: '/dms', titleKey: 'nav.dms', icon: TimeOutline },
      { path: '/legal', titleKey: 'nav.legal', icon: BookOutline },
    ],
  },
  {
    id: 'deliver',
    labelKey: 'nav.groupDeliver',
    // 家人真正会打开的两页：报告（拿到清单）与继承人向导（照着办）。
    routes: [
      { path: '/report', titleKey: 'nav.report', icon: DocumentTextOutline },
      { path: '/heir', titleKey: 'nav.heir', icon: PeopleOutline },
    ],
  },
  {
    id: 'check',
    labelKey: 'nav.groupCheck',
    routes: [{ path: '/review', titleKey: 'nav.review', icon: CheckmarkOutline }],
  },
];

const navRoutes: NavRoute[] = navGroups.flatMap(g => g.routes);

const currentRoute = computed(() => route.path);
const currentPageTitle = computed(() => {
  // 优先用 route.meta：路由解析完成后它一定存在；用 path 反查 navRoutes 在
  // 首帧/高并发下会拿到尚未重定向的 `/`，标题闪烁成 appName（E2E 已抓到）。
  const metaKey = route.meta?.titleKey as string | undefined;
  if (metaKey) return t(metaKey);
  const navKey = navRoutes.find(r => r.path === route.path)?.titleKey;
  return navKey ? t(navKey) : t('common.appName');
});

const checkBtnText = computed(() => {
  const status = dashboard.checkInStatus;
  if (!status) return t('app.checking');
  if (!status.dms_enabled) return t('app.dmsDisabled');
  const days = status.days_until_due;
  if (days === null) return t('app.checkNow');
  if (days <= 0) return t('app.checkSafety');
  if (days <= 3) return t('app.checkInDays', { days });
  return t('app.checkInDaysPlain', { days });
});

// 图标与文案分离：emoji 走 aria-hidden 装饰位，读屏只读语义文本
const checkBtnIcon = computed(() => {
  const status = dashboard.checkInStatus;
  if (!status?.dms_enabled) return '';
  const days = status.days_until_due;
  if (days === null) return '';
  if (days <= 0) return WarningOutline;
  if (days <= 3) return '';
  return CheckmarkOutline;
});

const checkBtnClass = computed(() => {
  const status = dashboard.checkInStatus;
  if (!status?.dms_enabled) return 'btn-secondary';
  if ((status.days_until_due ?? 999) <= 0) return 'btn-danger';
  if ((status.days_until_due ?? 999) <= 3) return 'btn-warning';
  return 'btn-success';
});

async function handleCheckIn() {
  await dashboard.checkIn();
}

// 托盘菜单与后台线程会向前端 emit 事件：
//   app-locked      托盘「锁定应用」——必须立刻清空界面上的已解密数据
//   dms-checked-in  托盘「我还安好」——刷新签到状态与统计
// 此前没有任何监听，导致托盘锁定后窗口仍显示已解密的账号与档案（安全缺陷）。
async function bindTrayEvents(): Promise<() => void> {
  if (!isTauriEnv()) return () => {};
  try {
    const { listen } = await import('@tauri-apps/api/event');
    const unlistenLocked = await listen('app-locked', () => {
      security.unlocked = false;
      wipeSensitiveData();
      bumpActivity();
    });
    const unlistenCheckedIn = await listen('dms-checked-in', () => {
      dashboard.loadCheckInStatus();
      dashboard.loadStats();
    });
    return () => {
      unlistenLocked();
      unlistenCheckedIn();
    };
  } catch (e) {
    console.error('绑定托盘事件失败:', e);
    return () => {};
  }
}


initTheme();

onMounted(async () => {
  security.refresh();
  registerActivityWatch();
  dashboard.loadCheckInStatus();
  dashboard.evaluateDms();
  stopReminders = startLocalReminderLoop();
  stopTrayEvents = await bindTrayEvents();
  window.setTimeout(() => {
    const locked = security.hasMasterPassword && !security.unlocked;
    if (!locked && !localStorage.getItem('digilegacy_onboarding_done')) {
      showTour.value = true;
    }
  }, 900);
});

onBeforeUnmount(() => {
  unregisterActivityWatch();
  if (stopReminders) stopReminders();
  if (stopTrayEvents) stopTrayEvents();
  document.removeEventListener('keydown', trapLockFocus, true);
});
</script>

<style scoped>
.lock-overlay {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.lock-card {
  padding: 34px 32px;
  width: 380px;
  max-width: calc(100vw - 48px);
  text-align: center;
  max-height: calc(100vh - 48px);
  overflow-y: auto;
}
.lock-hero-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 72px;
  height: 72px;
  margin: 0 auto;
  border-radius: 20px;
  background: linear-gradient(135deg, rgba(196, 106, 92, 0.16), rgba(196, 106, 92, 0.06));
  color: var(--primary);
}
.lock-hero-icon :deep(svg) { width: 30px; height: 30px; }
.recovery-msg {
  word-break: break-all;
}
.recovery-ack {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
  font-size: 13px;
  color: var(--gray-700);
  cursor: pointer;
  text-align: left;
}

.demo-banner {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  margin-bottom: 20px;
  padding: 12px 16px;
  color: var(--warning-strong);
  font-size: 13px;
  line-height: 1.6;
}
.demo-banner :deep(svg) { width: 18px; height: 18px; flex-shrink: 0; }
</style>
