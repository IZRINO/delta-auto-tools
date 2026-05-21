import { useMemo, useState, type CSSProperties } from "react";
import {
  RiCrosshair2Line,
  RiGamepadLine,
  RiPulseLine,
  RiRadarLine,
  RiTimerLine,
} from "@remixicon/react";

import { MorsePage } from "@/components/app/morse-page";
import { TimerPage } from "@/components/app/timer-page";
import { RapidfirePage } from "@/components/app/rapidfire-page";
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
  "--sidebar-width": "16rem",
  "--sidebar-width-icon": "3rem",
} as CSSProperties;

const tools = [
  {
    id: "morse" as const,
    icon: RiRadarLine,
    label: "摩斯密码解析",
    short: "Morse",
    eyebrow: "Workspace / Morse",
    title: "摩斯密码解析工作台",
    description: "区域框选、图像识别、热键触发与自动输入的三步工作流。",
  },
  {
    id: "timer" as const,
    icon: RiTimerLine,
    label: "计时\\计数器",
    short: "Timer",
    eyebrow: "Workspace / Timer & Counter",
    title: "计时\\计数器工作台",
    description: "多计时器、多计数器、独立透明窗口与全局快捷键管理。",
  },
  {
    id: "rapidfire" as const,
    icon: RiGamepadLine,
    label: "连发器",
    short: "Rapidfire",
    eyebrow: "Workspace / Rapidfire",
    title: "连发器工作台",
    description: "按住触发、定间隔连发、松开补齐偶数次数的战术输入面板。",
  },
];

type ToolId = (typeof tools)[number]["id"];

