import { useCallback, useEffect, useState } from "react";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiExternalLinkLine,
  RiWindowLine,
} from "@remixicon/react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  AppPage,
  CardBody,
  PageHero,
  SectionHeader,
  SignalTile,
  TacticalCard,
} from "@/components/app/app-ui";
import {
  BUILTIN_STRATEGY_SITES,
  createStrategySite,
  mergeStrategySites,
  readStoredUserSites,
  writeStoredUserSites,
  type StrategyOpenWindowRequest,
  type StrategyOpenWindowResponse,
  type StrategySite,
  type UserStrategySite,
} from "@/components/app/strategy-utils";
import { getErrorMessage } from "@/lib/error-utils";
import { useNativeShell } from "@/hooks/use-native-shell";

export function StrategyPage() {
  const isNativeShell = useNativeShell();
  const [userSites, setUserSites] = useState<StrategySite[]>(() => readStoredUserSites());
  const allSites = (() => {
    const merged = mergeStrategySites(BUILTIN_STRATEGY_SITES, userSites);
    return merged;
  })();
  const [activeId, setActiveId] = useState<string>(() => BUILTIN_STRATEGY_SITES[0]?.id ?? allSites[0]?.id ?? "");

  // 当用户删除了 activeId 对应的卡片，自动回退到第一个可用 Tab。
  useEffect(() => {
    if (allSites.length === 0) {
      return;
    }
    if (!allSites.some((site) => site.id === activeId)) {
      setActiveId(allSites[0].id);
    }
  }, [allSites, activeId]);

  const handleAddSite = useCallback((draft: UserStrategySite) => {
    const created = createStrategySite(draft);
    if (!created) {
      toast.error("网址无效：检查简称、标签与 URL 格式（必须以 http:// 或 https:// 开头）");
      return false;
    }
    setUserSites((current) => {
      const next = [...current, created];
      writeStoredUserSites(next);
      return next;
    });
    setActiveId(created.id);
    toast.success(`已新增攻略网站：${created.label}`);
    return true;
  }, []);

  const handleDeleteSite = useCallback((id: string) => {
    setUserSites((current) => {
      const target = current.find((site) => site.id === id);
      if (!target) {
        return current;
      }
      const next = current.filter((site) => site.id !== id);
      writeStoredUserSites(next);
      toast.success(`已删除攻略网站：${target.label}`);
      return next;
    });
  }, []);

  return (
    <AppPage>
      <PageHero
        eyebrow="Big-Category Utility"
        title="攻略网站工作台"
        description="在 Tauri 桌面窗口中打开攻略页面，由 WebView2 (Chromium) 真实渲染；支持自定义新增/删除攻略网站。"
        badges={
          <>
            <Badge variant="secondary">大类工具</Badge>
            <Badge variant="outline">WebView2 窗口</Badge>
          </>
        }
        stats={
          <>
            <SignalTile
              label="已集成站点"
              value={allSites.length}
              icon={<RiWindowLine />}
              detail={`${BUILTIN_STRATEGY_SITES.length} 个内置 + ${userSites.length} 个自定义`}
            />
            <SignalTile
              label="打开方式"
              value="应用内窗口"
              icon={<RiWindowLine />}
              detail="默认在 Tauri 桌面窗口打开；可改用系统浏览器。"
            />
            <SignalTile
              label="运行模式"
              value={isNativeShell ? "桌面应用" : "浏览器预览"}
              icon={<RiAddLine />}
              detail={isNativeShell ? "桌面端调 Tauri WebviewWindow。" : "浏览器预览模式无法打开窗口。"}
            />
          </>
        }
      />

      <TacticalCard>
        <Tabs value={activeId} onValueChange={setActiveId} className="min-h-0">
          <CardBody className="flex flex-col gap-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <TabsList variant="line" className="self-start">
                {allSites.map((site) => (
                  <TabsTrigger key={site.id} value={site.id}>
                    <img alt="" aria-hidden className="size-4 rounded-sm" src={site.favicon} />
                    <span>{site.label}</span>
                  </TabsTrigger>
                ))}
              </TabsList>
              <NewSiteDialog onSubmit={handleAddSite} />
            </div>

            {allSites.length === 0 ? (
              <div className="flex min-h-72 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_36%,transparent))] px-6 py-10 text-center text-sm text-muted-foreground">
                <p className="font-medium text-foreground">还没有任何攻略网站</p>
                <p className="max-w-md text-xs/relaxed">点上方"新增攻略网站"按钮，填入 URL 后即可加入工作台。</p>
              </div>
            ) : (
              allSites.map((site) => (
                <TabsContent key={site.id} value={site.id} className="flex flex-col gap-4">
                  <StrategySitePanel
                    site={site}
                    isNativeShell={isNativeShell}
                    onDelete={site.builtin ? null : () => handleDeleteSite(site.id)}
                  />
                </TabsContent>
              ))
            )}
          </CardBody>
        </Tabs>
      </TacticalCard>
    </AppPage>
  );
}

