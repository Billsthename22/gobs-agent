"use client";

import {
  Building2,
  Mail,
  Phone,
  UserRound,
} from "lucide-react";
import { useEffect, useState } from "react";

interface Employee {
  id: number;
  full_name: string;
  email: string;
  phone: string | null;
  department: string | null;
  created_at: string;
}

interface EmployeeInfoProps {
  employeeId: string;
}

export default function EmployeeInfo({
  employeeId,
}: EmployeeInfoProps) {
  const [employee, setEmployee] =
    useState<Employee | null>(null);

  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadEmployee() {
      try {
        setLoading(true);

        const response = await fetch(
          `https://gbos-backend-production.up.railway.app/employees/${employeeId}`,
          {
            cache: "no-store",
          }
        );

        if (!response.ok) {
          throw new Error("Failed to load employee");
        }

        const data: Employee = await response.json();

        if (!cancelled) {
          setEmployee(data);
        }
      } catch (error) {
        console.error(
          "Failed to load employee information:",
          error
        );

        if (!cancelled) {
          setEmployee(null);
        }
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
  }, [employeeId]);

  const employeeCode = employee
    ? `EMP-${String(employee.id).padStart(3, "0")}`
    : "—";

  const joinedDate = employee?.created_at
    ? new Date(employee.created_at).toLocaleDateString(
        "en-US",
        {
          month: "long",
          day: "numeric",
          year: "numeric",
        }
      )
    : "—";

  const information = [
    {
      label: "Full Name",
      value: employee?.full_name ?? "—",
      icon: UserRound,
    },
    {
      label: "Email",
      value: employee?.email ?? "—",
      icon: Mail,
    },
    {
      label: "Phone",
      value: employee?.phone || "—",
      icon: Phone,
    },
    {
      label: "Department",
      value: employee?.department || "—",
      icon: Building2,
    },
    {
      label: "Employee ID",
      value: employeeCode,
      icon: null,
    },
    {
      label: "Joined",
      value: joinedDate,
      icon: null,
    },
  ];

  return (
    <div className="rounded-2xl border border-zinc-200 bg-white">
      <div className="border-b border-zinc-100 px-6 py-5">
        <h2 className="text-sm font-semibold text-zinc-950">
          Employee Information
        </h2>

        <p className="mt-1 text-xs text-zinc-500">
          Employee and organizational details
        </p>
      </div>

      <div className="grid gap-x-8 gap-y-6 p-6">
        {information.map((item) => {
          const Icon = item.icon;

          return (
            <div key={item.label}>
              <div className="flex items-center gap-2 text-xs text-zinc-400">
                {Icon && <Icon size={14} />}
                {item.label}
              </div>

              <p className="mt-2 text-sm font-medium text-zinc-900">
                {loading ? "Loading..." : item.value}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
}
