import { useCallback, useEffect, useRef, useState } from "react";
import {
  RiAddLine,
  RiDeleteBinLine,
  RiExternalLinkLine,
  RiRefreshLine,
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
  type StrategyFetchResponse,
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
        description="通过 Rust 端 HTTP 抓取目标页面 HTML，在前端 iframe 中嵌入渲染；支持手动与定时刷新，CC check 命中时自动降级到 Tauri 窗口。"
        badges={
          <>
            <Badge variant="secondary">大类工具</Badge>
            <Badge variant="outline">iframe 渲染</Badge>
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
              label="渲染方式"
              value="iframe 嵌入"
              icon={<RiWindowLine />}
              detail="Rust 端抓取 HTML，前端 iframe 展示；CC check 降级 WebviewWindow。"
            />
            <SignalTile
              label="运行模式"
              value={isNativeShell ? "桌面应用" : "浏览器预览"}
              icon={<RiAddLine />}
              detail={isNativeShell ? "桌面端可抓取并嵌入页面。" : "浏览器预览模式无法调用 Tauri 命令。"}
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

function injectBaseHref(html: string, baseUrl: string): string {
  const base = `<base href="${baseUrl}">`;
  const headMatch = /<head(\s[^>]*)?>|<head>/i.exec(html);
  if (headMatch) {
    const insertPos = headMatch.index + headMatch[0].length;
    return html.slice(0, insertPos) + base + html.slice(insertPos);
  }
  return `${base}${html}`;
}
type StrategySitePanelProps = {
  site: StrategySite;
  isNativeShell: boolean;
  onDelete: (() => void) | null;
};

function StrategySitePanel({ site, isNativeShell, onDelete }: StrategySitePanelProps) {
  const [fetchedHtml, setFetchedHtml] = useState<string | null>(null);
  const [refreshInterval, setRefreshInterval] = useState<number>(0); // 0 = 关闭
  const [lastFetchTime, setLastFetchTime] = useState<Date | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchPage = useCallback(async () => {
    try {
      const response = await invoke<StrategyFetchResponse>("strategy_fetch_page", {
        url: site.url,
      });
      if (response.challenge) {
        // CC check 命中，降级到 WebviewWindow
        await invoke<StrategyOpenWindowResponse>("strategy_open_window", {
          request: {
            url: site.url,
            title: site.label,
            label: undefined,
          } satisfies StrategyOpenWindowRequest,
        });
        toast.warning(`${response.challenge.message} 已降级到 Tauri 窗口打开。`);
        return;
      }
      setFetchedHtml(response.html);
      setLastFetchTime(new Date());
    } catch (error) {
      const message = getErrorMessage(error);
      toast.error(`获取页面失败：${message}`);
    }
  }, [site.url, site.label]);

  // 首次加载 + 关闭面板时清理 interval
  useEffect(() => {
    fetchPage();
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [site.url]); // 仅在切换站点时重新加载

  // 定时刷新逻辑
  useEffect(() => {
    if (intervalRef.current) {
      clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
    if (refreshInterval > 0) {
      intervalRef.current = setInterval(fetchPage, refreshInterval * 1000);
    }
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [refreshInterval, fetchPage]);

  const handleManualRefresh = useCallback(() => {
    fetchPage();
  }, [fetchPage]);

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
        badge={<Badge variant="secondary">iframe 嵌入</Badge>}
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
                页面由 Rust 端通过 HTTP 抓取 HTML，在前端 iframe 中嵌入渲染。
              </FieldDescription>
            </FieldContent>
          </Field>
          <div className="flex flex-wrap items-center gap-2">
            <Button type="button" variant="default" onClick={handleManualRefresh}>
              <RiRefreshLine data-icon="inline-start" />
              手动刷新
            </Button>
            <Button type="button" variant="secondary" onClick={handleOpenExternal}>
              <RiExternalLinkLine data-icon="inline-start" />
              系统浏览器打开
            </Button>
          </div>
        </FieldGroup>

        {/* 定时刷新选择器 */}
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <span>定时刷新：</span>
          <select
            className="h-8 rounded-md border border-[var(--surface-border)] bg-[var(--surface-tile)] px-2 text-xs"
            value={refreshInterval}
            onChange={(e) => setRefreshInterval(Number(e.target.value))}
          >
            <option value={0}>关闭</option>
            <option value={30}>30 秒</option>
            <option value={60}>60 秒</option>
            <option value={120}>120 秒</option>
            <option value={300}>300 秒</option>
          </select>
          {lastFetchTime && (
            <span className="text-xs">
              上次更新：{lastFetchTime.toLocaleTimeString("zh-CN")}
            </span>
          )}
        </div>

        {/* iframe 渲染区域 */}
        {!isNativeShell ? (
          <div className="flex min-h-40 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_36%,transparent))] px-6 py-8 text-center text-sm text-muted-foreground">
            <p className="font-medium text-foreground">该工具需要在桌面端使用</p>
            <p className="max-w-md text-xs/relaxed">
              浏览器预览模式下无法调用 Tauri 命令。请在桌面端打开 "三角洲行动工具" 后再使用。
            </p>
          </div>
        ) : fetchedHtml ? (
          <iframe
            srcDoc={injectBaseHref(fetchedHtml, site.url)}
            className="w-full h-[600px] rounded-lg border border-[var(--surface-border)]"
            sandbox="allow-scripts allow-same-origin"
            title={site.label}
          />
        ) : (
          <div className="flex min-h-40 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_36%,transparent))] px-6 py-8 text-center text-sm text-muted-foreground">
            <p className="font-medium text-foreground">加载中...</p>
            <p className="max-w-md text-xs/relaxed">
              正在从 {site.url} 抓取页面内容，请稍候。
            </p>
          </div>
        )}
      </CardBody>
    </TacticalCard>
  );
}