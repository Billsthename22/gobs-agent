"use client";

import Link from "next/link";
import {
  Activity as ActivityIcon,
  Laptop,
  LogIn,
  LogOut,
  Monitor,
  Search,
  Settings,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";

type ActivityType =
  | "Application"
  | "Login"
  | "Logout"
  | "Device"
  | "System";

interface Activity {
  id: number;
  device_id: number;
  employee_id: number | null;
  activity_type: string;
  description: string;
  timestamp: string;
}

interface Employee {
  id: number;
  full_name: string;
}

interface Device {
  id: number;
  device_name: string;
  hostname: string;
}

interface ActivityEvent {
  id: string;
  employeeId: string;
  employee: string;
  deviceId: string;
  device: string;
  type: ActivityType;
  event: string;
  application: string;
  time: string;
}

const filters = [
  "All Events",
  "Application",
  "Login",
  "Logout",
  "Device",
  "System",
];

function getActivityType(activityType: string): ActivityType {
  switch (activityType) {
    case "APPLICATION_ACTIVE":
      return "Application";

    case "LOGIN":
    case "EMPLOYEE_LOGIN":
      return "Login";

    case "LOGOUT":
    case "EMPLOYEE_LOGOUT":
      return "Logout";

    case "DEVICE_CONNECTED":
    case "DEVICE_DISCONNECTED":
      return "Device";

    default:
      return "System";
  }
}

function getEventLabel(activityType: string): string {
  switch (activityType) {
    case "APPLICATION_ACTIVE":
      return "Application activity";

    case "LOGIN":
    case "EMPLOYEE_LOGIN":
      return "Employee logged in";

    case "LOGOUT":
    case "EMPLOYEE_LOGOUT":
      return "Employee logged out";

    case "DEVICE_CONNECTED":
      return "Device connected";

    case "DEVICE_DISCONNECTED":
      return "Device disconnected";

    default:
      return activityType
        .toLowerCase()
        .replace(/_/g, " ");
  }
}

function getApplication(
  activityType: string,
  description: string
): string {
  if (activityType !== "APPLICATION_ACTIVE") {
    return "—";
  }

  const match = description.match(
    /^(.+?) became active on /
  );

  return match?.[1] ?? "—";
}

function formatRelativeTime(timestamp: string): string {
  const date = new Date(timestamp);
  const now = Date.now();
  const difference = Math.max(
    0,
    now - date.getTime()
  );

  const seconds = Math.floor(difference / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (seconds < 60) {
    return "Just now";
  }

  if (minutes < 60) {
    return `${minutes} min ago`;
  }

  if (hours < 24) {
    return `${hours} hr${hours === 1 ? "" : "s"} ago`;
  }

  return `${days} day${days === 1 ? "" : "s"} ago`;
}

function getIcon(type: ActivityType) {
  switch (type) {
    case "Login":
      return <LogIn size={15} />;

    case "Logout":
      return <LogOut size={15} />;

    case "Device":
      return <Monitor size={15} />;

    case "System":
      return <Settings size={15} />;

    default:
      return <ActivityIcon size={15} />;
  }
}

function getIconStyle(type: ActivityType) {
  switch (type) {
    case "Login":
      return "bg-emerald-50 text-emerald-600";

    case "Logout":
      return "bg-zinc-100 text-zinc-500";

    case "Device":
      return "bg-blue-50 text-blue-600";

    case "System":
      return "bg-amber-50 text-amber-600";

    default:
      return "bg-violet-50 text-violet-600";
  }
}

export default function ActivityTable() {
  const [activities, setActivities] = useState<ActivityEvent[]>(
    []
  );

  const [loading, setLoading] = useState(true);

  const [search, setSearch] = useState("");

  const [filter, setFilter] = useState("All Events");

  useEffect(() => {
    let cancelled = false;

    async function loadActivities() {
      try {
        setLoading(true);

        const [
          activitiesResponse,
          employeesResponse,
          devicesResponse,
        ] = await Promise.all([
          fetch(
            "https://gbos-backend-production.up.railway.app/activities",
            {
              cache: "no-store",
            }
          ),
          fetch(
            "https://gbos-backend-production.up.railway.app/employees",
            {
              cache: "no-store",
            }
          ),
          fetch(
            "https://gbos-backend-production.up.railway.app/devices",
            {
              cache: "no-store",
            }
          ),
        ]);

        if (!activitiesResponse.ok) {
          throw new Error(
            "Failed to load activities"
          );
        }

        if (!employeesResponse.ok) {
          throw new Error(
            "Failed to load employees"
          );
        }

        if (!devicesResponse.ok) {
          throw new Error(
            "Failed to load devices"
          );
        }

        const [
          activityData,
          employeeData,
          deviceData,
        ]: [
          Activity[],
          Employee[],
          Device[]
        ] = await Promise.all([
          activitiesResponse.json(),
          employeesResponse.json(),
          devicesResponse.json(),
        ]);

        const employeeMap = new Map(
          employeeData.map((employee) => [
            employee.id,
            employee,
          ])
        );

        const deviceMap = new Map(
          deviceData.map((device) => [
            device.id,
            device,
          ])
        );

        const mappedActivities: ActivityEvent[] =
          activityData
            .filter(
              (activity) =>
                activity.employee_id !== null
            )
            .sort(
              (a, b) =>
                new Date(b.timestamp).getTime() -
                new Date(a.timestamp).getTime()
            )
            .map((activity) => {
              const employee =
                employeeMap.get(
                  activity.employee_id!
                );

              const device = deviceMap.get(
                activity.device_id
              );

              const type = getActivityType(
                activity.activity_type
              );

              return {
                id: String(activity.id),
                employeeId: employee
                  ? `EMP-${String(employee.id).padStart(3, "0")}`
                  : `EMP-${activity.employee_id}`,
                employee:
                  employee?.full_name ??
                  "Unknown employee",
                deviceId: device
                  ? String(device.id)
                  : String(activity.device_id),
                device:
                  device?.device_name ??
                  "Unknown device",
                type,
                event: getEventLabel(
                  activity.activity_type
                ),
                application: getApplication(
                  activity.activity_type,
                  activity.description
                ),
                time: formatRelativeTime(
                  activity.timestamp
                ),
              };
            });

        if (!cancelled) {
          setActivities(mappedActivities);
        }
      } catch (error) {
        console.error(
          "Failed to load activity table:",
          error
        );

        if (!cancelled) {
          setActivities([]);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadActivities();

    const interval = window.setInterval(
      loadActivities,
      5000
    );

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, []);

  const filteredActivities = useMemo(() => {
    return activities.filter((activity) => {
      const matchesFilter =
        filter === "All Events" ||
        activity.type === filter;

      const query = search.toLowerCase().trim();

      if (!query) {
        return matchesFilter;
      }

      const matchesSearch =
        activity.employee
          .toLowerCase()
          .includes(query) ||
        activity.employeeId
          .toLowerCase()
          .includes(query) ||
        activity.device
          .toLowerCase()
          .includes(query) ||
        activity.deviceId
          .toLowerCase()
          .includes(query) ||
        activity.event
          .toLowerCase()
          .includes(query) ||
        activity.application
          .toLowerCase()
          .includes(query);

      return matchesFilter && matchesSearch;
    });
  }, [activities, search, filter]);

  return (
    <div className="overflow-hidden rounded-2xl border border-zinc-200 bg-white">
      {/* Header */}
      <div className="flex flex-col gap-4 border-b border-zinc-100 p-5 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h2 className="text-sm font-semibold text-zinc-950">
            Activity Timeline
          </h2>

          <p className="mt-1 text-xs text-zinc-500">
            Recent activity across company devices
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
              onChange={(event) =>
                setSearch(event.target.value)
              }
              placeholder="Search activity..."
              className="h-9 w-full rounded-lg border border-zinc-200 bg-white pl-9 pr-3 text-xs text-zinc-900 outline-none placeholder:text-zinc-400 focus:border-zinc-400 sm:w-56"
            />
          </div>

          {/* Filter */}
          <select
            value={filter}
            onChange={(event) =>
              setFilter(event.target.value)
            }
            className="h-9 rounded-lg border border-zinc-200 bg-white px-3 text-xs text-zinc-700 outline-none focus:border-zinc-400"
          >
            {filters.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Loading */}
      {loading && activities.length === 0 ? (
        <div className="flex min-h-40 items-center justify-center">
          <p className="text-sm text-zinc-400">
            Loading activity...
          </p>
        </div>
      ) : (
        <>
          {/* Table */}
          <div className="overflow-x-auto">
            <table className="w-full min-w-[850px]">
              <thead>
                <tr className="border-b border-zinc-100 bg-zinc-50/60">
                  <th className="px-5 py-3 text-left text-[11px] font-medium uppercase tracking-wide text-zinc-400">
                    Employee
                  </th>

                  <th className="px-5 py-3 text-left text-[11px] font-medium uppercase tracking-wide text-zinc-400">
                    Device
                  </th>

                  <th className="px-5 py-3 text-left text-[11px] font-medium uppercase tracking-wide text-zinc-400">
                    Event
                  </th>

                  <th className="px-5 py-3 text-left text-[11px] font-medium uppercase tracking-wide text-zinc-400">
                    Application
                  </th>

                  <th className="px-5 py-3 text-right text-[11px] font-medium uppercase tracking-wide text-zinc-400">
                    Time
                  </th>
                </tr>
              </thead>

              <tbody className="divide-y divide-zinc-100">
                {filteredActivities.map(
                  (activity) => (
                    <tr
                      key={activity.id}
                      className="transition hover:bg-zinc-50/70"
                    >
                      {/* Employee */}
                      <td className="px-5 py-4">
                        <Link
                          href={`/employees/${activity.employeeId.replace(
                            "EMP-",
                            ""
                          )}`}
                          className="group block"
                        >
                          <p className="text-sm font-medium text-zinc-900 transition group-hover:text-zinc-600">
                            {activity.employee}
                          </p>

                          <p className="mt-0.5 text-xs text-zinc-400">
                            {activity.employeeId}
                          </p>
                        </Link>
                      </td>

                      {/* Device */}
                      <td className="px-5 py-4">
                        <Link
                          href={`/devices/${activity.deviceId}`}
                          className="group block"
                        >
                          <p className="text-sm font-medium text-zinc-700 transition group-hover:text-zinc-950">
                            {activity.device}
                          </p>

                          <p className="mt-0.5 text-xs text-zinc-400">
                            Device {activity.deviceId}
                          </p>
                        </Link>
                      </td>

                      {/* Event */}
                      <td className="px-5 py-4">
                        <div className="flex items-center gap-2.5">
                          <div
                            className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ${getIconStyle(
                              activity.type
                            )}`}
                          >
                            {getIcon(
                              activity.type
                            )}
                          </div>

                          <div>
                            <p className="text-sm font-medium text-zinc-800">
                              {activity.event}
                            </p>

                            <p className="mt-0.5 text-xs text-zinc-400">
                              {activity.type}
                            </p>
                          </div>
                        </div>
                      </td>

                      {/* Application */}
                      <td className="px-5 py-4">
                        <span className="text-sm text-zinc-600">
                          {activity.application}
                        </span>
                      </td>

                      {/* Time */}
                      <td className="px-5 py-4 text-right">
                        <span className="text-xs text-zinc-400">
                          {activity.time}
                        </span>
                      </td>
                    </tr>
                  )
                )}
              </tbody>
            </table>

            {filteredActivities.length === 0 && (
              <div className="flex min-h-40 items-center justify-center">
                <div className="text-center">
                  <Laptop
                    size={28}
                    strokeWidth={1.5}
                    className="mx-auto text-zinc-300"
                  />

                  <p className="mt-3 text-sm font-medium text-zinc-700">
                    No activity found
                  </p>

                  <p className="mt-1 text-xs text-zinc-400">
                    Try changing your search or
                    filter.
                  </p>
                </div>
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
