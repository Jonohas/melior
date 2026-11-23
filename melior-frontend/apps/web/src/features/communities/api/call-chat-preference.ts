import { screenAtLeast } from './screen';

/** Tailwind's `lg`: from here the call's chat sits beside the grid; below it, it takes the grid's room, so it starts closed. */
const CHAT_BESIDE_GRID_MIN_WIDTH = '64rem';

/** Whether a call's chat starts open: only where it has room beside the video grid. */
export const initialCallChatOpen = (): boolean => screenAtLeast(CHAT_BESIDE_GRID_MIN_WIDTH);
