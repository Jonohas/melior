import { LogOut, MessageSquare, Mic, MicOff, MonitorOff, MonitorUp, Settings2, Video, VideoOff } from 'lucide-react';
import { Badge, Button } from '@/components/ui';

type RoomDockProps = {
    chatOpen: boolean;
    onToggleChat: () => void;
    /** Chat messages that arrived while the panel was closed. */
    unread: number;
    /** False in a browser that cannot capture a screen: the button is left out. */
    screenShareSupported: boolean;
    screenSharing: boolean;
    /** Whether the room connection is up; a screen can only be shared then. */
    connected: boolean;
    onToggleScreenShare: () => void;
    onOpenDevices: () => void;
    microphoneMuted: boolean;
    onToggleMicrophone: () => void;
    cameraOff: boolean;
    onToggleCamera: () => void;
    /** Leaves the call; the caller decides where to go. */
    onLeave: () => void;
};

/** The controls at the bottom of a room. */
export const RoomDock = ({
    chatOpen,
    onToggleChat,
    unread,
    screenShareSupported,
    screenSharing,
    connected,
    onToggleScreenShare,
    onOpenDevices,
    microphoneMuted,
    onToggleMicrophone,
    cameraOff,
    onToggleCamera,
    onLeave,
}: RoomDockProps) => (
    <div className="flex shrink-0 flex-wrap items-center justify-center gap-2 border-t border-border bg-card px-4 py-3">
        <Button
            type="button"
            variant={microphoneMuted ? 'destructive' : 'outline'}
            aria-pressed={microphoneMuted}
            onClick={onToggleMicrophone}
        >
            {microphoneMuted ? <MicOff /> : <Mic />}
            {microphoneMuted ? 'Unmute' : 'Mute'}
        </Button>
        <Button
            type="button"
            variant={cameraOff ? 'destructive' : 'outline'}
            aria-pressed={cameraOff}
            onClick={onToggleCamera}
        >
            {cameraOff ? <VideoOff /> : <Video />}
            {cameraOff ? 'Camera on' : 'Camera off'}
        </Button>
        <Button type="button" variant="outline" onClick={onOpenDevices}>
            <Settings2 /> Audio and video
        </Button>
        {screenShareSupported && (
            <Button
                type="button"
                variant={screenSharing ? 'secondary' : 'outline'}
                aria-pressed={screenSharing}
                disabled={!connected && !screenSharing}
                onClick={onToggleScreenShare}
            >
                {screenSharing ? <MonitorOff /> : <MonitorUp />}
                {screenSharing ? 'Stop sharing' : 'Share screen'}
            </Button>
        )}
        <Button
            type="button"
            variant={chatOpen ? 'secondary' : 'outline'}
            aria-pressed={chatOpen}
            className="relative"
            onClick={onToggleChat}
        >
            <MessageSquare /> Chat
            {unread > 0 && !chatOpen && (
                <Badge className="absolute -top-2 -right-2 px-1.5" aria-label={`${unread} new`}>
                    {unread}
                </Badge>
            )}
        </Button>
        <Button type="button" variant="destructive" onClick={onLeave}>
            <LogOut /> Leave
        </Button>
    </div>
);
