<script setup lang="ts">
import { computed } from "vue";
import { Activity, Gauge, Globe, Info, TestTube, X } from "@lucide/vue";
import type { ChannelDiagnostics, ChannelProxyTestResult } from "../../types";
import { formatBytes, formatSpeed, networkModeLabel } from "../../utils/format";

const props = defineProps<{
  open: boolean;
  diagnostics: ChannelDiagnostics | null;
  testResult: ChannelProxyTestResult | null;
  busy: boolean;
  runTest: () => Promise<void>;
}>();

const emit = defineEmits<{
  close: [];
}>();

const totalTraffic = computed(() => {
  const stats = props.diagnostics?.stats;
  return (stats?.upload_total ?? 0) + (stats?.download_total ?? 0);
});
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="modal-backdrop">
      <section class="modal wide-modal">
        <header class="modal-header">
          <div>
            <h3>{{ diagnostics?.channel_name || "通道诊断" }}</h3>
            <p>{{ diagnostics?.http_url }} · {{ diagnostics?.socks_url }}</p>
          </div>
          <button type="button" title="关闭" @click="emit('close')">
            <X :size="17" />
          </button>
        </header>

        <div v-if="diagnostics" class="diagnostics-grid">
          <div class="diagnostic-cell">
            <Info :size="17" />
            <span>实际网络</span>
            <strong>{{ networkModeLabel(diagnostics.network_mode) }}</strong>
          </div>
          <div class="diagnostic-cell">
            <Globe :size="17" />
            <span>生效节点</span>
            <strong>{{ diagnostics.effective_node }}</strong>
          </div>
          <div class="diagnostic-cell">
            <Activity :size="17" />
            <span>当前速度</span>
            <strong>{{ formatSpeed(diagnostics.stats.upload_speed + diagnostics.stats.download_speed) }}</strong>
          </div>
          <div class="diagnostic-cell">
            <Gauge :size="17" />
            <span>本次流量</span>
            <strong>{{ formatBytes(totalTraffic) }}</strong>
          </div>
        </div>

        <dl v-if="diagnostics" class="details-list">
          <div>
            <dt>HTTP 地址</dt>
            <dd>{{ diagnostics.http_url }}</dd>
          </div>
          <div>
            <dt>SOCKS5 地址</dt>
            <dd>{{ diagnostics.socks_url }}</dd>
          </div>
          <div>
            <dt>全局代理链路</dt>
            <dd>{{ diagnostics.global_proxy_enabled ? "开启" : "本地直连" }}</dd>
          </div>
          <div>
            <dt>通道 Switch</dt>
            <dd>{{ diagnostics.channel_proxy_enabled ? "开启" : "本地直连" }}</dd>
          </div>
          <div>
            <dt>已选节点</dt>
            <dd>{{ diagnostics.selected_node }}</dd>
          </div>
          <div>
            <dt>本地端口</dt>
            <dd>HTTP {{ diagnostics.http_port }} / SOCKS5 {{ diagnostics.socks_port }}</dd>
          </div>
          <div>
            <dt>mihomo 上游</dt>
            <dd>HTTP {{ diagnostics.mihomo_http_port }} / SOCKS {{ diagnostics.mihomo_socks_port }}</dd>
          </div>
          <div>
            <dt>核心</dt>
            <dd>{{ diagnostics.core_running ? "运行中" : "未运行" }}</dd>
          </div>
          <div>
            <dt>最近错误</dt>
            <dd>{{ diagnostics.last_error || "无" }}</dd>
          </div>
        </dl>

        <section v-if="testResult" class="test-result" :class="{ failed: !testResult.success }">
          <strong>{{ testResult.success ? "测试成功" : "测试失败" }}</strong>
          <span>
            {{ networkModeLabel(testResult.network_mode) }} · {{ testResult.elapsed_ms }} ms
            <template v-if="testResult.error"> · {{ testResult.error }}</template>
          </span>
          <div
            v-for="entry in testResult.entries"
            :key="entry.protocol"
            class="test-protocol"
            :class="{ failed: !entry.success }"
          >
            <strong>{{ entry.protocol }} · {{ entry.ip || entry.error || "-" }}</strong>
            <span>{{ entry.proxy_url }} · {{ entry.elapsed_ms }} ms</span>
            <ul v-if="entry.tests.length > 0" class="test-targets">
              <li
                v-for="item in entry.tests"
                :key="`${entry.protocol}-${item.name}`"
                :class="{ failed: !item.success }"
              >
                <strong>{{ item.name }}</strong>
                <span>
                  {{ item.success ? "可达" : item.error || "失败" }}
                  <template v-if="item.status"> · HTTP {{ item.status }}</template>
                  · {{ item.elapsed_ms }} ms
                </span>
              </li>
            </ul>
          </div>
        </section>

        <section v-if="diagnostics" class="targets compact-targets">
          <div class="targets-title">
            <span>最近访问目标</span>
            <strong>{{ diagnostics.stats.active_connections }} 活动</strong>
          </div>
          <ul v-if="diagnostics.stats.recent_targets.length > 0">
            <li v-for="conn in diagnostics.stats.recent_targets.slice(0, 8)" :key="conn.id">
              <span class="target">{{ conn.target }}</span>
              <span>{{ conn.method }}</span>
              <span>{{ formatBytes(conn.upload) }} ↑</span>
              <span>{{ formatBytes(conn.download) }} ↓</span>
            </li>
          </ul>
          <p v-else>暂无连接</p>
        </section>

        <footer class="modal-actions">
          <button type="button" @click="emit('close')">关闭</button>
          <button class="primary" type="button" :disabled="busy" @click="runTest">
            <TestTube :size="16" />
            测试连接
          </button>
        </footer>
      </section>
    </div>
  </Teleport>
</template>
