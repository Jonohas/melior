import { Link } from '@tanstack/react-router';
import { Lock } from 'lucide-react';
import { StatePage } from '@/components';
import { Button } from '@/components/ui';
import { describeStop, type StopReason } from '@/features/conference/domain';

type StoppedNoticeProps = {
    reason: StopReason;
};

/** The room is closed to you: the page says why, and where to go next. */
export const StoppedNotice = ({ reason }: StoppedNoticeProps) => (
    <StatePage
        alert
        icon={Lock}
        title={describeStop(reason)}
        actions={
            <Button asChild>
                <Link to="/">Back to the lobby</Link>
            </Button>
        }
    >
        The room and its chat are no longer shown to you.
    </StatePage>
);
