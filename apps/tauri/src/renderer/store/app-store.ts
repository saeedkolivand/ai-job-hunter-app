import { create } from 'zustand';

interface AppState {
  paletteOpen: boolean;
  setPaletteOpen: (v: boolean) => void;
  togglePalette: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  paletteOpen: false,
  setPaletteOpen: (v) => set({ paletteOpen: v }),
  togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),
}));
