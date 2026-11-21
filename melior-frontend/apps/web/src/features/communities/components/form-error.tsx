/** A refusal under a form: the client's check or the server's answer. */
export const FormError = ({ message }: { message: string | null | undefined }) =>
    message ? (
        <p className="text-sm text-destructive" role="alert">
            {message}
        </p>
    ) : null;
