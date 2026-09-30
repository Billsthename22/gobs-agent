"use client";

import {
  Activity,
  Clock3,
  Monitor,
  Timer,
} from "lucide-react";
import { useEffect, useState } from "react";

interface Employee {
  id: number;
  full_name: string;
  status: string;
  is_active: boolean;
}

interface Device {
  id: number;
  device_name: string;
  hostname: string;
  employee_id: number | null;
  status: string;
}

interface EmployeeOverviewProps {
  employeeId: string;
}

export default function EmployeeOverview({
  employeeId,
}: EmployeeOverviewProps) {
  const [employee, setEmployee] = useState<Employee | null>(null);
  const [device, setDevice] = useState<Device | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadOverview() {
      try {
        setLoading(true);

        const [employeeResponse, devicesResponse] =
          await Promise.all([
            fetch(
              `https://gbos-backend-production.up.railway.app/employees/${employeeId}`,
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

        if (!employeeResponse.ok) {
          throw new Error("Failed to load employee");
        }

        if (!devicesResponse.ok) {
          throw new Error("Failed to load devices");
        }

        const employeeData: Employee =
          await employeeResponse.json();

        const devicesData: Device[] =
          await devicesResponse.json();

        const assignedDevice =
          devicesData.find(
            (item) =>
              item.employee_id === employeeData.id
          ) ?? null;

        if (!cancelled) {
          setEmployee(employeeData);
          setDevice(assignedDevice);
        }
      } catch (error) {
        console.error(
          "Failed to load employee overview:",
          error
        );

        if (!cancelled) {
          setEmployee(null);
          setDevice(null);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadOverview();

    return () => {
      cancelled = true;
    };
  }, [employeeId]);

  const employeeIsActive =
    employee?.is_active &&
    employee.status.toLowerCase() === "active";

  const deviceIsOnline =
    device?.status.toLowerCase() === "online";

  const metrics = [
    {
      label: "Current Status",
      value: loading
        ? "Loading..."
        : employeeIsActive
          ? "Active"
          : "Offline",
      detail: loading
        ? "Checking employee status"
        : employeeIsActive
          ? "Employee is active"
          : "Employee is inactive",
      icon: Activity,
    },
    {
      label: "Session Duration",
      value: "—",
      detail: "Session tracking coming soon",
      icon: Timer,
    },
    {
      label: "Active Time",
      value: "—",
      detail: "Activity tracking coming soon",
      icon: Clock3,
    },
    {
      label: "Assigned Device",
      value: loading
        ? "Loading..."
        : device
          ? device.device_name
          : "None",
      detail: loading
        ? "Finding assigned device"
        : device
          ? deviceIsOnline
            ? "Online"
            : "Offline"
          : "No device assigned",
      icon: Monitor,
    },
  ];

  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
      {metrics.map((metric) => {
        const Icon = metric.icon;

        return (
          <div
            key={metric.label}
            className="rounded-2xl border border-zinc-200 bg-white p-5"
          >
            <div className="flex items-center justify-between">
              <p className="text-xs text-zinc-500">
                {metric.label}
              </p>

              <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-zinc-100">
                <Icon
                  size={17}
                  className="text-zinc-600"
                />
              </div>
            </div>

            <p className="mt-4 truncate text-2xl font-semibold tracking-tight text-zinc-950">
              {metric.value}
            </p>

            <p className="mt-1 text-xs text-zinc-400">
              {metric.detail}
            </p>
          </div>
        );
      })}
    </div>
  );
}
