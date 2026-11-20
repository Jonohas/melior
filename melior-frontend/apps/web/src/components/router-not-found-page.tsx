import { Link } from '@tanstack/react-router';
import { FileQuestion } from 'lucide-react';
import { Button } from '@/components/ui';
import { StatePage } from './state-page';

export const RouterNotFoundPage = () => (
    <StatePage
        standalone
        icon={FileQuestion}
        title="Page not found"
        actions={
            <Button asChild>
                <Link to="/">Back to the lobby</Link>
            </Button>
        }
    />
);
