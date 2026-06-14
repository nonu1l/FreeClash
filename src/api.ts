import { invoke } from "@tauri-apps/api/core";
import type {
  AppSnapshot,
  ChannelDiagnostics,
  ChannelInput,
  ChannelProxyTestResult,
  ChannelStats,
  DelayResult,
  NodeInfo,
  ProxyChannel,
  Subscription,
  SubscriptionInput,
} from "./types";

type ApiTransport = "tauri" | "http";

interface HttpApiResponse<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

const COMMANDS = {
  getState: "get_state",
  setSubscription: "set_subscription",
  createSubscription: "create_subscription",
  updateSubscription: "update_subscription",
  deleteSubscription: "delete_subscription",
  refreshSubscription: "refresh_subscription",
  refreshNodes: "refresh_nodes",
  setGlobalProxyEnabled: "set_global_proxy_enabled",
  setHttpApiConfig: "set_http_api_config",
  createChannel: "create_channel",
  updateChannel: "update_channel",
  deleteChannel: "delete_channel",
  setChannelEnabled: "set_channel_enabled",
  duplicateChannel: "duplicate_channel",
  setChannelNode: "set_channel_node",
  getChannelStats: "get_channel_stats",
  listChannelConnections: "list_channel_connections",
  diagnoseChannel: "diagnose_channel",
  testChannelProxy: "test_channel_proxy",
  restartCore: "restart_core",
  testNodeDelay: "test_node_delay",
} as const;

export interface FreeClashApi {
  getState(): Promise<AppSnapshot>;
  setSubscription(url: string | null): Promise<void>;
  createSubscription(input: SubscriptionInput): Promise<Subscription>;
  updateSubscription(subscriptionId: string, input: SubscriptionInput): Promise<Subscription>;
  deleteSubscription(subscriptionId: string): Promise<void>;
  refreshSubscription(subscriptionId: string): Promise<NodeInfo[]>;
  refreshNodes(): Promise<NodeInfo[]>;
  setGlobalProxyEnabled(enabled: boolean): Promise<void>;
  setHttpApiConfig(enabled: boolean, port: number): Promise<void>;
  createChannel(input: ChannelInput): Promise<ProxyChannel>;
  updateChannel(channelId: string, input: ChannelInput): Promise<ProxyChannel>;
  deleteChannel(channelId: string): Promise<void>;
  setChannelEnabled(channelId: string, enabled: boolean): Promise<ProxyChannel>;
  duplicateChannel(channelId: string): Promise<ProxyChannel>;
  setChannelNode(channelId: string, node: string): Promise<void>;
  getChannelStats(): Promise<ChannelStats[]>;
  listChannelConnections(channelId: string): Promise<AppSnapshot["stats"][number]["recent_targets"]>;
  diagnoseChannel(channelId: string): Promise<ChannelDiagnostics>;
  testChannelProxy(channelId: string): Promise<ChannelProxyTestResult>;
  restartCore(): Promise<void>;
  testNodeDelay(node: string): Promise<DelayResult>;
}

export function createFreeClashApi(transport: ApiTransport = defaultTransport()): FreeClashApi {
  const call = createCaller(transport);
  return {
    getState: () => call(COMMANDS.getState),
    setSubscription: (url) => call(COMMANDS.setSubscription, { url }),
    createSubscription: (input) => call(COMMANDS.createSubscription, { input }),
    updateSubscription: (subscriptionId, input) =>
      call(COMMANDS.updateSubscription, { subscriptionId, input }),
    deleteSubscription: (subscriptionId) =>
      call(COMMANDS.deleteSubscription, { subscriptionId }),
    refreshSubscription: (subscriptionId) =>
      call(COMMANDS.refreshSubscription, { subscriptionId }),
    refreshNodes: () => call(COMMANDS.refreshNodes),
    setGlobalProxyEnabled: (enabled) => call(COMMANDS.setGlobalProxyEnabled, { enabled }),
    setHttpApiConfig: (enabled, port) => call(COMMANDS.setHttpApiConfig, { enabled, port }),
    createChannel: (input) => call(COMMANDS.createChannel, { input }),
    updateChannel: (channelId, input) => call(COMMANDS.updateChannel, { channelId, input }),
    deleteChannel: (channelId) => call(COMMANDS.deleteChannel, { channelId }),
    setChannelEnabled: (channelId, enabled) =>
      call(COMMANDS.setChannelEnabled, { channelId, enabled }),
    duplicateChannel: (channelId) => call(COMMANDS.duplicateChannel, { channelId }),
    setChannelNode: (channelId, node) => call(COMMANDS.setChannelNode, { channelId, node }),
    getChannelStats: () => call(COMMANDS.getChannelStats),
    listChannelConnections: (channelId) =>
      call(COMMANDS.listChannelConnections, { channelId }),
    diagnoseChannel: (channelId) => call(COMMANDS.diagnoseChannel, { channelId }),
    testChannelProxy: (channelId) => call(COMMANDS.testChannelProxy, { channelId }),
    restartCore: () => call(COMMANDS.restartCore),
    testNodeDelay: (node) => call(COMMANDS.testNodeDelay, { node }),
  };
}

export const freeClashApi = createFreeClashApi();

function createCaller(transport: ApiTransport) {
  return async function call<T>(command: string, payload: Record<string, unknown> = {}): Promise<T> {
    if (transport === "http") {
      return httpCall<T>(command, payload);
    }
    return invoke<T>(command, payload);
  };
}

async function httpCall<T>(command: string, payload: Record<string, unknown>): Promise<T> {
  const baseUrl = getHttpBaseUrl();
  const token = getHttpToken();
  const response = await fetch(`${baseUrl}/api/invoke/${encodeURIComponent(command)}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify({ payload }),
  });
  const body = (await response.json()) as HttpApiResponse<T>;
  if (!response.ok || !body.ok) {
    throw new Error(body.error || `HTTP API returned ${response.status}`);
  }
  return body.data as T;
}

function defaultTransport(): ApiTransport {
  const queryTransport = getQueryParam("freeclashApiTransport");
  if (queryTransport === "http" || queryTransport === "tauri") return queryTransport;
  const stored = localStorage.getItem("freeclashApiTransport");
  if (stored === "http" || stored === "tauri") return stored;
  return "__TAURI_INTERNALS__" in window ? "tauri" : "http";
}

function getHttpBaseUrl() {
  const queryBaseUrl = getQueryParam("freeclashApiBaseUrl");
  if (queryBaseUrl) return queryBaseUrl;
  return localStorage.getItem("freeclashApiBaseUrl") || "http://127.0.0.1:19290";
}

function getHttpToken() {
  const queryToken = getQueryParam("freeclashApiToken");
  if (queryToken) return queryToken;
  return localStorage.getItem("freeclashApiToken") || "";
}

function getQueryParam(name: string) {
  return new URLSearchParams(window.location.search).get(name);
}
