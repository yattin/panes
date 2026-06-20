import { create } from "zustand";
import {
  appendToast,
  DEFAULT_TOAST_DURATIONS,
  type ToastState,
  type ToastVariant,
} from "../domain/toast";

let nextId = 0;

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],

  addToast: ({ variant, message, duration }) => {
    const id = String(++nextId);
    const ms = duration ?? DEFAULT_TOAST_DURATIONS[variant];

    set((state) => ({
      toasts: appendToast(state.toasts, { id, variant, message, duration: ms }),
    }));

    return id;
  },

  dismissToast: (id) => {
    set((state) => ({
      toasts: state.toasts.filter((toast) => toast.id !== id),
    }));
  },
}));

function addToast(variant: ToastVariant, message: string, duration?: number): string {
  return useToastStore.getState().addToast({ variant, message, duration });
}

export const toast = {
  success: (message: string, duration?: number) => addToast("success", message, duration),
  error: (message: string, duration?: number) => addToast("error", message, duration),
  warning: (message: string, duration?: number) => addToast("warning", message, duration),
  info: (message: string, duration?: number) => addToast("info", message, duration),
};
