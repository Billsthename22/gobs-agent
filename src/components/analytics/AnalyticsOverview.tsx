"use client";

import { useEffect, useState } from "react";
import {
  Activity,
  Bell,
  HardDrive,
  MonitorCheck,
  Server,
  Wifi,
} from "lucide-react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import StatCard from "@/components/dashboard/StatCard";

type AnalyticsData = {
  stats: {
    total_activity: number;
    today_activity: number;
    issues_detected: number;
    active_alerts: number;
    total_devices: number;
    online_devices: number;
    average_cpu: number;
    average_memory: number;
    average_storage: number;
    average_network_mbps: number;
  };
  uptime: {
    day: string;
    date: string;
    uptime: number;
  }[];
  employee_activity: {
    id: number;
    name: string;
    activity_events: number;
  }[];
  application_usage: {
    name: string;
    events: number;
    percentage: number;
  }[];
};

const tooltipStyle = {
  borderRadius: "12px",
  border: "1px solid #e4e4e7",
  backgroundColor: "#ffffff",
  fontSize: "12px",
};

export default function AnalyticsOverview() {
  const [data, setData] = useState<AnalyticsData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function loadAnalytics() {
      try {
        setLoading(true);
        setError(null);

        const response = await fetch(
          "https://gbos-backend-production.up.railway.app/analytics/overview",
          {
            cache: "no-store",
          },
        );

        if (!response.ok) {
          throw new Error(
            `Analytics request failed with status ${response.status}`,
          );
        }

        const result: AnalyticsData = await response.json();

        if (!cancelled) {
          setData(result);
        }
      } catch (err) {
        if (!cancelled) {
          setError(
            err instanceof Error
              ? err.message
              : "Unable to load analytics.",
          );
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadAnalytics();

    const refreshInterval = window.setInterval(() => {
      loadAnalytics();
    }, 30_000);

    return () => {
      cancelled = true;
      window.clearInterval(refreshInterval);
    };
  }, []);

  if (loading) {
    return (
      <div className="space-y-6">
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {[1, 2, 3, 4].map((item) => (
            <div
              key={item}
              className="h-32 animate-pulse rounded-2xl border border-zinc-200 bg-zinc-100"
            />
          ))}
        </div>

        <div className="h-[400px] animate-pulse rounded-2xl border border-zinc-200 bg-zinc-100" />

        <div className="grid gap-6 xl:grid-cols-[1.5fr_1fr]">
          <div className="h-[400px] animate-pulse rounded-2xl border border-zinc-200 bg-zinc-100" />
          <div className="h-[400px] animate-pulse rounded-2xl border border-zinc-200 bg-zinc-100" />
        </div>
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="rounded-2xl border border-red-200 bg-red-50 px-6 py-5">
        <h2 className="text-sm font-semibold text-red-950">
          Unable to load analytics
        </h2>

        <p className="mt-1 text-xs text-red-700">
          {error ?? "No analytics data was returned by the backend."}
        </p>
      </div>
    );
  }

  const {
    stats,
    uptime,
    employee_activity: employeeActivityData,
    application_usage: applicationData,
  } = data;

  return (
    <div className="space-y-6">
      {/* Stats */}
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <StatCard
          title="Online Devices"
          value={`${stats.online_devices}/${stats.total_devices}`}
          description="currently online"
          icon={MonitorCheck}
        />

        <StatCard
          title="Avg. CPU Usage"
          value={`${stats.average_cpu.toFixed(1)}%`}
          description="across telemetry"
          icon={Server}
        />

        <StatCard
          title="Avg. Memory"
          value={`${stats.average_memory.toFixed(1)}%`}
          description="across telemetry"
          icon={Server}
        />

        <StatCard
          title="Avg. Storage"
          value={`${stats.average_storage.toFixed(1)}%`}
          description="across telemetry"
          icon={HardDrive}
        />

        <StatCard
          title="Avg. Network"
          value={`${stats.average_network_mbps.toFixed(1)} Mbps`}
          description="average network speed"
          icon={Wifi}
        />

        <StatCard
          title="Active Alerts"
          value={stats.active_alerts.toLocaleString()}
          description="currently unresolved"
          icon={Bell}
        />

        <StatCard
          title="Today's Activity"
          value={stats.today_activity.toLocaleString()}
          description="events recorded today"
          icon={Activity}
        />

        <StatCard
          title="Issues Detected"
          value={stats.issues_detected.toLocaleString()}
          description="device issues this week"
          icon={HardDrive}
        />
      </div>

      {/* Device Uptime */}
      <div className="rounded-2xl border border-zinc-200 bg-white">
        <div className="border-b border-zinc-100 px-6 py-5">
          <h2 className="text-sm font-semibold text-zinc-950">
            Device Uptime
          </h2>

          <p className="mt-1 text-xs text-zinc-500">
            Device availability over the last 7 days
          </p>
        </div>

        <div className="h-[320px] p-6">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={uptime}>
              <defs>
                <linearGradient
                  id="uptimeGradient"
                  x1="0"
                  y1="0"
                  x2="0"
                  y2="1"
                >
                  <stop offset="5%" stopOpacity={0.18} />
                  <stop offset="95%" stopOpacity={0} />
                </linearGradient>
              </defs>

              <CartesianGrid
                strokeDasharray="3 3"
                vertical={false}
                stroke="#f4f4f5"
              />

              <XAxis
                dataKey="day"
                axisLine={false}
                tickLine={false}
                tick={{
                  fontSize: 11,
                  fill: "#a1a1aa",
                }}
              />

              <YAxis
                domain={[0, 100]}
                axisLine={false}
                tickLine={false}
                tick={{
                  fontSize: 11,
                  fill: "#a1a1aa",
                }}
                tickFormatter={(value) => `${value}%`}
              />

              <Tooltip
                contentStyle={tooltipStyle}
                formatter={(value) => [`${value}%`, "Uptime"]}
              />

              <Area
                type="monotone"
                dataKey="uptime"
                stroke="#18181b"
                strokeWidth={2}
                fill="url(#uptimeGradient)"
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Employee Activity + Application Usage */}
      <div className="grid gap-6 xl:grid-cols-[1.5fr_1fr]">
        {/* Employee Activity */}
        <div className="rounded-2xl border border-zinc-200 bg-white">
          <div className="border-b border-zinc-100 px-6 py-5">
            <h2 className="text-sm font-semibold text-zinc-950">
              Employee Activity
            </h2>

            <p className="mt-1 text-xs text-zinc-500">
              Activity events recorded over the last 7 days
            </p>
          </div>

          <div className="h-[320px] p-6">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart
                data={employeeActivityData}
                barGap={6}
                margin={{ left: 8, right: 8, bottom: 8 }}
              >
                <CartesianGrid
                  strokeDasharray="3 3"
                  vertical={false}
                  stroke="#f4f4f5"
                />

                <XAxis
                  dataKey="name"
                  axisLine={false}
                  tickLine={false}
                  tick={{
                    fontSize: 11,
                    fill: "#a1a1aa",
                  }}
                />

                <YAxis
                  axisLine={false}
                  tickLine={false}
                  tick={{
                    fontSize: 11,
                    fill: "#a1a1aa",
                  }}
                  tickFormatter={(value) =>
                    Number(value).toLocaleString()
                  }
                />

                <Tooltip
                  contentStyle={tooltipStyle}
                  formatter={(value) => [
                    Number(value).toLocaleString(),
                    "Activity events",
                  ]}
                />

                <Bar
                  dataKey="activity_events"
                  name="Activity events"
                  fill="#18181b"
                  radius={[5, 5, 0, 0]}
                />
              </BarChart>
            </ResponsiveContainer>
          </div>

          <div className="px-6 pb-6 text-xs text-zinc-500">
            Higher values indicate more recorded activity events.
          </div>
        </div>

        {/* Application Usage */}
        <div className="rounded-2xl border border-zinc-200 bg-white">
          <div className="border-b border-zinc-100 px-6 py-5">
            <h2 className="text-sm font-semibold text-zinc-950">
              Application Usage
            </h2>

            <p className="mt-1 text-xs text-zinc-500">
              Application activity share over the last 7 days
            </p>
          </div>

          <div className="h-[320px] p-6">
            <ResponsiveContainer width="100%" height="100%">
              <PieChart>
                <Pie
                  data={applicationData}
                  dataKey="percentage"
                  nameKey="name"
                  cx="50%"
                  cy="50%"
                  innerRadius={70}
                  outerRadius={105}
                  paddingAngle={3}
                >
                  {applicationData.map((entry, index) => (
                    <Cell key={`cell-${entry.name}-${index}`} />
                  ))}
                </Pie>

                <Tooltip
                  contentStyle={tooltipStyle}
                  formatter={(value, name) => [
                    `${value}%`,
                    name,
                  ]}
                />
              </PieChart>
            </ResponsiveContainer>
          </div>

          <div className="grid grid-cols-2 gap-3 px-6 pb-6">
            {applicationData.map((application) => (
              <div
                key={application.name}
                className="flex items-center justify-between gap-3 text-xs"
              >
                <span className="truncate text-zinc-500">
                  {application.name}
                </span>

                <span className="font-medium text-zinc-950">
                  {application.percentage}%
                </span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
