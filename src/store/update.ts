import { create } from "zustand";

import type { UpdateInfo } from "../bindings/UpdateInfo";
import type { UpdateProgress } from "../bindings/UpdateProgress";
import { installUpdate, onUpdateAvailable, onUpdateProgress, toAppError } from "../lib/api";

interface UpdateState {
  /** The newest release found (startup check or the About tab); null when none is known. */
  available: UpdateInfo | null;
  /** The global banner was dismissed; the About tab still shows the update. */
  bannerDismissed: boolean;
  installing: boolean;
  progress: UpdateProgress | null;
  installError: string | null;

  setAvailable: (info: UpdateInfo | null) => void;
  dismissBanner: () => void;
  install: () => Promise<void>;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  available: null,
  bannerDismissed: false,
  installing: false,
  progress: null,
  installError: null,

  setAvailable: (info) =>
    set((s) => ({
      available: info?.available ? info : null,
      // A different version shows the banner again.
      bannerDismissed:
        info?.available && info.version === s.available?.version ? s.bannerDismissed : false,
    })),
  dismissBanner: () => set({ bannerDismissed: true }),

  // Downloads and runs the installer; the app exits and relaunches when it succeeds.
  install: async () => {
    if (get().installing) return;
    set({ installing: true, progress: null, installError: null });
    try {
      await installUpdate();
    } catch (e) {
      set({ installing: false, progress: null, installError: toAppError(e).message });
    }
  },
}));

/** Subscribes to the updater events. Returns the unsubscribe function. */
export async function initUpdateStore(): Promise<() => void> {
  const offAvailable = await onUpdateAvailable((info) =>
    useUpdateStore.getState().setAvailable(info),
  );
  const offProgress = await onUpdateProgress((progress) => useUpdateStore.setState({ progress }));
  return () => {
    offAvailable();
    offProgress();
  };
}
