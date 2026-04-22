import { useMemo, type CSSProperties } from "react";
import { RiRadarLine } from "@remixicon/react";

import { MorsePage } from "@/components/app/morse-page";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarSeparator,
} from "@/components/ui/sidebar";

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
    <SidebarProvider className="h-svh min-h-0 overflow-hidden" defaultOpen style={appShellStyle}>
      <Sidebar className="desktop-sidebar min-h-0 overflow-hidden" collapsible="none" variant="inset">
        <SidebarHeader className="p-3 pb-2">
          <div className="flex items-center gap-3 rounded-[calc(var(--radius-xl)+2px)] border border-sidebar-border bg-sidebar px-4 py-4 shadow-sm">
            <div className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary text-primary-foreground shadow-sm">
              <RiRadarLine />
            </div>
            <div className="min-w-0">
              <p className="text-sm font-semibold tracking-tight text-foreground">三角洲行动工具</p>
              <p className="mt-1 truncate text-[0.6875rem] tracking-[0.16em] text-muted-foreground uppercase">Morse Desktop Console</p>
            </div>
          </div>
        </SidebarHeader>

        <SidebarSeparator className="mt-1" />

        <SidebarContent className="desktop-sidebar-scroll desktop-scrollbar-hidden px-1 pb-2">
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

      <SidebarInset className="desktop-shell h-full min-h-0 overflow-hidden">
        <div className="desktop-workspace flex h-full min-h-0 flex-1 flex-col overflow-hidden">
          <header className="desktop-topbar shrink-0 px-5 py-3 xl:px-6 xl:py-4">
            <div className="flex min-w-0 items-center justify-between gap-4 rounded-[calc(var(--radius-2xl)+2px)] border border-border bg-card px-5 py-4 shadow-sm">
              <div className="flex min-w-0 items-center gap-3">
                <div className="flex size-10 shrink-0 items-center justify-center rounded-2xl border border-border bg-background text-primary shadow-sm">
                  <RiRadarLine />
                </div>
                <div className="min-w-0">
                  <p className="text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">Workspace / Morse</p>
                  <h2 className="mt-1 truncate font-heading text-[0.92rem] font-semibold tracking-[0.01em]">摩斯密码解析工作台</h2>
                </div>
              </div>

              <div className="desktop-topbar-meta hidden items-center gap-2 xl:flex">
                <span className="inline-flex items-center rounded-full border border-border bg-background px-3 py-1 text-xs font-medium text-muted-foreground">区域框选</span>
                <span className="inline-flex items-center rounded-full border border-border bg-background px-3 py-1 text-xs font-medium text-muted-foreground">识别结果</span>
                <span className="inline-flex items-center rounded-full border border-border bg-background px-3 py-1 text-xs font-medium text-muted-foreground">历史记录</span>
              </div>
            </div>
          </header>

          <div className="desktop-main-scroll desktop-scrollbar-hidden min-h-0 flex-1 overflow-y-auto px-5 pb-5 xl:px-6 xl:pb-6">
            <div className="flex min-h-full flex-col">
              <div className="flex min-h-full flex-col rounded-[calc(var(--radius-3xl)+2px)] border border-border bg-card px-4 py-4 shadow-sm xl:px-6 xl:py-6">
                <MorsePage />
              </div>
            </div>
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}

export default App;

