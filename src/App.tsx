import { useMemo, useState, type CSSProperties } from "react";
import { RiRadarLine, RiTimerLine } from "@remixicon/react";

import { MorsePage } from "@/components/app/morse-page";
import { TimerPage } from "@/components/app/timer-page";
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
    return params.get("mode");
  }, []);
  const [activeTool, setActiveTool] = useState<"morse" | "timer">("morse");

  if (overlayMode === "overlay") {
    return <MorsePage overlayMode />;
  }

  if (overlayMode === "timer-display") {
    return <TimerPage overlayMode="display" />;
  }

  if (overlayMode === "counter-display") {
    return <TimerPage overlayMode="counter-display" />;
  }

  if (overlayMode === "timer-position") {
    return <TimerPage overlayMode="position" />;
  }

  if (overlayMode === "counter-position") {
    return <TimerPage overlayMode="counter-position" />;
  }

  const activeToolMeta = activeTool === "morse"
    ? {
        icon: <RiRadarLine />,
        eyebrow: "Workspace / Morse",
        title: "摩斯密码解析工作台",
      }
    : {
        icon: <RiTimerLine />,
        eyebrow: "Workspace / Timer & Counter",
        title: "计时\\计数器工作台",
      };

  return (
    <SidebarProvider className="h-svh min-h-0 overflow-hidden" defaultOpen style={appShellStyle}>
       <Sidebar className="min-h-0 overflow-hidden" collapsible="none" variant="inset">
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

        <SidebarContent className="min-h-0 overflow-y-auto overflow-x-hidden px-1 pb-2 [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
          <SidebarGroup className="px-1 py-2">
            <SidebarGroupLabel>当前工具</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu className="gap-1">
                <SidebarMenuItem>
                  <SidebarMenuButton className="h-9 rounded-lg px-2.5" isActive={activeTool === "morse"} onClick={() => setActiveTool("morse")} tooltip="摩斯密码解析" type="button">
                    <RiRadarLine />
                    <span className="truncate">摩斯密码解析</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
                <SidebarMenuItem>
                  <SidebarMenuButton className="h-9 rounded-lg px-2.5" isActive={activeTool === "timer"} onClick={() => setActiveTool("timer")} tooltip="计时\\计数器" type="button">
                    <RiTimerLine />
                    <span className="truncate">计时\\计数器</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
      </Sidebar>

      <SidebarInset className="h-full min-h-0 overflow-hidden">
        <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
          <header className="shrink-0 px-5 py-3 xl:px-6 xl:py-4">
            <div className="flex min-w-0 items-center gap-4 rounded-[calc(var(--radius-2xl)+2px)] border border-border bg-card px-5 py-4 shadow-sm">
              <div className="flex min-w-0 items-center gap-3">
                <div className="flex size-10 shrink-0 items-center justify-center rounded-2xl border border-border bg-background text-primary shadow-sm">
                  {activeToolMeta.icon}
                </div>
                <div className="min-w-0">
                  <p className="text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">{activeToolMeta.eyebrow}</p>
                  <h2 className="mt-1 truncate font-heading text-[0.92rem] font-semibold tracking-[0.01em]">{activeToolMeta.title}</h2>
                </div>
              </div>
            </div>
          </header>

          <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-5 xl:px-6 xl:pb-6 [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
            <div className="flex min-h-full flex-col">
              <div className="flex min-h-full flex-col rounded-[calc(var(--radius-3xl)+2px)] border border-border bg-card px-4 py-4 shadow-sm xl:px-6 xl:py-6">
                {activeTool === "morse" ? <MorsePage /> : <TimerPage />}
              </div>
            </div>
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}

export default App;

