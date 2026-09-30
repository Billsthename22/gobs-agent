
import Sidebar from "@/components/layout/Sidebar";
import Topbar from "@/components/layout/Topbar";
import RemoteMonitor from "@/components/monitor/RemoteMonitor";

interface MonitorPageProps {
  params: Promise<{
    id: string;
  }>;
}

export default async function MonitorPage({
  params,
}: MonitorPageProps) {
  const { id } = await params;

  return (
    <div className="min-h-screen bg-zinc-50">
      <Sidebar />
      <Topbar />

      <main className="ml-64 pt-20">
        <div className="space-y-6 p-8">
          <RemoteMonitor deviceId={id} />
        </div>
      </main>
    </div>
  );
}
