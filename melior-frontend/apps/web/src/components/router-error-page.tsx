import { Link } from '@tanstack/react-router';
import { SessionRejectedError } from '@/lib/auth';

type RouterErrorPageProps = {
    error: Error;
};

export const RouterErrorPage = ({ error }: RouterErrorPageProps) => {
    const retryable = error instanceof SessionRejectedError && error.retryable;

    return (
        <div className="flex flex-col items-center gap-4 p-8">
            <h1 className="text-heading3">Something went wrong</h1>
            <p className="text-red-500" role="alert">
                {error.message}
            </p>
            {retryable && (
                <button type="button" className="underline" onClick={() => window.location.reload()}>
                    Try again
                </button>
            )}
            <Link to="/" className="underline">
                Back to the lobby
            </Link>
        </div>
    );
};
