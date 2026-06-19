import {AddCardButton} from "@/components/app/app-ui";
import {cn} from "@/lib/utils";
import type {ReactNode} from "react";

type SyncCardListProps<T> = {
    items: T[];
    renderCard: (item: T, index: number) => ReactNode;
    addButtonTitle: string;
    addButtonDescription: string;
    onAdd: () => void;
    disabled: boolean;
    className?: string;
};

/**
 * 同步卡片列表：封装 section 网格 + AddCardButton 的重复布局。
 */
export function SyncCardList<T>({
                                    items,
                                    renderCard,
                                    addButtonTitle,
                                    addButtonDescription,
                                    onAdd,
                                    disabled,
                                    className,
                                }: SyncCardListProps<T>) {
    return (
        <section className={cn("col-span-12 grid min-h-0 gap-3 xl:grid-cols-2", className)}>
            {items.map((item, index) => renderCard(item, index))}
            <AddCardButton
                className="min-h-36"
                disabled={disabled}
                title={addButtonTitle}
                description={addButtonDescription}
                onClick={onAdd}
            />
        </section>
    );
}
