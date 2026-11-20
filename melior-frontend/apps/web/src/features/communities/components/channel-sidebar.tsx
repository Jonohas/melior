import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui';
import { abilitiesOf, buildChannelTree, type CommunityView } from '@/features/communities/domain';
import { useCommunityFeed, useCreateCategory, useLeaveCommunity } from '@/features/communities/hooks';
import { CategorySettingsDialog } from './category-settings-dialog';
import { ChannelSettingsDialog } from './channel-settings-dialog';
import { ChannelTree } from './channel-tree';
import { ConfirmDialog } from './confirm-dialog';
import { CreateChannelDialog } from './create-channel-dialog';
import { InviteDialog } from './invite-dialog';
import { NameForm } from './name-form';
import { ServerMenu, type SidebarDialog } from './server-menu';
import { ServerSettingsDialog } from './server-settings-dialog';

type ChannelSidebarProps = {
    view: CommunityView;
    activeChannelId: string | null;
};

type DialogProps = { view: CommunityView; onClose: () => void };

const CreateCategoryDialog = ({ view, onClose }: DialogProps) => {
    const create = useCreateCategory(view.id);
    return (
        <Dialog open onOpenChange={(open) => !open && onClose()}>
            <DialogContent>
                <DialogHeader>
                    <DialogTitle>Create category</DialogTitle>
                </DialogHeader>
                <NameForm
                    label="Category name"
                    submitLabel="Create"
                    pending={create.isPending}
                    error={create.error?.message ?? null}
                    onSubmit={(name) => create.mutate(name, { onSuccess: onClose })}
                />
            </DialogContent>
        </Dialog>
    );
};

const LeaveDialog = ({ view, onClose }: DialogProps) => {
    const navigate = useNavigate();
    const leave = useLeaveCommunity(view.id, view.me.userId);
    return (
        <ConfirmDialog
            open
            onOpenChange={(open) => !open && onClose()}
            title={`Leave ${view.name}?`}
            confirmLabel="Leave server"
            pending={leave.isPending}
            error={leave.error?.message ?? null}
            onConfirm={() => leave.mutate(undefined, { onSuccess: () => void navigate({ to: '/' }) })}
        >
            You can only come back with a new invite.
        </ConfirmDialog>
    );
};

/** The open dialog, mounted only while open so each starts from the current data. */
const SidebarDialogs = ({ view, dialog, onClose }: DialogProps & { dialog: SidebarDialog }) => {
    const openProps = { open: true, onOpenChange: (open: boolean) => !open && onClose() };
    switch (dialog.kind) {
        case 'none':
            return null;
        case 'settings':
            return <ServerSettingsDialog view={view} {...openProps} />;
        case 'invite':
            return <InviteDialog view={view} {...openProps} />;
        case 'create-channel':
            return <CreateChannelDialog view={view} categoryId={dialog.categoryId} {...openProps} />;
        case 'create-category':
            return <CreateCategoryDialog view={view} onClose={onClose} />;
        case 'leave':
            return <LeaveDialog view={view} onClose={onClose} />;
        case 'channel-settings': {
            const channel = view.channels.find((entry) => entry.id === dialog.channelId);
            return channel ? <ChannelSettingsDialog view={view} channel={channel} {...openProps} /> : null;
        }
        case 'category-settings': {
            const category = view.categories.find((entry) => entry.id === dialog.categoryId);
            return category ? <CategorySettingsDialog view={view} category={category} {...openProps} /> : null;
        }
    }
};

/** The server's column: its menu, the channel list with who is in each voice channel, and the dialogs. */
export const ChannelSidebar = ({ view, activeChannelId }: ChannelSidebarProps) => {
    const { roster } = useCommunityFeed();
    const [dialog, setDialog] = useState<SidebarDialog>({ kind: 'none' });
    const abilities = abilitiesOf(view.me);

    return (
        <aside
            aria-label={`${view.name} channels`}
            className="flex w-60 shrink-0 flex-col border-r border-border bg-card"
        >
            <ServerMenu name={view.name} abilities={abilities} onOpen={setDialog} />
            <nav className="min-h-0 flex-1 overflow-y-auto pt-2">
                <ChannelTree
                    groups={buildChannelTree(view, abilities.manageChannels)}
                    roster={roster}
                    activeChannelId={activeChannelId}
                    canManage={abilities.manageChannels}
                    onCreateChannel={(categoryId) => setDialog({ kind: 'create-channel', categoryId })}
                    onEditChannel={(channel) => setDialog({ kind: 'channel-settings', channelId: channel.id })}
                    onEditCategory={(category) => setDialog({ kind: 'category-settings', categoryId: category.id })}
                />
                {view.channels.length === 0 && (
                    <p className="px-4 text-sm text-muted-foreground">There are no channels you can see.</p>
                )}
            </nav>
            <SidebarDialogs view={view} dialog={dialog} onClose={() => setDialog({ kind: 'none' })} />
        </aside>
    );
};
