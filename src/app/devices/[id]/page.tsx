import Sidebar from "@/components/layout/Sidebar";
import Topbar from "@/components/layout/Topbar";
import DeviceHeader from "@/components/devices/DeviceHeader";
import DeviceHealth from "@/components/devices/DeviceHealth";
import DeviceActivity from "@/components/devices/DeviceActivity";
import DeviceInfo from "@/components/devices/DeviceInfo";

interface DevicePageProps {
  params: Promise<{
    id: string;
  }>;
}

export default async function DevicePage({
  params,
}: DevicePageProps) {
  const { id } = await params;

  return (
    <div className="min-h-screen bg-zinc-50">
      <Sidebar />
      <Topbar />

      <main className="ml-64 pt-20">
        <div className="space-y-6 p-8">
          <DeviceHeader id={id} />

          <DeviceHealth id={id} />

          <div className="grid gap-6 xl:grid-cols-[1.4fr_1fr]">
            <DeviceActivity id={id} />
            <DeviceInfo id={id} />
          </div>
        </div>
      </main>
    </div>
  );
}