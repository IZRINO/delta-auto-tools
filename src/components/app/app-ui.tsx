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
    <div className={cn("flex min-h-0 flex-1 flex-col gap-5", className)}>
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
        "relative overflow-hidden rounded-xl border border-[var(--surface-border-strong)] bg-[linear-gradient(135deg,var(--surface-card-strong),color-mix(in_oklch,var(--secondary)_32%,transparent))] px-5 py-5 shadow-[var(--shadow-card)] backdrop-blur-xl",
        "before:pointer-events-none before:absolute before:inset-0 before:bg-[radial-gradient(circle_at_0_0,color-mix(in_oklch,var(--primary)_18%,transparent),transparent_28rem),radial-gradient(circle,color-mix(in_oklch,var(--surface-dot)_42%,transparent)_0.7px,transparent_0.95px)] before:bg-[length:100%_100%,22px_22px] before:opacity-[0.18]",
        "after:pointer-events-none after:absolute after:inset-x-0 after:top-0 after:h-px after:bg-[linear-gradient(90deg,transparent,var(--surface-highlight),transparent)]",
        className,
      )}
    >
      <div className="relative flex flex-col gap-5 xl:flex-row xl:items-end xl:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <p className="font-mono text-[0.68rem] font-semibold tracking-[0.24em] text-muted-foreground uppercase">
              {eyebrow}
            </p>
            {badges}
          </div>
          <h1 className="mt-3 text-balance font-heading text-2xl font-semibold tracking-[-0.045em] text-foreground xl:text-3xl">
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
        "rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_62%,transparent))] px-4 py-3 shadow-[var(--shadow-tile)] backdrop-blur-sm",
        className,
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="font-mono text-[0.65rem] font-medium tracking-[0.18em] text-muted-foreground uppercase">
            {label}
          </p>
          <div className="mt-1 truncate text-lg font-semibold tracking-tight text-foreground">{value}</div>
        </div>
        {icon ? <div className="text-primary/80">{icon}</div> : null}
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
        "border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),color-mix(in_oklch,var(--secondary)_28%,transparent))] shadow-[var(--shadow-card)] backdrop-blur-md transition-[border-color,background-color,box-shadow,opacity]",
        "hover:border-primary/30 hover:shadow-[var(--shadow-shell)]",
        active && "border-primary/45 bg-[linear-gradient(145deg,color-mix(in_oklch,var(--primary)_9%,var(--surface-card-strong)),color-mix(in_oklch,var(--surface-card)_78%,transparent))] ring-1 ring-primary/18",
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
            <div className="flex size-9 shrink-0 items-center justify-center rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_46%,transparent))] text-primary backdrop-blur-sm">
              {icon}
            </div>
          ) : null}
          <div className="min-w-0">
            {eyebrow ? (
              <p className="font-mono text-[0.62rem] font-semibold tracking-[0.2em] text-muted-foreground uppercase">
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
        "rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_44%,transparent))] p-3 shadow-[var(--shadow-tile)] backdrop-blur-sm",
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
        <Empty className="min-h-40 rounded-xl border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),var(--surface-tile))] px-4 py-8 text-center backdrop-blur-md">
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
        "group flex min-h-64 flex-col items-center justify-center rounded-xl border border-dashed border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_38%,transparent))] p-6 text-center transition-all hover:border-primary/35 hover:bg-[var(--surface-hover)] disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      <span className="mb-4 flex size-11 items-center justify-center rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-card-strong),var(--surface-tile))] text-primary transition-colors group-hover:border-primary/35 group-hover:bg-primary/5">
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
        "overflow-auto rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,var(--surface-tile),color-mix(in_oklch,var(--card)_42%,transparent))] p-3 font-mono text-xs text-muted-foreground",
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
    <Alert variant="destructive" className={cn("border-destructive/45 bg-[color-mix(in_oklch,var(--destructive)_8%,transparent)]", className)}>
      {title ? <AlertTitle>{title}</AlertTitle> : null}
      <AlertDescription>{children}</AlertDescription>
    </Alert>
  );
}

export function InlineControl({ children, className }: ControlTileProps) {
  return (
    <div
      className={cn(
        "rounded-lg border border-[var(--surface-border)] bg-[linear-gradient(145deg,color-mix(in_oklch,var(--card)_54%,transparent),var(--surface-tile))] p-3 backdrop-blur-sm",
        className,
      )}
    >
      {children}
    </div>
  );
}
