import { useState } from 'react';
import { canChooseSpeaker, saveDevicePreference } from '@/features/conference/api';
import { DeviceSettingsDialog } from '@/features/conference/components';
import type { VideoQuality } from '@/features/conference/domain';
import { useMediaDevices } from '@/features/conference/hooks';

type InputKind = 'audioinput' | 'videoinput';

type CallDeviceSettingsProps = {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    activeDevices: { audioinput: string | null; videoinput: string | null };
    changeInput: (kind: InputKind, deviceId: string) => Promise<void>;
    speakerId: string;
    onSpeakerChange: (deviceId: string) => void;
    videoQuality: VideoQuality;
    setVideoQuality: (quality: VideoQuality) => Promise<void>;
};

/** The call's microphone, camera and speaker choice, with the error of a switch that failed. */
export const CallDeviceSettings = ({
    open,
    onOpenChange,
    activeDevices,
    changeInput,
    speakerId,
    onSpeakerChange,
    videoQuality,
    setVideoQuality,
}: CallDeviceSettingsProps) => {
    const devices = useMediaDevices(open);
    const [error, setError] = useState<string | null>(null);

    const switchInput = (kind: InputKind, deviceId: string) => {
        setError(null);
        changeInput(kind, deviceId).catch((failure) => {
            const what = kind === 'audioinput' ? 'microphone' : 'camera';
            setError(`Could not switch the ${what}: ${failure instanceof Error ? failure.message : String(failure)}`);
        });
    };

    return (
        <DeviceSettingsDialog
            open={open}
            onOpenChange={(next) => {
                onOpenChange(next);
                if (!next) setError(null);
            }}
            microphones={devices.audioinput}
            cameras={devices.videoinput}
            speakers={devices.audiooutput}
            activeMicrophone={activeDevices.audioinput}
            activeCamera={activeDevices.videoinput}
            activeSpeaker={speakerId}
            canChooseSpeaker={canChooseSpeaker()}
            onChangeMicrophone={(deviceId) => switchInput('audioinput', deviceId)}
            onChangeCamera={(deviceId) => switchInput('videoinput', deviceId)}
            onChangeSpeaker={(deviceId) => {
                onSpeakerChange(deviceId);
                saveDevicePreference('audiooutput', deviceId);
            }}
            quality={videoQuality}
            onChangeQuality={(quality) => {
                setError(null);
                setVideoQuality(quality).catch((failure) => {
                    setError(
                        `Could not change the quality: ${failure instanceof Error ? failure.message : String(failure)}`,
                    );
                });
            }}
            error={error}
        />
    );
};
