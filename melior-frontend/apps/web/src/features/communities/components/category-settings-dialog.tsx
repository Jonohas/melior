import { useState } from 'react';
import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    Tabs,
    TabsContent,
    TabsList,
    TabsTrigger,
} from '@/components/ui';
import type { Category, CategoryChange, CommunityView } from '@/features/communities/domain';
import { useDeleteCategory, useUpdateCategory } from '@/features/communities/hooks';
import { ConfirmDialog } from './confirm-dialog';
import { NameForm } from './name-form';
import { PermissionsForm } from './permissions-form';

type CategorySettingsDialogProps = {
    view: CommunityView;
    category: Category;
    open: boolean;
    onOpenChange: (open: boolean) => void;
};

/** A category's name and the permissions its synced channels use. */
export const CategorySettingsDialog = ({ view, category, open, onOpenChange }: CategorySettingsDialogProps) => {
    const update = useUpdateCategory(view.id);
    const remove = useDeleteCategory(view.id);
    const [confirmDelete, setConfirmDelete] = useState(false);
    const error = update.error?.message ?? null;
    const save = (change: CategoryChange) => update.mutate({ categoryId: category.id, change });

    const deleteCategory = () =>
        remove.mutate(category.id, {
            onSuccess: () => {
                setConfirmDelete(false);
                onOpenChange(false);
            },
        });

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-2xl">
                <DialogHeader>
                    <DialogTitle>{category.name}</DialogTitle>
                    <DialogDescription>Category settings. Synced channels follow its permissions.</DialogDescription>
                </DialogHeader>
                <Tabs defaultValue="overview">
                    <TabsList>
                        <TabsTrigger value="overview">Overview</TabsTrigger>
                        <TabsTrigger value="permissions">Permissions</TabsTrigger>
                    </TabsList>
                    <TabsContent value="overview" className="flex flex-col gap-4">
                        <NameForm
                            label="Category name"
                            initial={category.name}
                            submitLabel="Save"
                            pending={update.isPending}
                            error={error}
                            onSubmit={(name) => save({ name })}
                        />
                        <Button
                            type="button"
                            variant="destructive"
                            className="w-fit"
                            onClick={() => setConfirmDelete(true)}
                        >
                            Delete category
                        </Button>
                    </TabsContent>
                    <TabsContent value="permissions">
                        <PermissionsForm
                            view={view}
                            initial={category.overrides}
                            pending={update.isPending}
                            error={error}
                            onSave={(overrides) => save({ overrides })}
                        />
                    </TabsContent>
                </Tabs>
            </DialogContent>
            <ConfirmDialog
                open={confirmDelete}
                onOpenChange={setConfirmDelete}
                title={`Delete ${category.name}?`}
                confirmLabel="Delete category"
                pending={remove.isPending}
                error={remove.error?.message ?? null}
                onConfirm={deleteCategory}
            >
                Its channels stay, without a category.
            </ConfirmDialog>
        </Dialog>
    );
};
