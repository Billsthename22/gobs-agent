import Sidebar from "@/components/layout/Sidebar";
import Topbar from "@/components/layout/Topbar";

import EmployeeHeader from "@/components/employees/EmployeeHeader";
import EmployeeOverview from "@/components/employees/EmployeeOverview";
import EmployeeActivity from "@/components/employees/EmployeeActivity";
import EmployeeDevice from "@/components/employees/EmployeeDevice";
import EmployeeInfo from "@/components/employees/EmployeeInfo";

interface EmployeePageProps {
  params: Promise<{
    id: string;
  }>;
}

export default async function EmployeePage({
  params,
}: EmployeePageProps) {
  const { id } = await params;

  return (
    <div className="min-h-screen bg-zinc-50">
      <Sidebar />
      <Topbar />

      <main className="ml-64 pt-20">
        <div className="space-y-6 p-8">
          <EmployeeHeader id={id} />

          <EmployeeOverview employeeId={id} />

          <div className="grid gap-6 xl:grid-cols-[1.4fr_1fr]">
            <EmployeeActivity />
            <div className="space-y-6">
              <EmployeeDevice employeeId={id} />
              <EmployeeInfo employeeId={id} />
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}