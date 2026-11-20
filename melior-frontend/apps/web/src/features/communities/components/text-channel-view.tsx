import { TopBar } from '@/components';
import { ChatPanel } from '@/features/chat/components';
import { type Channel, hasPermission } from '@/features/communities/domain';
import { useTextChannelChat } from '@/features/communities/hooks';
import { ChannelIcon } from './channel-tree';
import { MemberListButton } from './member-list';

type TextChannelViewProps = {
    channel: Channel;
    username: string;
};

const NO_PERMISSION = 'You do not have permission to send messages in this channel.';

/**
 * A text channel: its history and live messages, and the box to write one if allowed. Posting is plain HTTP, so it
 * works while the feed reconnects; the history is reloaded when the feed is back.
 */
export const TextChannelView = ({ channel, username }: TextChannelViewProps) => {
    const chat = useTextChannelChat(channel.id, username);
    const mayWrite = hasPermission(channel.myPermissions, 'send_messages');

    return (
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <TopBar
                title={
                    <>
                        <ChannelIcon channel={channel} className="text-muted-foreground" />
                        {channel.name}
                    </>
                }
                subtitle={channel.topic || undefined}
            >
                <MemberListButton />
            </TopBar>
            <ChatPanel
                fill
                state={chat.state}
                ownName={username}
                canSend={mayWrite}
                pausedReason={NO_PERMISSION}
                placeholder={`Message #${channel.name}`}
                onSend={chat.submit}
                onRetry={chat.retry}
                onLoadEarlier={chat.loadEarlier}
                onReloadHistory={chat.reloadHistory}
                onDismissNotice={chat.dismissNotice}
            />
        </div>
    );
};
