import { create } from "zustand";
import { getVersion } from "@tauri-apps/api/app";

/**
 * App version state.
 *
 * CKL builds do not check for updates over the network: the upstream release
 * check (api.github.com) was removed so the app makes no background requests.
 * Builds are distributed internally by CKL. `currentVersion` is kept in sync
 * with the other version files (see the Version Bump Checklist in AGENTS.md)
 * and is overwritten at runtime by the version Tauri reports.
 */
interface UpdateState {
  currentVersion: string;
  init: () => Promise<void>;
}

export const useUpdateStore = create<UpdateState>((set) => ({
  currentVersion: "0.9.0",

  init: async () => {
    try {
      const ver = await getVersion();
      if (ver) {
        set({ currentVersion: ver });
      }
    } catch {
      // Running outside Tauri (e.g. vite preview); keep the bundled default.
    }
  },
}));
