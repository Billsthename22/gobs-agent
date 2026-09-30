"use client";

import Link from "next/link";
import {
  ArrowLeft,
  CheckCircle2,
  CircleAlert,
  MoreHorizontal,
  UserRound,
} from "lucide-react";
import { useEffect, useState } from "react";

interface Employee {
  id: number;
  full_name: string;
  department: string | null;
  status: string;
  is_active: boolean;
}

interface Device {
  id: number;
  device_name: string;
  employee_id: number | null;
}

interface EmployeeHeaderProps {
  id: string;
}

export default function EmployeeHeader({
  id,
}: EmployeeHeaderProps) {
  const [employee, setEmployee] =
    useState<Employee | null>(null);

  const [device, setDevice] =
    useState<Device | null>(null);

  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadEmployee() {
      try {
        setLoading(true);

        const [employeeResponse, devicesResponse] =
          await Promise.all([
            fetch(
              `https://gbos-backend-production.up.railway.app/employees/${id}`,
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
          "Failed to load employee header:",
          error
        );
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadEmployee();

    return () => {
      cancelled = true;
    };
  }, [id]);

  const employeeIsActive =
    employee?.is_active &&
    employee.status.toLowerCase() === "active";

  const employeeCode = employee
    ? `EMP-${String(employee.id).padStart(3, "0")}`
    : "—";

  return (
    <div className="flex flex-col gap-5 lg:flex-row lg:items-center lg:justify-between">
      <div>
        <Link
          href="/employees"
          className="mb-4 inline-flex items-center gap-2 text-xs text-zinc-500 transition hover:text-zinc-950"
        >
          <ArrowLeft size={14} />
          <span>Back to employees</span>
        </Link>

        <div className="flex items-center gap-4">
          <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-zinc-950 text-white">
            <UserRound
              size={22}
              strokeWidth={1.8}
            />
          </div>

          <div>
            <div className="flex items-center gap-2">
              <h1 className="text-2xl font-semibold tracking-tight text-zinc-950">
                {loading
                  ? "Loading..."
                  : employee?.full_name ?? "Employee not found"}
              </h1>

              {!loading && employee && (
                <span
                  className={`flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-medium ${
                    employeeIsActive
                      ? "bg-emerald-50 text-emerald-600"
                      : "bg-zinc-100 text-zinc-500"
                  }`}
                >
                  {employeeIsActive ? (
                    <CheckCircle2 size={12} />
                  ) : (
                    <CircleAlert size={12} />
                  )}

                  {employeeIsActive
                    ? "Active"
                    : "Inactive"}
                </span>
              )}
            </div>

            <p className="mt-1 text-sm text-zinc-500">
              {loading
                ? "Loading employee information..."
                : [
                    employee?.department || "No department",
                    employeeCode,
                    device
                      ? `Assigned to ${device.device_name}`
                      : "No device assigned",
                  ].join(" · ")}
            </p>
          </div>
        </div>
      </div>

      <button
        type="button"
        aria-label="Employee options"
        className="flex h-10 w-10 items-center justify-center rounded-xl border border-zinc-200 bg-white text-zinc-500 transition hover:bg-zinc-50 hover:text-zinc-950"
      >
        <MoreHorizontal size={18} />
      </button>
    </div>
  );
}
