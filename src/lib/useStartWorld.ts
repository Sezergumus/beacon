import { useServerStore } from "../stores/useServerStore";
import { useNetworkStore } from "../stores/useNetworkStore";

/** Opens the selected world and, if nobody is hosting yet, a room so friends can join. */
export const useStartWorld = () => {
  const startServer = useServerStore((s) => s.startServer);
  const { network, startHostingRoom } = useNetworkStore();

  return async () => {
    await startServer();
    if (network.role === "idle") {
      try {
        await startHostingRoom();
      } catch {
        // the room can also be opened by hand from the Home page
      }
    }
  };
};
