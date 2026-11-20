import type { Path } from '@/features/conference/domain';
import { PathVideo } from './path-video';

type RemoteVideoProps = {
    /** The participant's media through the SFU. */
    forwarded?: MediaStream;
    /** The participant's media over the direct link. */
    direct?: MediaStream;
    shownPath: Path;
    username?: string;
};

/** Both paths stay attached and playing; only the shown one is visible and audible. */
export const RemoteVideo = ({ forwarded, direct, shownPath, username }: RemoteVideoProps) => {
    return (
        <div className="border-2 rounded border-green-500">
            <div className="relative aspect-video bg-black">
                {forwarded && <PathVideo stream={forwarded} shown={shownPath === 'forwarded'} />}
                {direct && <PathVideo stream={direct} shown={shownPath === 'direct'} />}
            </div>
            {username && <p className="px-2 py-1">{username}</p>}
        </div>
    );
};
