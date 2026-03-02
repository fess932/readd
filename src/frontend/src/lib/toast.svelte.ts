export interface Toast {
  id: number;
  type: 'success' | 'error' | 'info';
  text: string;
}

let _id = 0;
export const toasts = $state<Toast[]>([]);

export function toast(type: Toast['type'], text: string, ms = 4000) {
  const id = ++_id;
  toasts.push({ id, type, text });
  setTimeout(() => {
    const i = toasts.findIndex(t => t.id === id);
    if (i !== -1) toasts.splice(i, 1);
  }, ms);
}