function App() {
  const overlayMode = useMemo(() => {
    const params = new URLSearchParams(window.location.search);
    return params.get("mode");
  }, []);
  const [activeTool, setActiveTool] = useState<ToolId>("morse");

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

  if (overlayMode === "rapidfire-display") {
    return <RapidfirePage overlayMode="display" />;
  }

  if (overlayMode === "rapidfire-position") {
    return <RapidfirePage overlayMode="position" />;
  }

  const activeToolMeta = tools.find((tool) => tool.id === activeTool) ?? tools[0];
  const ActiveIcon = activeToolMeta.icon;

  return (
    <SidebarProvider className="h-svh min-h-0 overflow-hidden bg-transparent" defaultOpen style={appShellStyle}>
       <Sidebar className="min-h-0 overflow-hidden border-r border-sidebar-border/70 bg-sidebar/88 backdrop-blur-xl" collapsible="none" variant="sidebar">
        <SidebarHeader className="p-4 pb-3">
          <div className="relative overflow-hidden rounded-xl border border-sidebar-border/80 bg-sidebar px-4 py-4 shadow-sm">
            <div className="absolute inset-x-0 top-0 h-1 bg-primary" />
            <div className="flex items-center gap-3">
            <div className="flex size-11 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-sm">
              <RiCrosshair2Line />
            </div>
            <div className="min-w-0">
              <p className="text-sm font-semibold tracking-tight text-foreground">三角洲行动工具</p>
              <p className="mt-1 truncate font-mono text-[0.62rem] tracking-[0.2em] text-muted-foreground uppercase">Delta Ops Console</p>
            </div>
            </div>
            <div className="mt-4 grid grid-cols-3 gap-2 text-center">
              <div className="rounded-lg border border-sidebar-border bg-background/55 px-2 py-2">
                <p className="font-mono text-sm font-semibold text-foreground">03</p>
                <p className="mt-0.5 text-[0.62rem] text-muted-foreground">工具</p>
              </div>
              <div className="rounded-lg border border-sidebar-border bg-background/55 px-2 py-2">
                <p className="font-mono text-sm font-semibold text-foreground">07</p>
                <p className="mt-0.5 text-[0.62rem] text-muted-foreground">窗口</p>
              </div>
              <div className="rounded-lg border border-sidebar-border bg-background/55 px-2 py-2">
                <p className="font-mono text-sm font-semibold text-foreground">2.0</p>
                <p className="mt-0.5 text-[0.62rem] text-muted-foreground">Tauri</p>
              </div>
            </div>
          </div>
        </SidebarHeader>

        <SidebarSeparator className="mx-4 mt-1" />

        <SidebarContent className="min-h-0 overflow-y-auto overflow-x-hidden px-3 pb-3 [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
          <SidebarGroup className="px-0 py-2">
            <SidebarGroupLabel className="font-mono tracking-[0.18em] uppercase">当前工具</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu className="gap-2">
                {tools.map((tool, index) => {
                  const Icon = tool.icon;
                  const active = activeTool === tool.id;
                  return (
                    <SidebarMenuItem key={tool.id}>
                      <SidebarMenuButton
                        className="h-auto rounded-lg border border-transparent px-3 py-3 data-[active=true]:border-sidebar-border data-[active=true]:bg-background/70 data-[active=true]:shadow-sm"
                        isActive={active}
                        onClick={() => setActiveTool(tool.id)}
                        tooltip={tool.label}
                        type="button"
                      >
                        <Icon />
                        <span className="flex min-w-0 flex-1 flex-col items-start">
                          <span className="truncate text-sm">{tool.label}</span>
                          <span className="font-mono text-[0.62rem] tracking-[0.16em] text-muted-foreground uppercase">
                            0{index + 1} / {tool.short}
                          </span>
                        </span>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  );
                })}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
          <SidebarGroup className="mt-auto px-0">
            <div className="rounded-xl border border-sidebar-border/80 bg-background/50 px-4 py-4">
              <div className="flex items-center gap-2 text-xs font-medium text-foreground">
                <RiPulseLine />
                <span>本地原生能力</span>
              </div>
              <p className="mt-2 text-xs/relaxed text-muted-foreground">
                热键、透明窗口和输入模拟均由 Tauri 原生命令驱动。
              </p>
            </div>
          </SidebarGroup>
        </SidebarContent>
      </Sidebar>

      <SidebarInset className="h-full min-h-0 overflow-hidden bg-transparent">
        <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
          <header className="shrink-0 px-5 pt-5 xl:px-6">
            <div className="relative overflow-hidden rounded-xl border border-border/70 bg-card/88 px-5 py-4 shadow-sm backdrop-blur-xl">
              <div className="absolute inset-y-0 left-0 w-1 bg-primary" />
              <div className="relative flex min-w-0 flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
              <div className="flex min-w-0 items-center gap-3">
                <div className="flex size-11 shrink-0 items-center justify-center rounded-lg border border-border bg-background text-primary shadow-sm">
                  <ActiveIcon />
                </div>
                <div className="min-w-0">
                  <p className="font-mono text-[0.68rem] font-semibold tracking-[0.22em] text-muted-foreground uppercase">{activeToolMeta.eyebrow}</p>
                  <h2 className="mt-1 truncate text-xl font-semibold tracking-[-0.035em]">{activeToolMeta.title}</h2>
                  <p className="mt-1 max-w-2xl text-xs text-muted-foreground">{activeToolMeta.description}</p>
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-2 font-mono text-[0.65rem] uppercase tracking-[0.16em] text-muted-foreground">
                <span className="rounded-full border border-border bg-background/70 px-3 py-1.5">Local First</span>
                <span className="rounded-full border border-border bg-background/70 px-3 py-1.5">No Router</span>
                <span className="rounded-full border border-border bg-background/70 px-3 py-1.5">Overlay Ready</span>
              </div>
              </div>
            </div>
          </header>

          <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 xl:px-6 xl:py-6 [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
            <div className="flex min-h-full flex-col">
              <div className="flex min-h-full flex-col rounded-xl border border-border/70 bg-card/78 p-4 shadow-sm backdrop-blur-xl xl:p-5">
                {activeTool === "morse" ? <MorsePage /> : activeTool === "timer" ? <TimerPage /> : <RapidfirePage />}
              </div>
            </div>
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}

export default App;

