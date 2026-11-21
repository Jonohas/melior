import { useVoiceSession } from '@/features/communities/hooks';
import { RemoteAudio } from '@/features/conference/components';

/** The call's sound, mounted beside the routes so it plays on every page while you are in a call. */
export const VoiceCallAudio = () => {
    const { session, call, speakerId } = useVoiceSession();
    const { conference } = call;
    if (!session.active) return null;
    return (
        <RemoteAudio
            streams={conference.remoteStreams}
            directStreams={conference.directStreams}
            screens={conference.screenStreams}
            directScreens={conference.directScreens}
            shownPaths={conference.shownPaths}
            speakerId={speakerId}
        />
    );
};
