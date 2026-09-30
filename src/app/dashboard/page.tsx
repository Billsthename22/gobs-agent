"use client";

import { useEffect, useState } from "react";
import {
  Activity,
  AlertTriangle,
  Apple,
  CheckCircle2,
  Clock3,
  Download,
  HardDrive,
  Monitor,
  Users,
  Wifi,
  X,
} from "lucide-react";

import Sidebar from "@/components/layout/Sidebar";
import Topbar from "@/components/layout/Topbar";
import StatCard from "@/components/dashboard/StatCard";

type DashboardOverview = {
  total_devices: number;
  online_devices: number;
  stale_devices: number;
  offline_devices: number;
  total_employees: number;
  active_employees: number;
  active_alerts: number;
  resolved_alerts: number;
};

type DeviceHealth = {
  id: number;
  device_name: string;
  hostname: string;
  status: string;
  cpu_usage: number;
  memory_usage: number;
  storage_usage: number;
  network_speed_mbps: number;
  last_seen: string | null;
  telemetry_recorded_at: string | null;
};

type ActivityItem = {
  id: number;
  device_id: number;
  employee_id: number | null;
  activity_type: string;
  description: string | null;
  timestamp: string;
};

type DashboardData = {
  overview: DashboardOverview;
  deviceHealth: DeviceHealth[];
  recentActivity: ActivityItem[];
};

function statusClasses(status: string) {
  switch (status) {
    case "online":
      return "bg-emerald-50 text-emerald-700";
    case "stale":
      return "bg-amber-50 text-amber-700";
    default:
      return "bg-red-50 text-red-700";
  }
}

