import type { ComponentProps, ReactNode } from "react";
import { RiAddLine } from "@remixicon/react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { cn } from "@/lib/utils";

type AppPageProps = {
  children: ReactNode;
  className?: string;
};

export function AppPage({ children, className }: AppPageProps) {
  return (
    <div className={cn("flex min-h-0 flex-1 flex-col gap-4 font-sans", className)}>
      {children}
    </div>
  );
}

type PageHeroProps = {
  eyebrow: string;
  title: string;
  description: string;
  badges?: ReactNode;
  actions?: ReactNode;
  stats?: ReactNode;
  className?: string;
};

export function PageHero({ actions, badges, className, description, eyebrow, stats, title }: PageHeroProps) {
  return (
    <section
      className={cn(
        "relative overflow-hidden rounded-md border border-[var(--surface-border-strong)] bg-[linear-gradient(135deg,var(--surface-card-strong),color-mix(in_srgb,var(--secondary)_38%,transparent))] px-5 py-5 shadow-[var(--shadow-card)]",
        "before:pointer-events-none before:absolute before:inset-0 before:bg-[linear-gradient(color-mix(in_srgb,var(--surface-dot)_24%,transparent)_1px,transparent_1px),linear-gradient(90deg,color-mix(in_srgb,var(--surface-dot)_24%,transparent)_1px,transparent_1px)] before:bg-[length:20px_20px] before:opacity-55",
        "after:pointer-events-none after:absolute after:inset-y-0 after:left-0 after:w-1 after:bg-primary",
        className,
      )}
    >
      <div className="relative flex flex-col gap-5 xl:flex-row xl:items-end xl:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <p className="font-mono text-[0.64rem] font-semibold tracking-[0.26em] text-muted-foreground uppercase">
              {eyebrow}
            </p>
            {badges}
          </div>
          <h1 className="mt-3 text-balance font-heading text-2xl font-semibold tracking-[-0.035em] text-foreground xl:text-3xl">
            {title}
          </h1>
          <p className="mt-2 max-w-[65ch] text-sm/relaxed text-muted-foreground">{description}</p>
        </div>
        {actions ? <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div> : null}
      </div>
      {stats ? <div className="relative mt-5 grid gap-3 md:grid-cols-3">{stats}</div> : null}
    </section>
  );
}

type SignalTileProps = {
  label: string;
  value: ReactNode;
  detail?: ReactNode;
  icon?: ReactNode;
  className?: string;
};

