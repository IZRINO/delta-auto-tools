import {useState} from "react";
import {RiPaletteLine, RiStackLine, RiInformationLine} from "@remixicon/react";

import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
} from "@/components/ui/dialog";
import {Tabs, TabsContent, TabsList, TabsTrigger} from "@/components/ui/tabs";
import {ThemePanel} from "@/components/app/theme-panel";
import {ProfilePanel} from "@/components/app/profile-panel";
import {AboutPanel} from "@/components/app/about-page";

type SettingsDialogProps = {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    /** 初始打开的 Tab，默认 theme。 */
    initialTab?: "theme" | "profile" | "about";
};

/**
 * 统一设置 Dialog：含「主题 / 配置 / 关于」三个 Tab。
 *
 * 替代原独立的 AboutDialog 入口，Rail 底部「设置」按钮打开此 Dialog。
 * 内部三 Tab 各自懒挂载，切到关于 Tab 时才触发 AboutPanel 的数据加载。
 */
export function SettingsDialog({open, onOpenChange, initialTab = "theme"}: SettingsDialogProps) {
    const [tab, setTab] = useState<"theme" | "profile" | "about">(initialTab);

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="sm:max-w-3xl w-[min(96vw,900px)] max-h-[80vh] overflow-hidden">
                <DialogHeader>
                    <DialogTitle>设置</DialogTitle>
                    <DialogDescription>
                        主题外观、多配置管理与软件信息
                    </DialogDescription>
                </DialogHeader>

                <Tabs
                    value={tab}
                    onValueChange={(v) => setTab(v as typeof tab)}
                    className="flex min-h-0 flex-1 flex-col gap-3"
                >
                    <TabsList className="w-full">
                        <TabsTrigger value="theme" className="flex-1">
                            <RiPaletteLine className="size-3.5" data-icon="inline-start" aria-hidden="true"/>
                            主题
                        </TabsTrigger>
                        <TabsTrigger value="profile" className="flex-1">
                            <RiStackLine className="size-3.5" data-icon="inline-start" aria-hidden="true"/>
                            配置
                        </TabsTrigger>
                        <TabsTrigger value="about" className="flex-1">
                            <RiInformationLine className="size-3.5" data-icon="inline-start" aria-hidden="true"/>
                            关于
                        </TabsTrigger>
                    </TabsList>

                    <TabsContent value="theme" className="min-h-0 flex-1 overflow-y-auto [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                        <ThemePanel/>
                    </TabsContent>
                    <TabsContent value="profile" className="min-h-0 flex-1 overflow-y-auto [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                        <ProfilePanel/>
                    </TabsContent>
                    <TabsContent value="about" className="min-h-0 flex-1 overflow-y-auto [-ms-overflow-style:none] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
                        <AboutPanel active={open && tab === "about"}/>
                    </TabsContent>
                </Tabs>
            </DialogContent>
        </Dialog>
    );
}
