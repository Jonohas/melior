import { useNavigate } from '@tanstack/react-router';
import { useId, useState } from 'react';
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    Label,
    Switch,
    Tabs,
    TabsContent,
    TabsList,
    TabsTrigger,
} from '@/components/ui';
import {
    type Channel,
    type ChannelChange,
    type CommunityView,
    effectiveOverrides,
} from '@/features/communities/domain';
import { useDeleteChannel, useUpdateChannel } from '@/features/communities/hooks';
import { ChannelMediaForm } from './channel-media-form';
import { ChannelOverviewForm } from './channel-overview-form';
import { ConfirmDialog } from './confirm-dialog';
import { PermissionsForm } from './permissions-form';

type ChannelSettingsDialogProps = {
    view: CommunityView;
    channel: Channel;
    open: boolean;
    onOpenChange: (open: boolean) => void;
};

type SyncSwitchProps = { categoryName: string; synced: boolean; pending: boolean; onSync: () => void };

const SyncSwitch = ({ categoryName, synced, pending, onSync }: SyncSwitchProps) => {
    const id = useId();
    return (
        <div className="flex items-center justify-between gap-4 rounded-md border border-border p-3">
            <div>
                <Label htmlFor={id}>Synced with {categoryName}</Label>
                <p className="text-xs text-muted-foreground">
                    {synced
                        ? 'This channel uses the category’s permissions. Saving changes here ends the sync.'
                        : 'This channel has its own permissions. Turn on to use the category’s again.'}
                </p>
            </div>
            <Switch id={id} checked={synced} disabled={synced || pending} onCheckedChange={onSync} />
        </div>
    );
};

/** A channel's settings: overview, media (voice only) and permission overrides. */
export const ChannelSettingsDialog = ({ view, channel, open, onOpenChange }: ChannelSettingsDialogProps) => {
    const navigate = useNavigate();
    const update = useUpdateChannel(view.id);
    const remove = useDeleteChannel(view.id);
    const [confirmDelete, setConfirmDelete] = useState(false);
    const category = view.categories.find((entry) => entry.id === channel.categoryId);
    const error = update.error?.message ?? null;
    const save = (change: ChannelChange) => update.mutate({ channelId: channel.id, change });

    const deleteChannel = () =>
        remove.mutate(channel.id, {
            onSuccess: () => {
                setConfirmDelete(false);
                onOpenChange(false);
                void navigate({ to: '/c/$communityId', params: { communityId: view.id } });
            },
        });

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-2xl">
                <DialogHeader>
                    <DialogTitle>{channel.name}</DialogTitle>
                    <DialogDescription>Channel settings</DialogDescription>
                </DialogHeader>
                <Tabs defaultValue="overview">
                    <TabsList>
                        <TabsTrigger value="overview">Overview</TabsTrigger>
                        {channel.kind === 'voice' && <TabsTrigger value="media">Media</TabsTrigger>}
                        <TabsTrigger value="permissions">Permissions</TabsTrigger>
                    </TabsList>
                    <TabsContent value="overview">
                        <ChannelOverviewForm
                            view={view}
                            channel={channel}
                            pending={update.isPending}
                            error={error}
                            onSave={save}
                            onDelete={() => setConfirmDelete(true)}
                        />
                    </TabsContent>
                    {channel.kind === 'voice' && (
                        <TabsContent value="media">
                            <ChannelMediaForm
                                channel={channel}
                                pending={update.isPending}
                                error={error}
                                onSave={save}
                            />
                        </TabsContent>
                    )}
                    <TabsContent value="permissions">
                        <PermissionsForm
                            key={`${channel.synced}`}
                            view={view}
                            initial={effectiveOverrides(channel, view.categories)}
                            pending={update.isPending}
                            error={error}
                            onSave={(overrides) => save({ overrides })}
                            header={
                                category && (
                                    <SyncSwitch
                                        categoryName={category.name}
                                        synced={channel.synced}
                                        pending={update.isPending}
                                        onSync={() => save({ synced: true })}
                                    />
                                )
                            }
                        />
                    </TabsContent>
                </Tabs>
            </DialogContent>
            <ConfirmDialog
                open={confirmDelete}
                onOpenChange={setConfirmDelete}
                title={`Delete ${channel.name}?`}
                confirmLabel="Delete channel"
                pending={remove.isPending}
                error={remove.error?.message ?? null}
                onConfirm={deleteChannel}
            >
                The channel and all its messages are deleted. This cannot be undone.
            </ConfirmDialog>
        </Dialog>
    );
};
