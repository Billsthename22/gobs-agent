"use client";

import {
  Activity,
  CircleOff,
  Plus,
  UserCheck,
  Users,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";

import Sidebar from "@/components/layout/Sidebar";
import Topbar from "@/components/layout/Topbar";
import StatCard from "@/components/dashboard/StatCard";
import EmployeeTable from "@/components/employees/EmployeeTable";

export default function EmployeesPage() {
  const [showModal, setShowModal] = useState(false);
  const [editingEmployeeId, setEditingEmployeeId] = useState<number | null>(
    null
  );
  const [refreshKey, setRefreshKey] = useState(0);
  const [employees, setEmployees] = useState<any[]>([]);
  const [devices, setDevices] = useState<any[]>([]);

  const [fullName, setFullName] = useState("");
  const [email, setEmail] = useState("");
  const [department, setDepartment] = useState("");
  const [jobTitle, setJobTitle] = useState("");
  const [phone, setPhone] = useState("");

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    let cancelled = false;

    async function loadStats() {
      try {
        const [employeesResponse, devicesResponse] =
          await Promise.all([
            fetch("https://gbos-backend-production.up.railway.app/employees", {
              cache: "no-store",
            }),
            fetch("https://gbos-backend-production.up.railway.app/devices", {
              cache: "no-store",
            }),
          ]);

        if (!employeesResponse.ok || !devicesResponse.ok) {
          throw new Error("Failed to load employee statistics.");
        }

        const employeeData = await employeesResponse.json();
        const deviceData = await devicesResponse.json();

        if (!cancelled) {
          setEmployees(employeeData);
          setDevices(deviceData);
        }
      } catch (err) {
        console.error("Failed to load employee statistics:", err);
      }
    }

    loadStats();

    const interval = window.setInterval(loadStats, 5000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [refreshKey]);

  useEffect(() => {
    function handleEditEmployee(event: Event) {
      const customEvent = event as CustomEvent;
      const employee = customEvent.detail;

      if (!employee) return;

      setEditingEmployeeId(employee.id);
      setFullName(employee.full_name ?? "");
      setEmail(employee.email ?? "");
      setDepartment(employee.department ?? "");
      setJobTitle(employee.job_title ?? "");
      setPhone(employee.phone ?? "");
      setError("");
      setShowModal(true);
    }

    window.addEventListener("gbos:edit-employee", handleEditEmployee);

    return () => {
      window.removeEventListener(
        "gbos:edit-employee",
        handleEditEmployee
      );
    };
  }, []);

  const employeeStats = useMemo(() => {
    const total = employees.length;

    const active = employees.filter(
      (employee) =>
        employee.is_active &&
        employee.status?.toLowerCase() === "active"
    ).length;

    const onlineEmployeeIds = new Set(
      devices
        .filter(
          (device) =>
            device.status?.toLowerCase() === "online" &&
            device.employee_id !== null
        )
        .map((device) => device.employee_id)
    );

    const online = onlineEmployeeIds.size;

    const offline = Math.max(active - online, 0);

    return {
      total,
      active,
      online,
      offline,
    };
  }, [employees, devices]);

  function resetForm() {
    setFullName("");
    setEmail("");
    setDepartment("");
    setJobTitle("");
    setPhone("");
    setError("");
  }

  function closeModal() {
    if (saving) return;

    setShowModal(false);
    resetForm();
  }

  async function handleSaveEmployee(
    event: FormEvent<HTMLFormElement>
  ) {
    event.preventDefault();

    const isEditing = editingEmployeeId !== null;

    try {
      setSaving(true);
      setError("");

      const response = await fetch(
        isEditing
          ? `https://gbos-backend-production.up.railway.app/employees/${editingEmployeeId}`
          : "https://gbos-backend-production.up.railway.app/employees",
        {
          method: isEditing ? "PATCH" : "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            full_name: fullName.trim(),
            email: email.trim(),
            department: department.trim() || null,
            job_title: jobTitle.trim() || null,
            phone: phone.trim() || null,
          }),
        }
      );

      const data = await response.json();

      if (!response.ok) {
        throw new Error(
          data?.detail ||
            (isEditing
              ? "Failed to update employee."
              : "Failed to create employee.")
        );
      }

      setShowModal(false);
      setEditingEmployeeId(null);
      resetForm();
      setRefreshKey((value) => value + 1);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : isEditing
            ? "Failed to update employee."
            : "Failed to create employee."
      );
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="min-h-screen bg-zinc-50">
      <Sidebar />
      <Topbar />

      <main className="ml-64 pt-20">
        <div className="space-y-6 p-8">
          {/* Page Header */}
          <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
            <div>
              <p className="text-sm font-medium text-zinc-500">
                Workforce Management
              </p>

              <h1 className="mt-1 text-2xl font-semibold tracking-tight text-zinc-950">
                Employees
              </h1>

              <p className="mt-2 text-sm text-zinc-500">
                Monitor employee sessions and their assigned company devices.
              </p>
            </div>

            <button
              type="button"
              onClick={() => {
                setEditingEmployeeId(null);
                resetForm();
                setShowModal(true);
              }}
              className="inline-flex h-10 items-center justify-center gap-2 rounded-xl bg-zinc-950 px-4 text-sm font-medium text-white transition hover:bg-zinc-800"
            >
              <Plus size={16} />
              Add Employee
            </button>
          </div>

          {/* Stats */}
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            <StatCard
              title="Total Employees"
              value={String(employeeStats.total)}
              description="registered employees"
              icon={Users}
            />

            <StatCard
              title="Active Now"
              value={String(employeeStats.active)}
              description="currently working"
              trend="+2 today"
              icon={Activity}
            />

            <StatCard
              title="Online"
              value={String(employeeStats.online)}
              description="connected devices"
              trend="79%"
              icon={UserCheck}
            />

            <StatCard
              title="Offline"
              value={String(employeeStats.offline)}
              description="employees currently offline"
              icon={CircleOff}
            />
          </div>

          {/* Employee Table */}
          <EmployeeTable refreshKey={refreshKey} />
        </div>
      </main>

      {/* Add Employee Modal */}
      {showModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
          <div className="w-full max-w-lg rounded-2xl border border-zinc-200 bg-white shadow-xl">
            {/* Modal Header */}
            <div className="flex items-center justify-between border-b border-zinc-100 px-6 py-5">
              <div>
                <h2 className="text-lg font-semibold text-zinc-950">
                  {editingEmployeeId !== null
                    ? "Edit Employee"
                    : "Add Employee"}
                </h2>

                <p className="mt-1 text-sm text-zinc-500">
                  {editingEmployeeId !== null
                    ? "Update this employee's information."
                    : "Add a new employee to the workforce."}
                </p>
              </div>

              <button
                type="button"
                onClick={closeModal}
                disabled={saving}
                aria-label="Close modal"
                className="flex h-8 w-8 items-center justify-center rounded-lg text-zinc-400 transition hover:bg-zinc-100 hover:text-zinc-900 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <X size={18} />
              </button>
            </div>

            {/* Form */}
            <form
              onSubmit={handleSaveEmployee}
              className="space-y-5 p-6"
            >
              <div className="grid gap-5 sm:grid-cols-2">
                <div className="sm:col-span-2">
                  <label className="mb-2 block text-sm font-medium text-zinc-700">
                    Full name
                  </label>

                  <input
                    required
                    value={fullName}
                    onChange={(event) =>
                      setFullName(event.target.value)
                    }
                    placeholder="e.g. John Doe"
                    className="h-10 w-full rounded-xl border border-zinc-200 bg-white px-3 text-sm outline-none transition placeholder:text-zinc-400 focus:border-zinc-400"
                  />
                </div>

                <div className="sm:col-span-2">
                  <label className="mb-2 block text-sm font-medium text-zinc-700">
                    Email
                  </label>

                  <input
                    required
                    type="email"
                    value={email}
                    onChange={(event) =>
                      setEmail(event.target.value)
                    }
                    placeholder="employee@company.com"
                    className="h-10 w-full rounded-xl border border-zinc-200 bg-white px-3 text-sm outline-none transition placeholder:text-zinc-400 focus:border-zinc-400"
                  />
                </div>

                <div>
                  <label className="mb-2 block text-sm font-medium text-zinc-700">
                    Department
                  </label>

                  <input
                    value={department}
                    onChange={(event) =>
                      setDepartment(event.target.value)
                    }
                    placeholder="e.g. IT"
                    className="h-10 w-full rounded-xl border border-zinc-200 bg-white px-3 text-sm outline-none transition placeholder:text-zinc-400 focus:border-zinc-400"
                  />
                </div>

                <div>
                  <label className="mb-2 block text-sm font-medium text-zinc-700">
                    Job title
                  </label>

                  <input
                    value={jobTitle}
                    onChange={(event) =>
                      setJobTitle(event.target.value)
                    }
                    placeholder="e.g. Developer"
                    className="h-10 w-full rounded-xl border border-zinc-200 bg-white px-3 text-sm outline-none transition placeholder:text-zinc-400 focus:border-zinc-400"
                  />
                </div>

                <div className="sm:col-span-2">
                  <label className="mb-2 block text-sm font-medium text-zinc-700">
                    Phone
                  </label>

                  <input
                    value={phone}
                    onChange={(event) =>
                      setPhone(event.target.value)
                    }
                    placeholder="+234..."
                    className="h-10 w-full rounded-xl border border-zinc-200 bg-white px-3 text-sm outline-none transition placeholder:text-zinc-400 focus:border-zinc-400"
                  />
                </div>
              </div>

              {error && (
                <div className="rounded-xl border border-red-200 bg-red-50 px-3 py-2.5 text-sm text-red-600">
                  {error}
                </div>
              )}

              <div className="flex justify-end gap-3 border-t border-zinc-100 pt-5">
                <button
                  type="button"
                  onClick={closeModal}
                  disabled={saving}
                  className="h-10 rounded-xl border border-zinc-200 px-4 text-sm font-medium text-zinc-600 transition hover:bg-zinc-50 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  Cancel
                </button>

                <button
                  type="submit"
                  disabled={saving}
                  className="h-10 rounded-xl bg-zinc-950 px-5 text-sm font-medium text-white transition hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {saving
                    ? editingEmployeeId !== null
                      ? "Saving..."
                      : "Creating..."
                    : editingEmployeeId !== null
                      ? "Save Changes"
                      : "Create Employee"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
