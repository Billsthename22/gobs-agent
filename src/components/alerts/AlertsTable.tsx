"use client";

import Link from "next/link";
import {
  AlertCircle,
  AlertTriangle,
  Bell,
  CheckCircle2,
  Info,
  Search,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";

type Severity = "Critical" | "Warning" | "Info" | "Resolved";

interface AlertRecord {
  id: number;
  device_id: number;
  employee_id: number | null;
  alert_type: string;
  severity: string;
  title: string;
  description: string;
  is_resolved: boolean;
  created_at: string;
  resolved_at: string | null;
}

interface Employee {
  id: number;
  full_name: string;
  email: string;
}

interface Device {
  id: number;
  device_name: string;
  hostname: string;
  employee_id: number | null;
}

interface AlertEvent {
  id: string;
  employeeId: string;
  employee: string;
  deviceId: string;
  device: string;
  severity: Severity;
  message: string;
  time: string;
}

const severityFilters = [
  "All Alerts",
  "Critical",
  "Warning",
  "Info",
  "Resolved",
];

function normalizeSeverity(
  severity: string,
  isResolved: boolean
): Severity {
  if (isResolved) {
    return "Resolved";
  }

  switch (severity.toLowerCase()) {
    case "critical":
    case "high":
      return "Critical";
    case "warning":
    case "medium":
      return "Warning";
    case "info":
    case "low":
    default:
      return "Info";
  }
}

function formatRelativeTime(timestamp: string) {
  const date = new Date(timestamp);
  const now = new Date();
  const diffSeconds = Math.max(
    0,
    Math.floor((now.getTime() - date.getTime()) / 1000)
  );

  if (diffSeconds < 60) {
    return "Just now";
  }

  const minutes = Math.floor(diffSeconds / 60);

  if (minutes < 60) {
    return `${minutes} min ago`;
  }

  const hours = Math.floor(minutes / 60);

  if (hours < 24) {
    return `${hours} hr ago`;
  }

  const days = Math.floor(hours / 24);

  if (days < 7) {
    return `${days} day${days === 1 ? "" : "s"} ago`;
  }

  return date.toLocaleDateString();
}

function getSeverityIcon(severity: Severity) {
  switch (severity) {
    case "Critical":
      return <AlertCircle size={15} />;
    case "Warning":
      return <AlertTriangle size={15} />;
    case "Info":
      return <Info size={15} />;
    case "Resolved":
      return <CheckCircle2 size={15} />;
  }
}

function getSeverityStyle(severity: Severity) {
  switch (severity) {
    case "Critical":
      return "bg-red-50 text-red-600";
    case "Warning":
      return "bg-amber-50 text-amber-600";
    case "Info":
      return "bg-blue-50 text-blue-600";
    case "Resolved":
      return "bg-emerald-50 text-emerald-600";
  }
}

export default function AlertsTable() {
  const [alerts, setAlerts] = useState<AlertEvent[]>([]);
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState("All Alerts");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadAlerts() {
      try {
        const [alertsResponse, employeesResponse, devicesResponse] =
          await Promise.all([
            fetch("https://gbos-backend-production.up.railway.app/alerts", {
              cache: "no-store",
            }),
            fetch("https://gbos-backend-production.up.railway.app/employees", {
              cache: "no-store",
            }),
            fetch("https://gbos-backend-production.up.railway.app/devices", {
              cache: "no-store",
            }),
          ]);

        if (!alertsResponse.ok) {
          throw new Error("Failed to load alerts");
        }

        if (!employeesResponse.ok) {
          throw new Error("Failed to load employees");
        }

        if (!devicesResponse.ok) {
          throw new Error("Failed to load devices");
        }

        const [alertRecords, employees, devices]: [
          AlertRecord[],
          Employee[],
          Device[],
        ] = await Promise.all([
          alertsResponse.json(),
          employeesResponse.json(),
          devicesResponse.json(),
        ]);

        const employeeMap = new Map(
          employees.map((employee) => [employee.id, employee])
        );

        const deviceMap = new Map(
          devices.map((device) => [device.id, device])
        );

        const mappedAlerts = alertRecords
          .map((alert) => {
            const employee = alert.employee_id
              ? employeeMap.get(alert.employee_id)
              : undefined;

            const device = deviceMap.get(alert.device_id);

            return {
              id: String(alert.id),
              employeeId: alert.employee_id
                ? String(alert.employee_id)
                : "—",
              employee: employee?.full_name ?? "Unassigned",
              deviceId: String(alert.device_id),
              device: device?.device_name ?? "Unknown device",
              severity: normalizeSeverity(
                alert.severity,
                alert.is_resolved
              ),
              message: alert.title || alert.description,
              time: formatRelativeTime(alert.created_at),
            };
          })
          .sort((a, b) => Number(b.id) - Number(a.id));

        if (!cancelled) {
          setAlerts(mappedAlerts);
        }
      } catch (error) {
        console.error("Failed to load alerts:", error);

        if (!cancelled) {
          setAlerts([]);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadAlerts();

    const interval = window.setInterval(loadAlerts, 5000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, []);

  const filteredAlerts = useMemo(() => {
    return alerts.filter((alert) => {
      const matchesFilter =
        filter === "All Alerts" || alert.severity === filter;

      const query = search.toLowerCase();

      const matchesSearch =
        alert.employee.toLowerCase().includes(query) ||
        alert.employeeId.toLowerCase().includes(query) ||
        alert.device.toLowerCase().includes(query) ||
        alert.deviceId.toLowerCase().includes(query) ||
        alert.message.toLowerCase().includes(query) ||
        alert.id.toLowerCase().includes(query);

      return matchesFilter && matchesSearch;
    });
  }, [alerts, search, filter]);

  return (
    <div className="overflow-hidden rounded-2xl border border-zinc-200 bg-white">
      {/* Header */}
      <div className="flex flex-col gap-4 border-b border-zinc-100 p-5 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h2 className="text-sm font-semibold text-zinc-950">
            Recent Alerts
          </h2>
          <p className="mt-1 text-xs text-zinc-500">
            Issues detected across company devices
          </p>
        </div>

        <div className="flex flex-col gap-3 sm:flex-row">
          {/* Search */}
          <div className="relative">
            <Search
              size={15}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400"
            />

            <input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search alerts..."
              className="h-9 w-full rounded-lg border border-zinc-200 bg-white pl-9 pr-3 text-xs text-zinc-900 outline-none placeholder:text-zinc-400 focus:border-zinc-400 sm:w-56"
            />
          </div>

          {/* Filter */}
          <select
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
            className="h-9 rounded-lg border border-zinc-200 bg-white px-3 text-xs text-zinc-700 outline-none focus:border-zinc-400"
          >
            {severityFilters.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Table */}
      <div className="overflow-x-auto">
        <table className="w-full min-w-[750px]">
          <thead>
            <tr className="border-b border-zinc-100 bg-zinc-50/60">
              <th className="px-5 py-3 text-left text-[11px] font-medium uppercase tracking-wide text-zinc-400">
                Device
              </th>

              <th className="px-5 py-3 text-left text-[11px] font-medium uppercase tracking-wide text-zinc-400">
                Alert
              </th>

              <th className="px-5 py-3 text-left text-[11px] font-medium uppercase tracking-wide text-zinc-400">
                Employee
              </th>

              <th className="px-5 py-3 text-right text-[11px] font-medium uppercase tracking-wide text-zinc-400">
                Time
              </th>
            </tr>
          </thead>

          <tbody className="divide-y divide-zinc-100">
            {filteredAlerts.map((alert) => (
              <tr
                key={alert.id}
                className="transition hover:bg-zinc-50/70"
              >
                {/* Device */}
                <td className="px-5 py-4">
                  <Link
                    href={`/devices/${alert.deviceId}`}
                    className="group block"
                  >
                    <p className="text-sm font-medium text-zinc-700 transition group-hover:text-zinc-950">
                      Device {alert.deviceId}
                    </p>

                    <p className="mt-0.5 text-xs text-zinc-400">
                      {alert.device}
                    </p>
                  </Link>
                </td>

                {/* Alert */}
                <td className="px-5 py-4">
                  <div className="flex items-center gap-2.5">
                    <div
                      className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ${getSeverityStyle(
                        alert.severity
                      )}`}
                    >
                      {getSeverityIcon(alert.severity)}
                    </div>

                    <div>
                      <p className="text-sm font-medium text-zinc-800">
                        {alert.message}
                      </p>

                      <p className="mt-0.5 text-xs text-zinc-400">
                        {alert.severity} · Alert {alert.id}
                      </p>
                    </div>
                  </div>
                </td>

                {/* Employee */}
                <td className="px-5 py-4">
                  {alert.employeeId !== "—" ? (
                    <Link
                      href={`/employees/${alert.employeeId}`}
                      className="group block"
                    >
                      <p className="text-sm font-medium text-zinc-900 transition group-hover:text-zinc-600">
                        {alert.employee}
                      </p>

                      <p className="mt-0.5 text-xs text-zinc-400">
                        Employee {alert.employeeId}
                      </p>
                    </Link>
                  ) : (
                    <div>
                      <p className="text-sm font-medium text-zinc-700">
                        {alert.employee}
                      </p>

                      <p className="mt-0.5 text-xs text-zinc-400">
                        No employee assigned
                      </p>
                    </div>
                  )}
                </td>

                {/* Time */}
                <td className="px-5 py-4 text-right">
                  <span className="text-xs text-zinc-400">
                    {alert.time}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>

        {loading ? (
          <div className="flex min-h-40 items-center justify-center">
            <div className="text-center">
              <Bell
                size={28}
                strokeWidth={1.5}
                className="mx-auto animate-pulse text-zinc-300"
              />

              <p className="mt-3 text-sm font-medium text-zinc-700">
                Loading alerts...
              </p>

              <p className="mt-1 text-xs text-zinc-400">
                Checking the GBOS alert service.
              </p>
            </div>
          </div>
        ) : filteredAlerts.length === 0 ? (
          <div className="flex min-h-40 items-center justify-center">
            <div className="text-center">
              <Bell
                size={28}
                strokeWidth={1.5}
                className="mx-auto text-zinc-300"
              />

              <p className="mt-3 text-sm font-medium text-zinc-700">
                {alerts.length === 0
                  ? "No alerts recorded"
                  : "No alerts found"}
              </p>

              <p className="mt-1 text-xs text-zinc-400">
                {alerts.length === 0
                  ? "Alerts will appear here when GBOS detects an issue."
                  : "Try changing your search or filter."}
              </p>
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}
