/** What went wrong with a community request, by HTTP status. */
export type RequestFailure = 'invalid' | 'forbidden' | 'not_found' | 'conflict' | 'gone' | 'unavailable' | 'failed';

export const failureOfStatus = (status: number): RequestFailure => {
    switch (status) {
        case 400:
            return 'invalid';
        case 403:
            return 'forbidden';
        case 404:
            return 'not_found';
        case 409:
            return 'conflict';
        case 410:
            return 'gone';
        case 503:
            return 'unavailable';
        default:
            return 'failed';
    }
};

const DEFAULT_MESSAGE: Record<RequestFailure, string> = {
    invalid: 'The server rejected the request.',
    forbidden: 'You do not have permission to do that.',
    not_found: 'It does not exist (any more).',
    conflict: 'That is not possible right now.',
    gone: 'This invite is no longer valid.',
    unavailable: 'The server is unavailable. Try again shortly.',
    failed: 'The request failed.',
};

/** A failed community request. The message is the server's own reason when it gave a short one. */
export class CommunityRequestError extends Error {
    readonly failure: RequestFailure;
    readonly status: number;

    constructor(status: number, serverText: string | null) {
        const failure = failureOfStatus(status);
        super(serverText ?? DEFAULT_MESSAGE[failure]);
        this.name = 'CommunityRequestError';
        this.failure = failure;
        this.status = status;
    }
}

/** True when the error is a community request that failed in this way. */
export const isRequestFailure = (error: unknown, failure: RequestFailure): boolean =>
    error instanceof CommunityRequestError && error.failure === failure;
