export type ToastVariant = "success" | "error" | "warning" | "info";

export interface Toast {
  id: string;
  variant: ToastVariant;
  message: string;
  duration: number;
}

export interface ToastState {
  toasts: Toast[];
  addToast: (opts: {
    variant: ToastVariant;
    message: string;
    duration?: number;
  }) => string;
  dismissToast: (id: string) => void;
}

export const MAX_TOASTS = 5;

export const DEFAULT_TOAST_DURATIONS: Record<ToastVariant, number> = {
  success: 4000,
  info: 4000,
  warning: 6000,
  error: 8000,
};

export function appendToast(toasts: Toast[], toast: Toast): Toast[] {
  const next = [...toasts, toast];
  return next.length > MAX_TOASTS ? next.slice(1) : next;
}
