import type {ReactNode} from "react";

type BlackmarkPageProps = {
    title: string;
    copy?: string;
    actions?: ReactNode;
    specs?: ReactNode;
    children?: ReactNode;
};

export function BlackmarkPage({actions, children, copy, specs, title}: BlackmarkPageProps) {
    return (
        <>
            <section className="relative px-8 pb-10 pt-12">
                <span className="bm-watermark" aria-hidden="true" key={`mark-${title}`}>
                    {title}
                </span>
                <div className="relative flex flex-wrap items-end justify-between gap-6">
                    <div>
                        <h1
                            className="bm-title max-w-[12ch] text-[3.5rem] leading-none font-bold tracking-tight uppercase"
                            key={`title-${title}`}
                        >
                            {title}
                        </h1>
                        {copy ? (
                            <p className="bm-copy mt-4 max-w-[46ch] text-base leading-relaxed font-light">
                                {copy}
                            </p>
                        ) : null}
                    </div>
                    {actions}
                </div>
                <div className="bm-stripe bm-stripe-draw mt-8 h-1 w-28" key={`stripe-${title}`} aria-hidden="true"/>
            </section>
            {specs}
            {children}
        </>
    );
}

export function BlackmarkSpec({
    label,
    value,
    readout = false,
    warning = false,
}: {
    label: string;
    value: ReactNode;
    readout?: boolean;
    warning?: boolean;
}) {
    return (
        <article className="bm-spec">
            <div className={`value${readout ? " bm-readout" : ""}${warning ? " text-warning" : ""}`}>
                {value}
            </div>
            <div className="label">{label}</div>
        </article>
    );
}
