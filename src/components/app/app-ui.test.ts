import {createElement} from "react";
import {renderToStaticMarkup} from "react-dom/server";
import {describe, expect, it} from "vitest";

import {ConfigRow, FieldUnit, StampFold} from "@/components/app/app-ui";

describe("ConfigRow", () => {
    it("值列不截断且可放按钮", () => {
        const html = renderToStaticMarkup(
            createElement(ConfigRow, {
                label: "热键",
                value: createElement("button", {type: "button"}, "录制"),
            }),
        );
        expect(html).not.toContain("truncate");
        expect(html).toContain("录制");
        expect(html).toContain("热键");
    });
});

describe("StampFold", () => {
    it("原生按钮且不含 daisyUI btn", () => {
        const html = renderToStaticMarkup(
            createElement(StampFold, {label: "高级校准"}),
        );
        expect(html).toContain("高级校准");
        expect(html).toContain("border-base-content");
        expect(html).not.toMatch(/\bbtn\b/);
    });
});

describe("FieldUnit", () => {
    it("标题条用内容色粗描边", () => {
        const html = renderToStaticMarkup(
            createElement(FieldUnit, {header: "总开关"}, "body"),
        );
        expect(html).toContain("总开关");
        expect(html).toContain("border-b-2");
        expect(html).toContain("border-base-content");
    });
});
