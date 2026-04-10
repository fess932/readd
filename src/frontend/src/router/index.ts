import { createRouter, createWebHistory } from 'vue-router';
import { auth } from '../stores/auth';
import LoginView from '../views/LoginView.vue';
import LibraryView from '../views/LibraryView.vue';
import ExploreView from '../views/ExploreView.vue';
import BookView from '../views/BookView.vue';
import StatsView from '../views/StatsView.vue';
import TextBooksView from '../views/TextBooksView.vue';

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: () => (auth.user ? '/library' : '/login') },
    { path: '/login', name: 'login', component: LoginView },
    { path: '/library', name: 'library', component: LibraryView, meta: { requiresAuth: true } },
    { path: '/explore', name: 'explore', component: ExploreView, meta: { requiresAuth: true } },
    { path: '/stats', name: 'stats', component: StatsView, meta: { requiresAuth: true } },
    { path: '/book/:id', name: 'book', component: BookView, meta: { requiresAuth: true } },
    { path: '/text-books', name: 'text-books', component: TextBooksView, meta: { requiresAuth: true } },
  ],
});

router.beforeEach((to) => {
  if (to.meta.requiresAuth && !auth.user) return '/login';
  if (to.name === 'login' && auth.user) return '/library';
});

export default router;
