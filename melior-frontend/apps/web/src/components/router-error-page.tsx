import { Link } from '@tanstack/react-router';
import { TriangleAlert } from 'lucide-react';
import { Button } from '@/components/ui';
import { SessionRejectedError } from '@/lib/auth';
import { StatePage } from './state-page';

type RouterErrorPageProps = {
    error: Error;
};

export const RouterErrorPage = ({ error }: RouterErrorPageProps) => {
    const retryable = error instanceof SessionRejectedError && error.retryable;

    return (
        <StatePage
            standalone
            alert
            icon={TriangleAlert}
            title="Something went wrong"
            actions={
                <>
                    {retryable && (
                        <Button type="button" onClick={() => window.location.reload()}>
                            Try again
                        </Button>
                    )}
                    <Button asChild variant="outline">
                        <Link to="/">Back to the lobby</Link>
                    </Button>
                </>
            }
        >
            {error.message}
        </StatePage>
    );
};
