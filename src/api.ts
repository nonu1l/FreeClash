import { invoke } from "@tauri-apps/api/core";
import type {
  AppSnapshot,
  DelayResult,
  NodeInfo,
  PinnedNode,
  Subscription,
  SubscriptionInput,
} from "./types";

const COMMANDS = {
  getState: "get_state",
  setSubscription: "set_subscription",
  refreshSubscription: "refresh_subscription",
  refreshNodes: "refresh_nodes",
  pinNode: "pin_node",
  unpinNode: "unpin_node",
  updatePinPort: "update_pin_port",
  testNodeDelay: "test_node_delay",
  testAllNodeDelays: "test_all_node_delays",
} as const;

function call<T>(command: string, payload?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, payload);
}

export const freeClashApi = {
  getState: () => call<AppSnapshot>(COMMANDS.getState),
  setSubscription: (input: SubscriptionInput) =>
    call<Subscription>(COMMANDS.setSubscription, { input }),
  refreshSubscription: () => call<NodeInfo[]>(COMMANDS.refreshSubscription),
  refreshNodes: () => call<NodeInfo[]>(COMMANDS.refreshNodes),
  pinNode: (nodeName: string) => call<PinnedNode>(COMMANDS.pinNode, { nodeName }),
  unpinNode: (nodeName: string) => call<void>(COMMANDS.unpinNode, { nodeName }),
  updatePinPort: (nodeName: string, port: number) =>
    call<PinnedNode>(COMMANDS.updatePinPort, { nodeName, port }),
  testNodeDelay: (nodeName: string) =>
    call<DelayResult>(COMMANDS.testNodeDelay, { nodeName }),
  testAllNodeDelays: () => call<DelayResult[]>(COMMANDS.testAllNodeDelays),
};
