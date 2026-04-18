import { useMemo, type CSSProperties } from "react";
import { RiRadarLine } from "@remixicon/react";

import { MorsePage } from "@/components/app/morse-page";
import { Sidebar, SidebarContent, SidebarGroup, SidebarGroupContent, SidebarGroupLabel, SidebarHeader, SidebarInset, SidebarMenu, SidebarMenuButton, SidebarMenuItem, SidebarProvider, SidebarSeparator } from "@/components/ui/sidebar";

import "./App.css";

const appShellStyle = {
  "--sidebar-width": "14rem",
  "--sidebar-width-icon": "3rem",
} as CSSProperties;

function App() {
  const overlayMode = useMemo(() => {
    const params = new URLSearchParams(window.location.search);
    return params.get("mode") === "overlay";
  }, []);

  if (overlayMode) {
    return <MorsePage overlayMode />;
  }

  return (
    <SidebarProvider defaultOpen style={appShellStyle}>
      <Sidebar collapsible="none" variant="inset">
        <SidebarHeader className="p-2 pb-1">
          <div className="flex items-center gap-2 rounded-lg border border-sidebar-border/70 bg-background px-2.5 py-2">
            <div className="flex size-8 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
              <RiRadarLine />
            </div>
            <div className="min-w-0">
              <p className="text-sm font-semibold text-foreground">三角洲行动工具</p>
              <p className="mt-1 truncate text-xs text-muted-foreground">桌面识别工作台</p>
            </div>
          </div>
        </SidebarHeader>

        <SidebarSeparator className="mt-1" />

        <SidebarContent className="px-1 pb-2">
          <SidebarGroup className="px-1 py-2">
            <SidebarGroupLabel>当前工具</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu className="gap-1">
                <SidebarMenuItem>
                  <SidebarMenuButton className="h-9 rounded-lg px-2.5" isActive tooltip="摩斯密码解析" type="button">
                    <RiRadarLine />
                    <span className="truncate">摩斯密码解析</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
      </Sidebar>

      <SidebarInset className="desktop-shell min-h-svh overflow-hidden">
        <header className="sticky top-0 z-10 border-b border-border/70 bg-background/95 px-4 py-3 backdrop-blur">
          <div className="flex min-w-0 items-center gap-3">
            <div className="flex size-9 shrink-0 items-center justify-center rounded-lg border border-border/70 bg-muted/40 text-muted-foreground">
              <RiRadarLine />
            </div>
            <div className="min-w-0">
              <h2 className="font-heading text-sm font-semibold">摩斯密码解析</h2>
              <p className="truncate text-xs text-muted-foreground">区域框选、识别结果、设置与历史记录</p>
            </div>
          </div>
        </header>

        <div className="flex flex-1 flex-col p-3">
          <MorsePage />
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}

export default App;