function formatStatus(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

function formatActivityType(type: string) {
  return type
    .replaceAll("_", " ")
    .toLowerCase()
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function formatTime(timestamp: string) {
  return new Date(timestamp).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function usageClass(value: number) {
  if (value >= 90) return "text-red-600";
  if (value >= 75) return "text-amber-600";
  return "text-zinc-950";
}

export default function DashboardPage() {
  const [data, setData] = useState<DashboardData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [downloadModalOpen, setDownloadModalOpen] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function loadDashboard() {
      try {
        const [overviewResponse, healthResponse, activityResponse] =
          await Promise.all([
            fetch("https://gbos-backend-production.up.railway.app/dashboard/overview", {
              cache: "no-store",
            }),
            fetch("https://gbos-backend-production.up.railway.app/dashboard/device-health", {
              cache: "no-store",
            }),
            fetch("https://gbos-backend-production.up.railway.app/dashboard/recent-activity", {
              cache: "no-store",
            }),
          ]);

        if (
          !overviewResponse.ok ||
          !healthResponse.ok ||
          !activityResponse.ok
        ) {
          throw new Error("Unable to load dashboard data.");
        }

        const [overview, deviceHealth, recentActivity] =
          await Promise.all([
            overviewResponse.json(),
            healthResponse.json(),
            activityResponse.json(),
          ]);

        if (!cancelled) {
          setData({
            overview,
            deviceHealth,
            recentActivity,
          });
          setError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setError(
            err instanceof Error
              ? err.message
              : "Unable to load dashboard.",
          );
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadDashboard();

    const refreshInterval = window.setInterval(
      loadDashboard,
      30_000,
    );

    return () => {
      cancelled = true;
      window.clearInterval(refreshInterval);
    };
  }, []);

  return (
    <div className="min-h-screen bg-zinc-50">
      <Sidebar />
      <Topbar />

      <main className="ml-64 pt-20">
        <div className="space-y-6 p-8">
          <div className="flex flex-col gap-5 sm:flex-row sm:items-end sm:justify-between">
            <div>
              <p className="text-sm font-medium text-zinc-500">
                Dashboard
              </p>

              <h1 className="mt-1 text-2xl font-semibold tracking-tight text-zinc-950">
                Welcome back, Administrator
              </h1>

              <p className="mt-2 text-sm text-zinc-500">
                Here&apos;s what&apos;s happening across your company&apos;s
                devices.
              </p>
            </div>

            <button
              type="button"
              onClick={() => setDownloadModalOpen(true)}
              className="inline-flex h-10 items-center justify-center gap-2 rounded-xl bg-zinc-950 px-4 text-sm font-medium text-white shadow-sm transition hover:bg-zinc-800 active:scale-[0.98]"
            >
              <Download size={16} />
              Download Agent
            </button>
          </div>

          {loading && !data ? (
            <div className="space-y-6">
              <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                {[1, 2, 3, 4].map((item) => (
                  <div
                    key={item}
                    className="h-32 animate-pulse rounded-2xl border border-zinc-200 bg-zinc-100"
                  />
                ))}
              </div>

              <div className="h-96 animate-pulse rounded-2xl border border-zinc-200 bg-zinc-100" />
            </div>
          ) : error && !data ? (
            <div className="rounded-2xl border border-red-200 bg-red-50 p-6">
              <h2 className="text-sm font-semibold text-red-950">
                Unable to load dashboard
              </h2>

              <p className="mt-1 text-sm text-red-700">
                {error}
              </p>
            </div>
          ) : data ? (
            <>
              <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                <StatCard
                  title="Total Devices"
                  value={data.overview.total_devices.toLocaleString()}
                  description={`${data.overview.online_devices} currently online`}
                  icon={Monitor}
                />

                <StatCard
                  title="Active Employees"
                  value={data.overview.active_employees.toLocaleString()}
                  description={`of ${data.overview.total_employees} employees`}
                  icon={Users}
                />

                <StatCard
                  title="Active Alerts"
                  value={data.overview.active_alerts.toLocaleString()}
                  description={`${data.overview.resolved_alerts} resolved`}
                  icon={AlertTriangle}
                />

                <StatCard
                  title="Device Availability"
                  value={
                    data.overview.total_devices > 0
                      ? `${Math.round(
                          (data.overview.online_devices /
                            data.overview.total_devices) *
                            100,
                        )}%`
                      : "0%"
                  }
                  description={`${data.overview.offline_devices} offline`}
                  icon={Wifi}
                />
              </div>

              <div className="grid gap-6 xl:grid-cols-[1.5fr_1fr]">
                <div className="rounded-2xl border border-zinc-200 bg-white">
                  <div className="border-b border-zinc-100 px-6 py-5">
                    <div className="flex items-center justify-between">
                      <div>
                        <h2 className="text-sm font-semibold text-zinc-950">
                          Device Health
                        </h2>

                        <p className="mt-1 text-xs text-zinc-500">
                          Current telemetry from managed devices
                        </p>
                      </div>

                      <div className="flex items-center gap-4 text-xs text-zinc-500">
                        <span>
                          Online {data.overview.online_devices}
                        </span>
                        <span>
                          Stale {data.overview.stale_devices}
                        </span>
                        <span>
                          Offline {data.overview.offline_devices}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="overflow-x-auto">
                    {data.deviceHealth.length === 0 ? (
                      <div className="px-6 py-12 text-center text-sm text-zinc-500">
                        No devices found.
                      </div>
                    ) : (
                      <table className="w-full min-w-[760px]">
                        <thead>
                          <tr className="border-b border-zinc-100 text-left text-xs text-zinc-400">
                            <th className="px-6 py-3 font-medium">
                              Device
                            </th>
                            <th className="px-4 py-3 font-medium">
                              Status
                            </th>
                            <th className="px-4 py-3 font-medium">
                              CPU
                            </th>
                            <th className="px-4 py-3 font-medium">
                              Memory
                            </th>
                            <th className="px-4 py-3 font-medium">
                              Storage
                            </th>
                            <th className="px-4 py-3 font-medium">
                              Network
                            </th>
                          </tr>
                        </thead>

                        <tbody>
                          {data.deviceHealth.map((device) => (
                            <tr
                              key={device.id}
                              className="border-b border-zinc-100 last:border-0"
                            >
                              <td className="px-6 py-4">
                                <div className="flex items-center gap-3">
                                  <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-zinc-100">
                                    <Monitor
                                      size={16}
                                      className="text-zinc-600"
                                    />
                                  </div>

                                  <div>
                                    <p className="text-sm font-medium text-zinc-950">
                                      {device.device_name}
                                    </p>

                                    <p className="mt-0.5 text-xs text-zinc-400">
                                      {device.hostname}
                                    </p>
                                  </div>
                                </div>
                              </td>

                              <td className="px-4 py-4">
                                <span
                                  className={`inline-flex rounded-full px-2.5 py-1 text-xs font-medium ${statusClasses(
                                    device.status,
                                  )}`}
                                >
                                  {formatStatus(device.status)}
                                </span>
                              </td>

                              <td
                                className={`px-4 py-4 text-sm font-medium ${usageClass(
                                  device.cpu_usage,
                                )}`}
                              >
                                {device.cpu_usage.toFixed(1)}%
                              </td>

                              <td
                                className={`px-4 py-4 text-sm font-medium ${usageClass(
                                  device.memory_usage,
                                )}`}
                              >
                                {device.memory_usage.toFixed(1)}%
                              </td>

                              <td
                                className={`px-4 py-4 text-sm font-medium ${usageClass(
                                  device.storage_usage,
                                )}`}
                              >
                                {device.storage_usage.toFixed(1)}%
                              </td>

                              <td className="px-4 py-4 text-sm text-zinc-600">
                                {device.network_speed_mbps.toFixed(1)} Mbps
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}
                  </div>
                </div>

                <div className="rounded-2xl border border-zinc-200 bg-white">
                  <div className="border-b border-zinc-100 px-6 py-5">
                    <h2 className="text-sm font-semibold text-zinc-950">
                      Recent Activity
                    </h2>

                    <p className="mt-1 text-xs text-zinc-500">
                      Latest events across managed devices
                    </p>
                  </div>

                  <div className="divide-y divide-zinc-100">
                    {data.recentActivity.length === 0 ? (
                      <div className="px-6 py-12 text-center text-sm text-zinc-500">
                        No recent activity.
                      </div>
                    ) : (
                      data.recentActivity.map((activity) => (
                        <div
                          key={activity.id}
                          className="flex gap-3 px-6 py-4"
                        >
                          <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-zinc-100">
                            {activity.activity_type.includes("ALERT") ? (
                              <AlertTriangle
                                size={15}
                                className="text-zinc-600"
                              />
                            ) : activity.activity_type.includes(
                                "CONNECTED",
                              ) ? (
                              <CheckCircle2
                                size={15}
                                className="text-zinc-600"
                              />
                            ) : activity.activity_type.includes(
                                "DISCONNECTED",
                              ) ? (
                              <Wifi
                                size={15}
                                className="text-zinc-600"
                              />
                            ) : (
                              <Activity
                                size={15}
                                className="text-zinc-600"
                              />
                            )}
                          </div>

                          <div className="min-w-0 flex-1">
                            <p className="text-xs font-medium text-zinc-950">
                              {formatActivityType(
                                activity.activity_type,
                              )}
                            </p>

                            <p className="mt-1 line-clamp-2 text-xs leading-5 text-zinc-500">
                              {activity.description ||
                                "Activity recorded."}
                            </p>

                            <div className="mt-1 flex items-center gap-1 text-[11px] text-zinc-400">
                              <Clock3 size={11} />
                              {formatTime(activity.timestamp)}
                            </div>
                          </div>
                        </div>
                      ))
                    )}
                  </div>
                </div>
              </div>

              <div className="grid gap-4 md:grid-cols-3">
                <div className="rounded-2xl border border-zinc-200 bg-white p-5">
                  <div className="flex items-center gap-3">
                    <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-zinc-100">
                      <CheckCircle2
                        size={18}
                        className="text-zinc-700"
                      />
                    </div>

                    <div>
                      <p className="text-xs text-zinc-500">
                        Online
                      </p>

                      <p className="mt-1 text-xl font-semibold text-zinc-950">
                        {data.overview.online_devices}
                      </p>
                    </div>
                  </div>
                </div>

                <div className="rounded-2xl border border-zinc-200 bg-white p-5">
                  <div className="flex items-center gap-3">
                    <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-zinc-100">
                      <Clock3
                        size={18}
                        className="text-zinc-700"
                      />
                    </div>

                    <div>
                      <p className="text-xs text-zinc-500">
                        Stale
                      </p>

                      <p className="mt-1 text-xl font-semibold text-zinc-950">
                        {data.overview.stale_devices}
                      </p>
                    </div>
                  </div>
                </div>

                <div className="rounded-2xl border border-zinc-200 bg-white p-5">
                  <div className="flex items-center gap-3">
                    <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-zinc-100">
                      <HardDrive
                        size={18}
                        className="text-zinc-700"
                      />
                    </div>

                    <div>
                      <p className="text-xs text-zinc-500">
                        Offline
                      </p>

                      <p className="mt-1 text-xl font-semibold text-zinc-950">
                        {data.overview.offline_devices}
                      </p>
                    </div>
                  </div>
                </div>
              </div>
            </>
          ) : null}
        </div>
      </main>

      {downloadModalOpen && (
        <div
          className="fixed inset-0 z-[100] flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) {
              setDownloadModalOpen(false);
            }
          }}
        >
          <div className="w-full max-w-lg overflow-hidden rounded-2xl border border-zinc-200 bg-white shadow-2xl">
            <div className="flex items-start justify-between border-b border-zinc-100 px-6 py-5">
              <div>
                <h2 className="text-lg font-semibold tracking-tight text-zinc-950">
                  Download GBOS Agent
                </h2>

                <p className="mt-1 text-sm text-zinc-500">
                  Choose the computer you want to connect to GBOS.
                </p>
              </div>

              <button
                type="button"
                onClick={() => setDownloadModalOpen(false)}
                className="flex h-9 w-9 items-center justify-center rounded-lg text-zinc-400 transition hover:bg-zinc-100 hover:text-zinc-700"
                aria-label="Close download modal"
              >
                <X size={18} />
              </button>
            </div>

            <div className="grid gap-4 p-6 sm:grid-cols-2">
              <div className="rounded-2xl border border-zinc-200 p-5 transition hover:border-zinc-300 hover:shadow-sm">
                <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-zinc-100">
                  <Monitor size={21} className="text-zinc-700" />
                </div>

                <h3 className="mt-4 text-sm font-semibold text-zinc-950">
                  Windows
                </h3>

                <p className="mt-1 min-h-[40px] text-xs leading-5 text-zinc-500">
                  Install the GBOS Agent on a Windows 10 or Windows 11 computer.
                  It runs as a background service — there is nothing to launch
                  afterwards.
                </p>

                <a
                  href="/downloads/GBOS-Agent-Windows.exe"
                  download
                  className="mt-5 inline-flex h-9 w-full items-center justify-center gap-2 rounded-lg bg-zinc-950 text-xs font-medium text-white transition hover:bg-zinc-800"
                >
                  <Download size={14} />
                  Download
                </a>

                <p className="mt-2 text-center text-[11px] text-zinc-400">
                  Windows 10/11 · 64-bit
                </p>
              </div>

              <div className="rounded-2xl border border-zinc-200 p-5 transition hover:border-zinc-300 hover:shadow-sm">
                <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-zinc-100">
                  <Apple size={21} className="text-zinc-700" />
                </div>

                <h3 className="mt-4 text-sm font-semibold text-zinc-950">
                  MacBook
                </h3>

                <p className="mt-1 min-h-[40px] text-xs leading-5 text-zinc-500">
                  Install the GBOS Agent on a Mac running macOS. It runs as a
                  background service — there is nothing to launch afterwards.
                </p>

                <a
                  href="/downloads/GBOS-Agent-macOS.dmg"
                  download
                  className="mt-5 inline-flex h-9 w-full items-center justify-center gap-2 rounded-lg bg-zinc-950 text-xs font-medium text-white transition hover:bg-zinc-800"
                >
                  <Download size={14} />
                  Download
                </a>

                <p className="mt-2 text-center text-[11px] text-zinc-400">
                  macOS 13+ · Apple Silicon
                </p>
              </div>
            </div>

            <div className="border-t border-zinc-100 bg-zinc-50 px-6 py-4">
              <p className="text-xs leading-5 text-zinc-500">
                After installation, the agent will connect the computer to GBOS
                and make it available for device management. It starts on its own
                at boot and shows no window.
              </p>

              <p className="mt-2 text-xs leading-5 text-zinc-500">
                On macOS, screen viewing and location are not available. macOS
                grants those permissions per user, and a background service has
                no user to ask, so it cannot be given them. Device status, system
                telemetry and login tracking work normally.
              </p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
