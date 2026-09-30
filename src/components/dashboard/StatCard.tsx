import { LucideIcon } from "lucide-react";

interface StatCardProps {
  title: string;
  value: string;
  description: string;
  icon: LucideIcon;
  trend?: string;
}

export default function StatCard({
  title,
  value,
  description,
  icon: Icon,
  trend,
}: StatCardProps) {
  return (
    <div className="rounded-2xl border border-zinc-200 bg-white p-5">
      <div className="flex items-start justify-between">
        <div>
          <p className="text-sm text-zinc-500">{title}</p>

          <h3 className="mt-2 text-3xl font-semibold tracking-tight text-zinc-950">
            {value}
          </h3>
        </div>

        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-zinc-100">
          <Icon size={19} className="text-zinc-700" />
        </div>
      </div>

      <div className="mt-4 flex items-center gap-2 text-xs">
        {trend && (
          <span className="font-medium text-emerald-600">
            {trend}
          </span>
        )}

        <span className="text-zinc-400">
          {description}
        </span>
      </div>
    </div>
  );
}