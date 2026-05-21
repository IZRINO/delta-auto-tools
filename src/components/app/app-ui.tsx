import type { ComponentProps, ReactNode } from "react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
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
        "relative overflow-hidden rounded-xl border border-border/70 bg-card/92 px-5 py-5 shadow-sm",
        "before:pointer-events-none before:absolute before:inset-0 before:bg-[linear-gradient(135deg,var(--primary)_0,transparent_26%),linear-gradient(to_right,var(--border)_1px,transparent_1px),linear-gradient(to_bottom,var(--border)_1px,transparent_1px)] before:bg-[length:100%_100%,28px_28px,28px_28px] before:opacity-[0.055]",
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
          <h1 className="mt-3 text-2xl font-semibold tracking-[-0.045em] text-foreground xl:text-3xl">
            {title}
          </h1>
          <p className="mt-2 max-w-3xl text-sm/relaxed text-muted-foreground">{description}</p>
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
    <div className={cn("rounded-lg border border-border/70 bg-background/76 px-4 py-3 shadow-sm", className)}>
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
        "border-border/70 bg-card/94 shadow-sm transition-colors",
        "hover:border-primary/30 hover:bg-card",
        active && "border-primary/35 bg-primary/5 ring-1 ring-primary/15",
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
  className?: string;
};

export function SectionHeader({ badge, className, description, eyebrow, icon, title }: SectionHeaderProps) {
  return (
    <CardHeader className={cn("border-b border-border/60 bg-muted/18", className)}>
      <div className="flex min-w-0 items-start justify-between gap-4">
        <div className="flex min-w-0 items-start gap-3">
          {icon ? (
            <div className="flex size-9 shrink-0 items-center justify-center rounded-lg border border-border bg-background text-primary shadow-sm">
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
        {badge ? <div className="shrink-0">{badge}</div> : null}
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
    <div className={cn("rounded-lg border border-border/70 bg-background/72 p-3 shadow-sm", className)}>
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
