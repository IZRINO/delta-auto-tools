import type {ReactNode} from "react";

import type {BlackmarkPaneId} from "@/components/app/tool-nav";

function glyph(children: ReactNode) {
    return (
        <svg aria-hidden="true" fill="none" height="22" viewBox="0 0 24 24" width="22">
            {children}
        </svg>
    );
}

const stroke = {
    stroke: "currentColor",
    strokeLinecap: "square" as const,
    strokeLinejoin: "miter" as const,
    strokeWidth: 1.75,
};

export function BlackmarkGlyph({id}: {id: BlackmarkPaneId}) {
    switch (id) {
        case "favorites":
            return glyph(
                <>
                    <path d="M12 3.5 13.8 10.2 20.5 12 13.8 13.8 12 20.5 10.2 13.8 3.5 12 10.2 10.2Z" {...stroke}/>
                    <path d="M12 8.5v7M8.5 12h7" {...stroke}/>
                </>,
            );
        case "timer":
            return glyph(
                <>
                    <path d="M8 4.5h8" {...stroke}/>
                    <path d="M12 4.5v2.2" {...stroke}/>
                    <path d="M12 8.2v4.3l3.2 2.1" {...stroke}/>
                    <path d="M19.2 12a7.2 7.2 0 1 1-2.1-5.1" {...stroke}/>
                </>,
            );
        case "counter":
            return glyph(
                <>
                    <path d="M5 18V9" {...stroke}/>
                    <path d="M10.5 18V6" {...stroke}/>
                    <path d="M16 18v-7" {...stroke}/>
                    <path d="M19.5 6.5v4M17.5 8.5h4" {...stroke}/>
                </>,
            );
        case "rapidfire":
            return glyph(
                <>
                    <path d="M4.5 16.5 9 8.5l3.2 5.2 3.3-7.2 4 10" {...stroke}/>
                    <path d="M4 19.5h16" {...stroke}/>
                </>,
            );
        case "strategy":
            return glyph(
                <>
                    <path d="M4.5 7.5h11.5v12H4.5z" {...stroke}/>
                    <path d="M16 10.5h3.5v9.5H8.5v-3" {...stroke}/>
                    <path d="M4.5 12.5h11.5" {...stroke}/>
                    <path d="M10.2 7.5 13.8 19.5" {...stroke}/>
                </>,
            );
        case "recognition":
            return glyph(
                <>
                    <path d="M12 12.5v.01" {...stroke} strokeWidth={2.4}/>
                    <path d="M8.2 16.2a5.5 5.5 0 1 1 7.6 0" {...stroke}/>
                    <path d="M5.4 18.6a9 9 0 1 1 13.2 0" {...stroke}/>
                </>,
            );
        case "privacyScreen":
            return glyph(
                <>
                    <path d="M3.5 12h17" {...stroke}/>
                    <path d="M6 8.5 12 5.5 18 8.5" {...stroke}/>
                    <path d="M6 15.5 12 18.5 18 15.5" {...stroke}/>
                </>,
            );
        case "specialOps":
            return glyph(
                <>
                    <path d="M5 7.2 12 4.2 19 7.2v6.2c0 3.4-2.8 5.7-7 7.4-4.2-1.7-7-4-7-7.4z" {...stroke}/>
                    <path d="M12 9.4v5" {...stroke}/>
                    <path d="M9.5 11.9h5" {...stroke}/>
                </>,
            );
        case "morse":
            return glyph(
                <>
                    <rect fill="currentColor" height="3" width="3" x="2.5" y="10.5"/>
                    <rect fill="currentColor" height="3" width="3" x="7.5" y="10.5"/>
                    <rect fill="currentColor" height="3" width="3" x="12.5" y="10.5"/>
                    <rect fill="currentColor" height="3" width="6.5" x="17" y="10.5"/>
                </>,
            );
        case "settings":
            return glyph(
                <>
                    <path d="M9 4.2h6v2.4h4.2v4.2H21.5v2.4H19.2v4.2H15V19.8H9v-2.4H4.8v-4.2H2.5v-2.4H4.8V6.6H9z" {...stroke}/>
                    <path d="M9.6 9.6h4.8v4.8H9.6z" {...stroke}/>
                </>,
            );
    }
}
