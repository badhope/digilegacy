import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import router from './router';
// 样式分两层：main.css 是基础与既有组件（类名被测试与 E2E 依赖，只增不改），
// design-upgrade.css 是设计增强层（令牌阶梯、动效、新组件）。
// 顺序很重要：增强层必须在后，才能覆盖基础层的同名规则。
import './assets/main.css';
import './assets/design-upgrade.css';

const app = createApp(App);
const pinia = createPinia();

app.use(pinia);
app.use(router);

// 兜底：渲染期异常不应把整页变成白屏（日志保留，便于用户反馈时定位）
app.config.errorHandler = (err, _instance, info) => {
  console.error('[render-error]', info, err);
};

app.mount('#app');
