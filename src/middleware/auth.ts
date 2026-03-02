import Elysia from 'elysia';
import { jwt } from '@elysiajs/jwt';

export const jwtPlugin = jwt({
  name: 'jwt',
  secret: process.env.JWT_SECRET ?? 'readd-secret-key',
});

export interface JwtPayload {
  id: number;
  name: string;
  isAdmin: boolean;
}

export const authMiddleware = new Elysia({ name: 'auth-middleware' })
  .use(jwtPlugin)
  .derive({ as: 'global' }, async ({ jwt, request }) => {
    const auth = request.headers.get('authorization');
    if (!auth?.startsWith('Bearer ')) return { user: null };
    const token = auth.slice(7);
    const payload = await jwt.verify(token) as JwtPayload | false;
    if (!payload) return { user: null };
    return { user: payload };
  });
