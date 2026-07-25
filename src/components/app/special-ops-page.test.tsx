import {createElement} from "react";
import {renderToStaticMarkup} from "react-dom/server";
import {describe, expect, it} from "vitest";

import {SpecialOpsPage} from "@/components/app/special-ops-page";

describe("SpecialOpsPage 登录试运行配置", () => {
    it("显示可执行文件、紧急热键与单账号试运行边界", () => {
        const html = renderToStaticMarkup(createElement(SpecialOpsPage));

        expect(html).toContain("WeGame 可执行文件");
        expect(html).toContain("游戏可执行文件");
        expect(html).toContain("录制紧急停止热键");
        expect(html).toContain("仅运行所选账号一次，不执行收取、生产、购买或子弹兑换");
        expect(html).toContain("先把游戏置顶");
        expect(html).toContain("运行时不搜索或滚动窗口");
        expect(html).toContain('class="card card-border');
        expect(html).toContain('class="select');
        expect(html).toContain('role="alert"');
    });
});