type NewSiteDialogProps = {
  onSubmit: (draft: UserStrategySite) => boolean;
};

function NewSiteDialog({ onSubmit }: NewSiteDialogProps) {
  const [open, setOpen] = useState(false);
  const [shortLabel, setShortLabel] = useState("");
  const [label, setLabel] = useState("");
  const [url, setUrl] = useState("");
  const [description, setDescription] = useState("");

  const reset = () => {
    setShortLabel("");
    setLabel("");
    setUrl("");
    setDescription("");
  };

  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const accepted = onSubmit({ shortLabel, label, url, description });
    if (accepted) {
      reset();
      setOpen(false);
    }
  };

  return (
    <Dialog onOpenChange={setOpen} open={open}>
      <DialogTrigger asChild>
        <Button type="button" variant="outline">
          <RiAddLine data-icon="inline-start" />
          新增攻略网站
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>新增攻略网站</DialogTitle>
          <DialogDescription>
            填入简称、标签、URL 与简介。URL 必须以 http:// 或 https:// 开头。
          </DialogDescription>
        </DialogHeader>
        <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
          <FieldGroup className="grid gap-3 md:grid-cols-2">
            <Field>
              <FieldLabel htmlFor="new-site-short">简称</FieldLabel>
              <FieldContent>
                <Input
                  id="new-site-short"
                  maxLength={6}
                  onChange={(event) => setShortLabel(event.currentTarget.value)}
                  placeholder="KK"
                  required
                  value={shortLabel}
                />
              </FieldContent>
              <FieldDescription>2-6 个字符的简短标签。</FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="new-site-label">完整标签</FieldLabel>
              <FieldContent>
                <Input
                  id="new-site-label"
                  onChange={(event) => setLabel(event.currentTarget.value)}
                  placeholder="KK 日报攻略总览"
                  required
                  value={label}
                />
              </FieldContent>
            </Field>
            <Field className="md:col-span-2">
              <FieldLabel htmlFor="new-site-url">URL</FieldLabel>
              <FieldContent>
                <Input
                  id="new-site-url"
                  inputMode="url"
                  onChange={(event) => setUrl(event.currentTarget.value)}
                  placeholder="https://example.com/path"
                  required
                  type="url"
                  value={url}
                />
              </FieldContent>
              <FieldDescription>必须以 http:// 或 https:// 开头。</FieldDescription>
            </Field>
            <Field className="md:col-span-2">
              <FieldLabel htmlFor="new-site-description">简介</FieldLabel>
              <FieldContent>
                <Input
                  id="new-site-description"
                  onChange={(event) => setDescription(event.currentTarget.value)}
                  placeholder="可选，简要描述站点内容"
                  value={description}
                />
              </FieldContent>
            </Field>
          </FieldGroup>
          <DialogFooter className="gap-2">
            <DialogClose asChild>
              <Button type="button" variant="ghost">
                取消
              </Button>
            </DialogClose>
            <Button type="submit">
              <RiAddLine data-icon="inline-start" />
              添加到工作台
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

type StrategySitePanelProps = {
  site: StrategySite;
  isNativeShell: boolean;
  onDelete: (() => void) | null;
};

function StrategySitePanel({ site, isNativeShell, onDelete }: StrategySitePanelProps) {
  const handleOpenInView = useCallback(async () => {
    if (!isNativeShell) {
      toast.error("浏览器预览模式下无法打开应用内窗口，请在桌面端打开。");
      return;
    }
    try {
      await invoke<StrategyOpenWindowResponse>("strategy_open_window", {
        request: {
          url: site.url,
          title: site.label,
          label: undefined,
        } satisfies StrategyOpenWindowRequest,
      });
      toast.success(`已在 Tauri 窗口中打开：${site.label}`);
    } catch (error) {
      const message = getErrorMessage(error);
      toast.error(`打开应用内窗口失败：${message}`);
    }
  }, [isNativeShell, site.url, site.label]);

  const handleOpenExternal = useCallback(async () => {
    if (isNativeShell) {
      try {
        await openUrl(site.url);
        return;
      } catch {
        // 落到 window.open 兜底
      }
    }
    window.open(site.url, "_blank", "noopener,noreferrer");
  }, [isNativeShell, site.url]);

  const handleDelete = useCallback(() => {
    if (!onDelete) {
      return;
    }
    onDelete();
  }, [onDelete]);

  return (
    <TacticalCard size="sm" className="flex flex-col gap-4">
      <SectionHeader
        eyebrow={`Station · ${site.shortLabel}`}
        icon={<img alt="" aria-hidden className="size-5 rounded-sm" src={site.favicon} />}
        title={site.label}
        description={site.description}
        badge={<Badge variant="secondary">Tauri 窗口</Badge>}
        actions={
          onDelete ? (
            <Button type="button" variant="ghost" size="sm" onClick={handleDelete}>
              <RiDeleteBinLine data-icon="inline-start" />
              删除此网站
            </Button>
          ) : null
        }
      />

      <CardBody className="flex flex-col gap-4">
        <FieldGroup className="grid gap-3 md:grid-cols-[1fr_auto] md:items-end">
          <Field>
            <FieldLabel>目标 URL</FieldLabel>
            <FieldContent>
              <div className="flex h-9 items-center rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_38%,transparent))] px-3 font-mono text-xs text-muted-foreground">
                {site.url}
              </div>
              <FieldDescription>
                在 Tauri 主进程下新建 WebviewWindow 加载该 URL，尺寸默认 1024×720、可调整，同一 host 复用窗口。
              </FieldDescription>
            </FieldContent>
          </Field>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="default"
              onClick={handleOpenInView}
              disabled={!isNativeShell}
              title="在 Tauri 桌面窗口（WebView2 Chromium）中打开目标站点"
            >
              <RiWindowLine data-icon="inline-start" />
              在窗口中打开
            </Button>
            <Button type="button" variant="secondary" onClick={handleOpenExternal}>
              <RiExternalLinkLine data-icon="inline-start" />
              系统浏览器打开
            </Button>
          </div>
        </FieldGroup>

        {!isNativeShell ? (
          <div className="flex min-h-40 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_36%,transparent))] px-6 py-8 text-center text-sm text-muted-foreground">
            <p className="font-medium text-foreground">该工具需要在桌面端使用</p>
            <p className="max-w-md text-xs/relaxed">
              浏览器预览模式下无法调起 Tauri WebviewWindow。请在桌面端打开 "三角洲行动工具" 后再使用。
            </p>
          </div>
        ) : (
          <div className="flex min-h-40 flex-col items-center justify-center gap-2 rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_36%,transparent))] px-6 py-8 text-center text-sm text-muted-foreground">
            <p className="font-medium text-foreground">点击 "在窗口中打开" 启动 Tauri 窗口</p>
            <p className="max-w-md text-xs/relaxed">
              每个站点的目标 URL 会作为 top-level navigation 加载到独立的 Tauri 子窗口（带标题栏、可拖动、可关闭），由真正的 Chromium 直接渲染站点本身。
            </p>
          </div>
        )}
      </CardBody>
    </TacticalCard>
  );
}
