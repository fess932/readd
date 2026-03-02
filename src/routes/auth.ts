import Elysia, { t } from 'elysia';
import { eq } from 'drizzle-orm';
import { db } from '../db';
import { users } from '../db/schema';
import { jwtPlugin, authMiddleware } from '../middleware/auth';
import { log } from '../logger';

export const authRoutes = new Elysia({ prefix: '/api/auth' })
  .use(jwtPlugin)
  .use(authMiddleware)
  .post('/login', async ({ body, jwt, set }) => {
    const { name } = body;
    if (!name || name.trim().length === 0) {
      set.status = 400;
      return { error: 'Name is required' };
    }
    const trimmed = name.trim();

    const isAdmin = trimmed.toLowerCase() === 'admin';

    let user = db.select().from(users).where(eq(users.name, trimmed)).get();
    if (!user) {
      const [created] = db.insert(users).values({ name: trimmed, isAdmin }).returning().all();
      user = created;
      log.info('user registered', { name: user.name, isAdmin: user.isAdmin });
    } else if (isAdmin && !user.isAdmin) {
      db.update(users).set({ isAdmin: true }).where(eq(users.id, user.id)).run();
      user = { ...user, isAdmin: true };
      log.info('user promoted to admin', { name: user.name });
    } else {
      log.info('user login', { name: user.name, isAdmin: user.isAdmin });
    }

    const token = await jwt.sign({ id: user.id, name: user.name, isAdmin: user.isAdmin });
    return { token, user: { id: user.id, name: user.name, isAdmin: user.isAdmin } };
  }, {
    body: t.Object({ name: t.String() }),
  })
  .get('/me', ({ user, set }) => {
    if (!user) {
      set.status = 401;
      return { error: 'Unauthorized' };
    }
    return { user };
  });
