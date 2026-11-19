import { createRouter, createWebHashHistory, createWebHistory } from 'vue-router';
import { t } from './i18n';

const router = createRouter({
  // 桌面版用 history 模式（Tauri 内部协议会回退到 index.html）；
  // 在线演示站点是纯静态托管，缺少服务端 rewrite，必须用 hash 模式，
  // 否则用户刷新 /dashboard 就会拿到 404。
  history: __DEMO_MODE__
    ? createWebHashHistory(import.meta.env.BASE_URL)
    : createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      redirect: '/dashboard'
    },
    {
      path: '/dashboard',
      name: 'Dashboard',
      component: () => import('./views/DashboardView.vue'),
      meta: { titleKey: 'nav.dashboard' }
    },
    {
      path: '/discover',
      name: 'Discover',
      component: () => import('./views/DiscoverView.vue'),
      meta: { titleKey: 'nav.discover' }
    },
    {
      path: '/assets',
      name: 'Assets',
      component: () => import('./views/AssetsView.vue'),
      meta: { titleKey: 'nav.assets' }
    },
    {
      path: '/assets/:id',
      name: 'AssetDetail',
      component: () => import('./views/AssetDetailView.vue'),
      meta: { titleKey: 'nav.assetDetail' }
    },
    {
      path: '/profile',
      name: 'Profile',
      component: () => import('./views/ProfileView.vue'),
      meta: { titleKey: 'nav.profile' }
    },
    {
      path: '/report',
      name: 'Report',
      component: () => import('./views/ReportView.vue'),
      meta: { titleKey: 'nav.report' }
    },
    {
      path: '/dms',
      name: 'DMS',
      component: () => import('./views/DMSView.vue'),
      meta: { titleKey: 'nav.dms' }
    },
    {
      path: '/heir',
      name: 'HeirGuide',
      component: () => import('./views/HeirGuideView.vue'),
      meta: { titleKey: 'nav.heir' }
    },
    {
      path: '/review',
      name: 'Review',
      component: () => import('./views/ReviewView.vue'),
      meta: { titleKey: 'nav.review' }
    },
    {
      path: '/legal',
      name: 'LegalTemplates',
      component: () => import('./views/LegalTemplates.vue'),
      meta: { titleKey: 'nav.legal' }
    },
    {
      // 兜底路由：手输 URL / 旧链接 / 演示站点刷新深层路径时，
      // 不能落到空白页；给一个明确的「页面不存在」出口。
      path: '/:pathMatch(.*)*',
      name: 'NotFound',
      component: () => import('./views/NotFoundView.vue'),
      meta: { titleKey: 'nav.notFound' }
    },
  ],
  scrollBehavior(_to, _from, saved) {
    return saved ?? { top: 0 };
  },
});

/** 由路由 meta 解析页面标题；无 titleKey 时回退应用名。导出以便单测覆盖两个分支。 */
interface RouteMetaLike { titleKey?: unknown }

export function resolveTitleKey(meta?: RouteMetaLike): string {
  const key = typeof meta?.titleKey === 'string' ? meta.titleKey : '';
  return key ? t(key) : '';
}

router.afterEach((to) => {
  const title = resolveTitleKey(to.meta as RouteMetaLike);
  document.title = title ? `${title} · ${t('common.appName')}` : t('common.appName');
});

export default router;
