import {create} from 'zustand';

export const useStore = create(() => ({
  isPlaying: false,
}));