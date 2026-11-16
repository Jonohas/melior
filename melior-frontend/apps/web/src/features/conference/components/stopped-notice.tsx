import { Link } from '@tanstack/react-router';
import { describeStop, type StopReason } from '@/features/conference/domain';

type StoppedNoticeProps = {
    reason: StopReason;
};

export const StoppedNotice = ({ reason }: StoppedNoticeProps) => {
    return (
        <div className="flex flex-col items-center gap-2" role="alert">
            <p className="text-red-500">{describeStop(reason)}</p>
            <Link to="/" className="underline">
                Back to the lobby
            </Link>
        </div>
    );
};
