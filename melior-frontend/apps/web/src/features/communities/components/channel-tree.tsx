import { Link } from '@tanstack/react-router';
import { ChevronDown, ChevronRight, Hash, Plus, Settings, Volume2 } from 'lucide-react';
import { useState } from 'react';
import { Button } from '@/components/ui';
import {
    type Category,
    type Channel,
    type ChannelGroup,
    mediaSummary,
    occupantsOf,
    type VoiceRoster,
} from '@/features/communities/domain';
import { cn } from '@/lib/utils';
import { VoiceOccupants } from './voice-occupants';

type ChannelTreeProps = {
    groups: ChannelGroup[];
    roster: VoiceRoster;
    activeChannelId: string | null;
    /** Show the create and settings buttons. */
    canManage: boolean;
    onCreateChannel: (categoryId: string | null) => void;
    onEditChannel: (channel: Channel) => void;
    onEditCategory: (category: Category) => void;
};

export const ChannelIcon = ({ channel, className }: { channel: Pick<Channel, 'kind'>; className?: string }) =>
    channel.kind === 'voice' ? (
        <Volume2 className={cn('size-4 shrink-0', className)} aria-label="Voice channel" />
    ) : (
        <Hash className={cn('size-4 shrink-0', className)} aria-label="Text channel" />
    );

type ChannelRowProps = {
    channel: Channel;
    active: boolean;
    occupants: number;
    canManage: boolean;
    onEdit: () => void;
};

const ChannelRow = ({ channel, active, occupants, canManage, onEdit }: ChannelRowProps) => {
    const summary = mediaSummary(channel, occupants);
    return (
        <div
            className={cn(
                'group flex items-center rounded-md text-muted-foreground hover:bg-accent hover:text-accent-foreground',
                active && 'bg-accent text-accent-foreground',
            )}
        >
            <Link
                to="/c/$communityId/$channelId"
                params={{ communityId: channel.communityId, channelId: channel.id }}
                className="flex min-w-0 flex-1 items-center gap-1.5 px-2 py-1.5 text-sm"
                aria-current={active ? 'page' : undefined}
            >
                <ChannelIcon channel={channel} />
                <span className={cn('truncate', active && 'font-medium')}>{channel.name}</span>
                {summary && <span className="ml-auto shrink-0 text-[11px] text-muted-foreground">{summary}</span>}
            </Link>
            {canManage && (
                <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="size-7 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                    aria-label={`Edit ${channel.name}`}
                    onClick={onEdit}
                >
                    <Settings />
                </Button>
            )}
        </div>
    );
};

type CategoryHeaderProps = {
    category: Category;
    collapsed: boolean;
    canManage: boolean;
    onToggle: () => void;
    onCreate: () => void;
    onEdit: () => void;
};

const CategoryHeader = ({ category, collapsed, canManage, onToggle, onCreate, onEdit }: CategoryHeaderProps) => (
    <div className="group flex items-center pt-3 text-muted-foreground">
        <button
            type="button"
            className="flex min-w-0 flex-1 cursor-pointer items-center gap-0.5 text-xs font-semibold tracking-wide uppercase hover:text-foreground"
            aria-expanded={!collapsed}
            onClick={onToggle}
        >
            {collapsed ? <ChevronRight className="size-3" /> : <ChevronDown className="size-3" />}
            <span className="truncate">{category.name}</span>
        </button>
        {canManage && (
            <>
                <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="size-6 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                    aria-label={`Edit category ${category.name}`}
                    onClick={onEdit}
                >
                    <Settings />
                </Button>
                <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="size-6"
                    aria-label={`Create a channel in ${category.name}`}
                    onClick={onCreate}
                >
                    <Plus />
                </Button>
            </>
        )}
    </div>
);

/** The channel list: categories that fold, channels, and who is in each voice channel. */
export const ChannelTree = ({
    groups,
    roster,
    activeChannelId,
    canManage,
    onCreateChannel,
    onEditChannel,
    onEditCategory,
}: ChannelTreeProps) => {
    const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
    const toggle = (categoryId: string) =>
        setCollapsed((current) => {
            const next = new Set(current);
            if (!next.delete(categoryId)) next.add(categoryId);
            return next;
        });

    return (
        <div className="flex flex-col gap-0.5 px-2 pb-3">
            {groups.map(({ category, channels }) => {
                const folded = category !== null && collapsed.has(category.id);
                return (
                    <section key={category?.id ?? 'loose'} aria-label={category?.name ?? 'Channels'}>
                        {category && (
                            <CategoryHeader
                                category={category}
                                collapsed={folded}
                                canManage={canManage}
                                onToggle={() => toggle(category.id)}
                                onCreate={() => onCreateChannel(category.id)}
                                onEdit={() => onEditCategory(category)}
                            />
                        )}
                        {channels
                            .filter((channel) => !folded || channel.id === activeChannelId)
                            .map((channel) => (
                                <div key={channel.id}>
                                    <ChannelRow
                                        channel={channel}
                                        active={channel.id === activeChannelId}
                                        occupants={occupantsOf(roster, channel.id).length}
                                        canManage={canManage}
                                        onEdit={() => onEditChannel(channel)}
                                    />
                                    {channel.kind === 'voice' && (
                                        <VoiceOccupants occupants={occupantsOf(roster, channel.id)} />
                                    )}
                                </div>
                            ))}
                    </section>
                );
            })}
        </div>
    );
};
