type MeliorLogoProps = {
    className?: string;
};

/** The Melior mark, the same drawing as the favicon. */
export const MeliorLogo = ({ className }: MeliorLogoProps) => (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" role="img" aria-label="Melior" className={className}>
        <rect width="32" height="32" rx="6" fill="#262524" />
        <path d="M7 24V8h4l5 8 5-8h4v16h-3.5V14.5L16 22l-5.5-7.5V24z" fill="#ffb800" />
    </svg>
);
