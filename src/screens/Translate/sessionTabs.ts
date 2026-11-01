import { create } from "zustand";

import type { SessionTarget } from "../../bindings/SessionTarget";
import type { TargetClosed } from "../../bindings/TargetClosed";
import { onTargetClosed, toAppError, unbindTarget } from "../../lib/api";
import { useAppStore } from "../../store/app";
import { createTabStore, type TabStore } from "./store";

/** At most this many tabs are bound to sessions (the backend allows the same number). */
export const MAX_BOUND_TABS = 5;

/** Id of the default tab, the one that isn't bound to a session. */
export const DEFAULT_TAB = "default";

export interface BoundTab {
  id: string;
  target: SessionTarget;
  /** The session's title when it was bound (a label only). */
  title: string;
  store: TabStore;
}

interface SessionTabsState {
  tabs: BoundTab[];
  /** `DEFAULT_TAB` or a bound tab's id. */
  activeId: string;
  activate: (id: string) => void;
  /**
   * Adds a tab for a target that was just bound (one binding exists for it). When a tab for the
   * same target exists already, that one is focused and the extra binding is released.
   */
  addBound: (target: SessionTarget) => void;
  /** The user closes a bound tab: releases its binding. */
  close: (id: string) => void;
  /** The target is gone (its binding with it): closes the matching tabs. */
  closeGone: (closed: TargetClosed) => void;
}

function sameTarget(a: SessionTarget, b: SessionTarget): boolean {
  return a.hwnd === b.hwnd && a.tabRuntimeId === b.tabRuntimeId;
}

export const useSessionTabs = create<SessionTabsState>((set, get) => ({
  tabs: [],
  activeId: DEFAULT_TAB,

  activate: (id) => {
    if (id === DEFAULT_TAB || get().tabs.some((t) => t.id === id)) set({ activeId: id });
  },

  addBound: (target) => {
    const existing = get().tabs.find((t) => sameTarget(t.target, target));
    if (existing) {
      // Each bind needs one unbind; the tab keeps its own binding.
      void unbindTarget(target).catch(() => undefined);
      set({ activeId: existing.id });
      return;
    }
    const id = crypto.randomUUID();
    // A bound tab starts empty with the settings' defaults, in mode Translate (Improve there is the
    // slower Claude-prompt rewrite, #199, so the user picks it).
    const tab: BoundTab = {
      id,
      target,
      title: target.title,
      store: createTabStore({ mode: "translate" }),
    };
    set((s) => ({ tabs: [...s.tabs, tab], activeId: id }));
  },

  close: (id) => {
    const tab = get().tabs.find((t) => t.id === id);
    if (!tab) return;
    set((s) => ({
      tabs: s.tabs.filter((t) => t.id !== id),
      activeId: s.activeId === id ? DEFAULT_TAB : s.activeId,
    }));
    void unbindTarget(tab.target).catch((e) => {
      useAppStore.getState().showToast("error", toAppError(e).message);
    });
  },

  closeGone: (closed) => {
    // A closed window takes all its tabs along (no tab id); otherwise only the named tab.
    const gone = get().tabs.filter(
      (t) =>
        t.target.hwnd === closed.hwnd &&
        (closed.tabRuntimeId === undefined || t.target.tabRuntimeId === closed.tabRuntimeId),
    );
    if (gone.length === 0) return;
    const ids = new Set(gone.map((t) => t.id));
    set((s) => ({
      tabs: s.tabs.filter((t) => !ids.has(t.id)),
      activeId: ids.has(s.activeId) ? DEFAULT_TAB : s.activeId,
    }));
    for (const t of gone) useAppStore.getState().showToast("info", `${t.title} closed`);
  },
}));

/** Listens to `target://closed` for the whole app run; returns the unlisten promise. */
export function initSessionTabs() {
  return onTargetClosed((closed) => useSessionTabs.getState().closeGone(closed));
}
