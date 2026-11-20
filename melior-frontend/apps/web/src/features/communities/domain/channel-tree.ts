import type { ChannelId } from '@/features/conference/domain';
import type { Category, Channel, CommunityView } from './community';

/** One section of the channel list: a category with its channels, or the channels without one. */
export type ChannelGroup = { category: Category | null; channels: Channel[] };

const byPosition = <T extends { position: number; name: string }>(a: T, b: T): number =>
    a.position - b.position || a.name.localeCompare(b.name);

/**
 * The channel list in display order: channels without a category first, then each category by position with its
 * channels by position. An empty category is shown only to someone who can put channels in it.
 */
export const buildChannelTree = (
    view: Pick<CommunityView, 'categories' | 'channels'>,
    showEmptyCategories: boolean,
): ChannelGroup[] => {
    const known = new Set(view.categories.map((category) => category.id));
    const loose = view.channels.filter((channel) => channel.categoryId === null || !known.has(channel.categoryId));
    const groups: ChannelGroup[] = [...view.categories].sort(byPosition).map((category) => ({
        category,
        channels: view.channels.filter((channel) => channel.categoryId === category.id).sort(byPosition),
    }));
    const shown = groups.filter((group) => showEmptyCategories || group.channels.length > 0);
    return loose.length > 0 ? [{ category: null, channels: loose.sort(byPosition) }, ...shown] : shown;
};

/**
 * Where `/c/<id>` leads: the first text channel of the list, else its first channel, so opening a server does not
 * start a call by itself. `except` skips the channel being left.
 */
export const firstChannel = (
    view: Pick<CommunityView, 'categories' | 'channels'>,
    except: string | null = null,
): Channel | null => {
    const ordered = buildChannelTree(view, false)
        .flatMap((group) => group.channels)
        .filter((channel) => channel.id !== except);
    return ordered.find((channel) => channel.kind === 'text') ?? ordered[0] ?? null;
};

export const findChannel = (view: Pick<CommunityView, 'channels'>, channelId: ChannelId | string): Channel | null =>
    view.channels.find((channel) => channel.id === channelId) ?? null;
