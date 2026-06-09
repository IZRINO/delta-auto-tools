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
    <div className={cn("grid min-h-0 flex-1 grid-cols-12 gap-3 font-sans", className)}>
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
        "relative col-span-12 overflow-hidden border-2 border-[var(--ink)] bg-[var(--paper)] text-[var(--ink)]",
        className,
      )}
    >
      <div className="relative grid gap-px bg-[var(--ink)] lg:grid-cols-[minmax(0,1fr)_auto]">
        <div className="min-w-0 bg-[var(--paper)] px-3 py-3 sm:px-4">
          <div className="flex flex-wrap items-center gap-2">
            <p className="font-mono text-[0.6rem] font-black tracking-[0.22em] text-[var(--alert-red)] uppercase">
              [ {eyebrow} ]
            </p>
            {badges}
          </div>
          <div className="mt-2 flex min-w-0 flex-wrap items-end gap-x-4 gap-y-2">
            <h1 className="max-w-4xl text-balance font-heading text-[clamp(1.75rem,3.5vw,3.4rem)] font-black leading-[0.9] tracking-[-0.055em] text-[var(--ink)] uppercase">
              {title}
            </h1>
            <p className="max-w-[64ch] border-l-4 border-[var(--alert-red)] pl-3 font-mono text-[0.68rem] font-bold leading-relaxed tracking-[0.06em] text-[var(--steel)] uppercase">
              {description}
            </p>
          </div>
        </div>
        {(actions || stats) ? (
          <aside className="grid min-w-0 bg-[var(--paper)] lg:min-w-80 lg:max-w-[28rem]">
            {actions ? <div className="flex flex-wrap items-center justify-end gap-2 border-b-2 border-[var(--ink)] px-3 py-2">{actions}</div> : null}
            {stats ? <div className="grid gap-px bg-[var(--ink)] sm:grid-cols-3 lg:grid-cols-1">{stats}</div> : null}
          </aside>
        ) : null}
      </div>
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
    <div className={cn("flex min-h-12 items-center justify-between gap-3 bg-[var(--paper)] px-3 py-2 text-[var(--ink)]", className)}>
      <div className="min-w-0">
        <p className="truncate font-mono text-[0.56rem] font-black tracking-[0.18em] text-[var(--steel)] uppercase">{label}</p>
        {detail ? <div className="mt-0.5 truncate font-mono text-[0.58rem] font-bold tracking-[0.06em] text-[var(--steel)] uppercase">{detail}</div> : null}
      </div>
      <div className="flex shrink-0 items-center gap-2 font-mono text-lg font-black leading-none tracking-[-0.04em] text-[var(--ink)] tabular-nums">
        {icon ? <div className="text-[var(--alert-red)]">{icon}</div> : null}
        {value}
      </div>
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
        "border-2 border-[var(--ink)] bg-[var(--paper)] text-[var(--ink)] shadow-none transition-[background-color,outline-color]",
        active && "bg-[var(--bone)] outline-4 outline-[var(--alert-red)]",
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
    <CardHeader className={cn("border-b-2 border-[var(--ink)] bg-[var(--ink)] text-[var(--paper)]", className)}>
      <div className="flex min-w-0 items-start justify-between gap-4">
        <div className="flex min-w-0 items-start gap-3">
          {icon ? (
            <div className="flex size-9 shrink-0 items-center justify-center border border-[var(--paper)] bg-[var(--alert-red)] text-[var(--paper)]">
              {icon}
            </div>
          ) : null}
          <div className="min-w-0">
            {eyebrow ? <p className="font-mono text-[0.6rem] font-black tracking-[0.24em] text-[var(--bone)] uppercase">[ {eyebrow} ]</p> : null}
            <CardTitle className="mt-0.5 text-[var(--paper)] uppercase">{title}</CardTitle>
            {description ? <CardDescription className="mt-1 font-mono text-[0.68rem] font-bold tracking-[0.08em] text-[var(--bone)] uppercase">{description}</CardDescription> : null}
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
  return <div className={cn("border-2 border-[var(--ink)] bg-[var(--bone)] p-3", className)}>{children}</div>;
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
        <Empty className="min-h-40 border-2 border-dashed border-[var(--ink)] bg-[var(--bone)] px-4 py-8 text-center">
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
        "group flex min-h-32 flex-col items-center justify-center border-2 border-dashed border-[var(--ink)] bg-[var(--bone)] p-4 text-center transition-colors hover:bg-[var(--paper)] focus:outline-none focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--alert-red)] active:bg-[var(--ink)] active:text-[var(--paper)] disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      <span className="mb-4 flex size-11 items-center justify-center border-2 border-[var(--ink)] bg-[var(--alert-red)] text-[var(--paper)]">
        <RiAddLine />
      </span>
      <span className="text-sm font-black uppercase text-[var(--ink)] group-active:text-[var(--paper)]">{title}</span>
      <span className="mt-1 max-w-56 font-mono text-[0.68rem] font-bold leading-relaxed tracking-[0.08em] text-[var(--steel)] uppercase group-active:text-[var(--bone)]">{description}</span>
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
        "overflow-auto border-2 border-[var(--ink)] bg-[var(--data-well)] p-3 font-mono text-xs font-bold leading-relaxed text-[var(--paper)]",
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
    <Alert variant="destructive" className={cn("border-2 border-[var(--alert-red)] bg-[var(--paper)] text-[var(--ink)]", className)}>
      {title ? <AlertTitle>{title}</AlertTitle> : null}
      <AlertDescription>{children}</AlertDescription>
    </Alert>
  );
}

export function InlineControl({ children, className }: ControlTileProps) {
  return <div className={cn("border border-[var(--ink)] bg-[var(--paper)] p-3", className)}>{children}</div>;
}

type CardToolbarProps = {
  children: ReactNode;
  className?: string;
};

export function CardToolbar({ children, className }: CardToolbarProps) {
  return <div className={cn("flex flex-wrap items-center gap-2 border-2 border-[var(--ink)] bg-[var(--bone)] p-2", className)}>{children}</div>;
}

type SurfaceToggleGroupProps = {
  children: ReactNode;
  className?: string;
};

export function SurfaceToggleGroup({ children, className }: SurfaceToggleGroupProps) {
  return <div className={cn("border-2 border-[var(--ink)] bg-[var(--ink)] p-px", className)}>{children}</div>;
}