export function SignalTile({ className, detail, icon, label, value }: SignalTileProps) {
  return (
    <div
      className={cn(
        "relative rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_srgb,var(--card)_68%,transparent))] px-4 py-3 shadow-[var(--shadow-tile)] before:pointer-events-none before:absolute before:inset-y-2 before:left-0 before:w-px before:bg-primary/60",
        className,
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="font-mono text-[0.62rem] font-semibold tracking-[0.2em] text-muted-foreground uppercase">
            {label}
          </p>
          <div className="mt-1 truncate font-mono text-lg font-semibold tracking-[-0.02em] text-foreground tabular-nums">{value}</div>
        </div>
        {icon ? <div className="text-primary">{icon}</div> : null}
      </div>
      {detail ? <div className="mt-2 text-xs/relaxed text-muted-foreground">{detail}</div> : null}
    </div>
  );
}

type TacticalCardProps = ComponentProps<typeof Card> & {
  active?: boolean;
};

export function TacticalCard({ active, children, className, size = "sm", ...props }: TacticalCardProps) {
  return (
    <Card
      size={size}
      className={cn(
        "border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),color-mix(in_srgb,var(--secondary)_32%,transparent))] shadow-[var(--shadow-card)] transition-[border-color,background-color,box-shadow,opacity,transform]",
        "hover:border-[var(--surface-border-strong)] hover:shadow-[var(--shadow-subtle)]",
        active && "border-primary/70 bg-[linear-gradient(145deg,color-mix(in_srgb,var(--primary)_10%,var(--surface-card-strong)),color-mix(in_srgb,var(--surface-card)_82%,transparent))] ring-2 ring-primary/28",
        className,
      )}
      {...props}
    >
      {children}
    </Card>
  );
}

type SectionHeaderProps = {
  title: string;
  description?: string;
  eyebrow?: string;
  icon?: ReactNode;
  badge?: ReactNode;
  actions?: ReactNode;
  className?: string;
};

export function SectionHeader({ actions, badge, className, description, eyebrow, icon, title }: SectionHeaderProps) {
  return (
    <CardHeader
      className={cn(
        "border-b border-[var(--surface-border)] bg-[linear-gradient(180deg,var(--surface-muted),transparent)]",
        className,
      )}
    >
      <div className="flex min-w-0 items-start justify-between gap-4">
        <div className="flex min-w-0 items-start gap-3">
          {icon ? (
            <div className="flex size-9 shrink-0 items-center justify-center rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_srgb,var(--card)_58%,transparent))] text-primary shadow-[var(--shadow-tile)]">
              {icon}
            </div>
          ) : null}
          <div className="min-w-0">
            {eyebrow ? (
              <p className="font-mono text-[0.6rem] font-semibold tracking-[0.22em] text-muted-foreground uppercase">
                {eyebrow}
              </p>
            ) : null}
            <CardTitle className="mt-0.5">{title}</CardTitle>
            {description ? <CardDescription className="mt-1">{description}</CardDescription> : null}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {actions}
          {badge ? <div className="shrink-0">{badge}</div> : null}
        </div>
      </div>
    </CardHeader>
  );
}

type ControlTileProps = {
  children: ReactNode;
  className?: string;
};

export function ControlTile({ children, className }: ControlTileProps) {
  return (
    <div
      className={cn(
        "rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_srgb,var(--card)_54%,transparent))] p-3 shadow-[var(--shadow-tile)]",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function SaveStateBadge({ dirty, saving }: { dirty: boolean; saving: boolean }) {
  if (saving) return <Badge variant="outline">保存中</Badge>;
  if (dirty) return <Badge variant="outline">待保存</Badge>;
  return <Badge variant="secondary">已保存</Badge>;
}

export function CardBody({ children, className }: { children: ReactNode; className?: string }) {
  return <CardContent className={cn("pt-4", className)}>{children}</CardContent>;
}

type TacticalEmptyStateProps = {
  icon?: ReactNode;
  title: string;
  description: ReactNode;
  className?: string;
  children?: ReactNode;
};

export function TacticalEmptyState({ children, className, description, icon, title }: TacticalEmptyStateProps) {
  return (
    <TacticalCard className={cn("min-h-48", className)}>
      <CardBody className="flex h-full items-center justify-center">
        <Empty className="min-h-40 rounded-md border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),var(--surface-tile))] px-4 py-8 text-center shadow-[var(--shadow-tile)]">
          {icon ? <EmptyMedia variant="icon">{icon}</EmptyMedia> : null}
          <EmptyHeader>
            <EmptyTitle>{title}</EmptyTitle>
            <EmptyDescription>{description}</EmptyDescription>
          </EmptyHeader>
          {children}
        </Empty>
      </CardBody>
    </TacticalCard>
  );
}

type AddCardButtonProps = {
  disabled?: boolean;
  title: string;
  description: ReactNode;
  className?: string;
  onClick: () => void;
};

export function AddCardButton({ className, description, disabled, onClick, title }: AddCardButtonProps) {
  return (
    <button
      className={cn(
        "group flex min-h-64 flex-col items-center justify-center rounded-md border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_srgb,var(--card)_48%,transparent))] p-6 text-center transition-[border-color,background-color,transform,opacity] hover:border-primary/55 hover:bg-[var(--surface-hover)] active:translate-y-px disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      <span className="mb-4 flex size-11 items-center justify-center rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),var(--surface-tile))] text-primary transition-colors group-hover:border-primary/55 group-hover:bg-primary/10">
        <RiAddLine />
      </span>
      <span className="text-sm font-semibold text-foreground">{title}</span>
      <span className="mt-1 max-w-56 text-xs/relaxed text-muted-foreground">{description}</span>
    </button>
  );
}

type JsonPreBlockProps = {
  data: unknown;
  className?: string;
  maxHeightClassName?: string;
};

export function JsonPreBlock({ className, data, maxHeightClassName = "max-h-64" }: JsonPreBlockProps) {
  return (
    <pre
      className={cn(
        maxHeightClassName,
        "overflow-auto rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_srgb,var(--card)_52%,transparent))] p-3 font-mono text-xs text-muted-foreground shadow-[var(--shadow-tile)]",
        className,
      )}
    >
      {JSON.stringify(data, null, 2)}
    </pre>
  );
}

type InlineNoticeProps = {
  title?: string;
  children: ReactNode;
  className?: string;
};

export function InlineNotice({ children, className, title }: InlineNoticeProps) {
  return (
    <Alert variant="destructive" className={cn("rounded-md border-destructive/55 bg-[color-mix(in_srgb,var(--destructive)_10%,transparent)]", className)}>
      {title ? <AlertTitle>{title}</AlertTitle> : null}
      <AlertDescription>{children}</AlertDescription>
    </Alert>
  );
}

export function InlineControl({ children, className }: ControlTileProps) {
  return (
    <div
      className={cn(
        "rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,color-mix(in_srgb,var(--card)_64%,transparent),var(--surface-tile))] p-3 shadow-[var(--shadow-tile)]",
        className,
      )}
    >
      {children}
    </div>
  );
}

type CardToolbarProps = {
  children: ReactNode;
  className?: string;
};

export function CardToolbar({ children, className }: CardToolbarProps) {
  return (
    <div
      className={cn(
        "flex flex-wrap items-center gap-2 rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-panel),color-mix(in_srgb,var(--card)_48%,transparent))] p-2 shadow-[var(--shadow-tile)]",
        className,
      )}
    >
      {children}
    </div>
  );
}

type SurfaceToggleGroupProps = {
  children: ReactNode;
  className?: string;
};

export function SurfaceToggleGroup({ children, className }: SurfaceToggleGroupProps) {
  return (
    <div
      className={cn(
        "rounded-md border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-panel),color-mix(in_srgb,var(--card)_42%,transparent))] p-1 shadow-[var(--shadow-tile)]",
        className,
      )}
    >
      {children}
    </div>
  );
}
