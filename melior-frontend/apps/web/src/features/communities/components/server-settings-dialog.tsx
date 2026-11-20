import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    Tabs,
    TabsContent,
    TabsList,
    TabsTrigger,
} from '@/components/ui';
import { abilitiesOf, type CommunityView } from '@/features/communities/domain';
import { useDeleteCommunity, useRenameCommunity, useVoiceSession } from '@/features/communities/hooks';
import { ConfirmDialog } from './confirm-dialog';
import { InvitesTab } from './invites-tab';
import { MembersTab } from './members-tab';
import { NameForm } from './name-form';
import { RolesTab } from './roles-tab';

type ServerSettingsDialogProps = {
    view: CommunityView;
    open: boolean;
    onOpenChange: (open: boolean) => void;
};

const OverviewTab = ({ view }: { view: CommunityView }) => {
    const navigate = useNavigate();
    const rename = useRenameCommunity(view.id);
    const remove = useDeleteCommunity(view.id);
    const { exitCommunity } = useVoiceSession();
    const onDeleted = () => {
        // The owner chose this, like leaving: the call there ends without a notice.
        exitCommunity(view.id, 'left');
        void navigate({ to: '/' });
    };
    const [confirmDelete, setConfirmDelete] = useState(false);
    const abilities = abilitiesOf(view.me);

    return (
        <div className="flex flex-col gap-4">
            {abilities.renameServer && (
                <NameForm
                    label="Server name"
                    initial={view.name}
                    submitLabel="Save"
                    pending={rename.isPending}
                    error={rename.error?.message ?? null}
                    onSubmit={(name) => rename.mutate(name)}
                />
            )}
            <p className="text-sm text-muted-foreground">Owner: {view.ownerUsername}</p>
            {abilities.deleteServer && (
                <Button type="button" variant="destructive" className="w-fit" onClick={() => setConfirmDelete(true)}>
                    Delete server
                </Button>
            )}
            <ConfirmDialog
                open={confirmDelete}
                onOpenChange={setConfirmDelete}
                title={`Delete ${view.name}?`}
                confirmLabel="Delete server"
                pending={remove.isPending}
                error={remove.error?.message ?? null}
                onConfirm={() => remove.mutate(undefined, { onSuccess: onDeleted })}
            >
                Every channel and every message goes with it. This cannot be undone.
            </ConfirmDialog>
        </div>
    );
};

/** The server's settings; only the tabs the caller can use are shown. */
export const ServerSettingsDialog = ({ view, open, onOpenChange }: ServerSettingsDialogProps) => {
    const abilities = abilitiesOf(view.me);
    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-3xl">
                <DialogHeader>
                    <DialogTitle>{view.name}</DialogTitle>
                    <DialogDescription>Server settings</DialogDescription>
                </DialogHeader>
                <Tabs defaultValue="overview">
                    <TabsList>
                        <TabsTrigger value="overview">Overview</TabsTrigger>
                        <TabsTrigger value="roles">Roles</TabsTrigger>
                        <TabsTrigger value="members">Members</TabsTrigger>
                        {abilities.viewInvites && <TabsTrigger value="invites">Invites</TabsTrigger>}
                    </TabsList>
                    <TabsContent value="overview">
                        <OverviewTab view={view} />
                    </TabsContent>
                    <TabsContent value="roles">
                        <RolesTab view={view} />
                    </TabsContent>
                    <TabsContent value="members">
                        <MembersTab view={view} />
                    </TabsContent>
                    {abilities.viewInvites && (
                        <TabsContent value="invites">
                            <InvitesTab view={view} />
                        </TabsContent>
                    )}
                </Tabs>
            </DialogContent>
        </Dialog>
    );
};
