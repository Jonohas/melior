import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute, Link, redirect, useRouter } from '@tanstack/react-router';
import { useEffect, useState } from 'react';
import { ConferenceStatusLine, LocalPreview, RemoteVideoGrid, StoppedNotice } from '@/features/conference/components';
import { isChannelId, parseChannelId, parseForceRelay } from '@/features/conference/domain';
import { useConference } from '@/features/conference/hooks';
import { probeRoomAccess } from '@/features/rooms/api';
import { RoomAccessPanel, RoomAccessSummary } from '@/features/rooms/components';
import { useRoomQuery, useSaveRoomMutation } from '@/features/rooms/hooks';
import { recheckSession } from '@/lib/auth';
import { queryKeys } from '@/lib/query-keys';

type RoomSearch = {
    relay?: boolean;
};

export const Route = createFileRoute('/_authenticated/rooms/$channelId')({
    // `?relay=1` (or `true`) forces all media through the TURN relay, to verify that path works.
    validateSearch: (raw: Record<string, unknown>): RoomSearch => (parseForceRelay(raw.relay) ? { relay: true } : {}),
    beforeLoad: ({ params }) => {
        if (!isChannelId(params.channelId)) throw redirect({ to: '/' });
    },
    component: RoomPage,
});

function RoomPage() {
    const params = Route.useParams();
    const { relay } = Route.useSearch();
    const router = useRouter();
    const queryClient = useQueryClient();
    const [authNotice, setAuthNotice] = useState<string | null>(null);
    const channelId = parseChannelId(params.channelId);
    const {
        sessionId,
        username,
        participants,
        localStream,
        remoteStreams,
        directStreams,
        shownPaths,
        status,
        reconnectAttempt,
        stopReason,
        error,
    } = useConference(channelId, {
        forceRelay: relay === true,
        // After a lost connection the conference asks whether the session is still good: a 401 runs the
        // login gate (redirect, or a refusal message), a 403 shows the no-access page.
        checkSession: async () => {
            const result = await recheckSession(queryClient, setAuthNotice);
            if (result === 'forbidden') void router.invalidate();
            return result;
        },
        checkRoomAccess: () => probeRoomAccess(channelId),
    });

    const room = useRoomQuery(channelId);
    const saveRoom = useSaveRoomMutation(channelId);

    // Joining registers an unregistered room, so refresh its access settings once connected.
    useEffect(() => {
        if (status === 'connected') {
            void queryClient.invalidateQueries({ queryKey: queryKeys.rooms.detail(channelId) });
        }
    }, [status, channelId, queryClient]);

    return (
        <div className="flex items-center flex-col w-full min-h-screen gap-4 p-4">
            <ConferenceStatusLine
                channelId={channelId}
                sessionId={sessionId}
                status={status}
                reconnectAttempt={reconnectAttempt}
                error={error}
            />
            {stopReason && <StoppedNotice reason={stopReason} />}
            {authNotice && (
                <p className="text-red-500" role="alert">
                    {authNotice}
                </p>
            )}
            <Link to="/" className="underline">
                Leave
            </Link>
            {room.data?.isOwner && (
                <RoomAccessPanel
                    room={room.data}
                    saving={saveRoom.isPending}
                    saveError={saveRoom.error?.message ?? null}
                    onSave={(settings) => saveRoom.mutate(settings)}
                />
            )}
            {room.data && !room.data.isOwner && <RoomAccessSummary room={room.data} />}
            <LocalPreview stream={localStream} username={username} />
            <RemoteVideoGrid
                streams={remoteStreams}
                directStreams={directStreams}
                shownPaths={shownPaths}
                usernames={participants}
            />
        </div>
    );
}
