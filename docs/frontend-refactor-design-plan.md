# FreeClash 前端商业级重构方案

## 目标

把 FreeClash 从“功能可用的管理页”重构成一个 Windows 桌面端的应用级代理工作台。整体感觉应该像成熟的网络工具、开发者工具或运维控制台：信息密度高、状态清楚、操作路径短、视觉克制但有质感。

本方案只描述前端重构，不改后端业务接口。

## 调研来源

- Atlassian Design System: color roles、semantic color、design token 思路  
  https://atlassian.design/foundations/color
- Fluent 2: neutral/shared/brand palette、布局空间关系、响应式、系统字体  
  https://fluent2.microsoft.design/color  
  https://fluent2.microsoft.design/layout  
  https://fluent2.microsoft.design/typography  
  https://fluent2.microsoft.design/accessibility
- GitHub Primer: focused layout、responsive viewport ranges、layout regions  
  https://primer.style/product/getting-started/foundations/layout/

## 设计原则

1. 操作优先  
   首页就是规则和订阅管理，不做营销式首页、不做大 hero、不做解释性大卡片。

2. 安静但不平淡  
   用浅色中性底、清晰边界、稳定排版建立专业感。品牌色只用于主操作、选中态、关键状态，不铺满大面积背景。

3. 信息密度合理  
   桌面端优先展示表格、状态、诊断结果、流量数据。不要把每条规则做成巨大卡片。

4. 真实产品感  
   避免 AI 常见视觉套路：大面积紫蓝渐变、玻璃拟态、漂浮光球、夸张阴影、过圆卡片、无意义插画。

5. 状态永远可见  
   核心、全局链路、规则启用数、速度、流量、最近错误要在固定区域持续可见。

6. 错误可诊断  
   任何失败状态都应该能进一步看到原因、端口、节点、最近目标、测试结果。

## 推荐视觉方向

名称：`Calm Ops Console`

关键词：

- 网络工具
- 本地桌面软件
- 轻量运维台
- 精准、克制、可靠

不要做成：

- SaaS 营销后台
- AI 聊天产品
- 游戏控制面板
- 暗黑炫彩仪表盘

## 配色方案

这套配色以冷中性灰为主体，用沉稳青绿色做品牌色，辅以蓝色、琥珀色、红色、绿色表达状态。它适合代理工具：不花哨，但能清楚表达链路、风险和成功。

### CSS Token

建议在 `src/style.css` 顶部重建 token：

```css
:root {
  --fc-bg: #f4f7f9;
  --fc-bg-subtle: #edf2f6;
  --fc-surface: #ffffff;
  --fc-surface-muted: #f8fafc;
  --fc-surface-raised: #ffffff;

  --fc-border: #d8e0e8;
  --fc-border-strong: #bcc8d4;
  --fc-divider: #e6edf3;

  --fc-text: #172033;
  --fc-text-muted: #64748b;
  --fc-text-subtle: #8792a2;
  --fc-text-inverse: #ffffff;

  --fc-brand: #176b63;
  --fc-brand-hover: #125950;
  --fc-brand-active: #0d453f;
  --fc-brand-soft: #e4f3f0;
  --fc-brand-border: #a8d6cf;

  --fc-info: #2563eb;
  --fc-info-soft: #eaf1ff;
  --fc-success: #16825d;
  --fc-success-soft: #e4f6ee;
  --fc-warning: #a15c07;
  --fc-warning-soft: #fff3d6;
  --fc-danger: #b42318;
  --fc-danger-soft: #fff0ef;

  --fc-shadow-sm: 0 1px 2px rgba(16, 24, 40, 0.06);
  --fc-shadow-md: 0 8px 24px rgba(16, 24, 40, 0.10);

  --fc-radius-xs: 4px;
  --fc-radius-sm: 6px;
  --fc-radius-md: 8px;

  --fc-space-1: 4px;
  --fc-space-2: 8px;
  --fc-space-3: 12px;
  --fc-space-4: 16px;
  --fc-space-5: 20px;
  --fc-space-6: 24px;
}
```

### 使用规则

- 背景只用 `--fc-bg` 和 `--fc-bg-subtle`，不要叠加渐变。
- 页面区域、表格、弹窗使用 `--fc-surface`。
- 主按钮、选中菜单、开关开启态使用 `--fc-brand`。
- 成功、警告、错误必须同时使用颜色和文字/icon，不只靠颜色。
- 不使用纯黑文字，主文字用 `#172033`。
- 卡片圆角不超过 8px。
- 阴影只用于弹窗、浮层、置顶区域，不用于每个页面区块。

## 字体与排版

字体栈：

```css
font-family:
  "Segoe UI",
  "Microsoft YaHei UI",
  "Microsoft YaHei",
  system-ui,
  -apple-system,
  BlinkMacSystemFont,
  sans-serif;
```

字号建议：

- 页面标题：22px / 30px，600
- 分区标题：16px / 24px，600
- 表格主文本：14px / 20px，600 或 400
- 辅助文本：12px / 16px，400
- 数字指标：18px / 24px，650

注意：

- 不用 viewport 动态缩放字体。
- 字距保持默认，不使用负 letter-spacing。
- 表格里的长路径、长节点名必须 `text-overflow: ellipsis`。

## 布局重构

### 桌面端主结构

推荐结构：

```text
┌──────────────────────────────────────────────────────────────┐
│ Top Bar: FreeClash / 当前链路 / 全局操作 / 搜索               │
├──────────────┬───────────────────────────────────────────────┤
│ Left Rail    │ Content Header                                │
│ - 规则       │ - 当前页面标题                                │
│ - 订阅       │ - 页面级操作                                  │
│ - 日志       ├───────────────────────────────────────────────┤
│ - 设置       │ Main Content                                  │
├──────────────┤ - 规则表格 / 订阅列表 / 诊断详情              │
│ Status Pane  │                                               │
│ - 核心       │                                               │
│ - 速度       │                                               │
│ - 流量       │                                               │
│ - HTTP API   │                                               │
└──────────────┴───────────────────────────────────────────────┘
```

实现建议：

- 保留左侧状态和菜单，但减少“卡片堆叠感”。
- 左侧宽度建议 `280px`，内容区最小宽度 `0`，避免表格撑破。
- Top Bar 高度 `52px`，放品牌、当前网络模式、全局搜索或快速操作。
- 左侧状态区固定，菜单和状态之间用分组标题，不用大块装饰。

### 响应式结构

- `>= 1280px`：左侧状态菜单 + 主内容表格。
- `960px - 1279px`：左侧缩窄，状态指标改为两列小指标。
- `< 960px`：左侧变为顶部横向导航，状态进入可展开面板。
- `< 720px`：规则列表从表格切换为紧凑列表，每条只显示规则名、节点、状态、主操作。

## 信息架构

### 一级菜单

1. 代理规则
2. 订阅
3. 连接记录
4. 设置

其中 v1 可以先实现前两项，第三项和第四项可只预留入口或做轻量页面。

### Top Bar

显示：

- FreeClash 标识
- 全局代理开关：节点模式 / 本地直连
- 核心状态：运行中 / 未运行 / 异常
- 主操作：重启核心

不要显示：

- 大段说明文字
- Core 路径
- Config 路径

### 左侧 Status Pane

展示：

- 核心状态
- 规则数量：启用 / 总数
- 当前速度：上行、下行、总速
- 本次流量
- HTTP API 小型调试区

HTTP API 调试区建议折叠，默认只显示开关和端口，token 复制放在展开后。

## 页面设计

### 代理规则页

核心是表格，不是卡片。

表格列：

1. 规则名  
   显示规则名、网络模式 badge、最近测试摘要。

2. 软件  
   显示 exe 名，下一行显示路径。

3. 节点  
   显示当前选择节点，下一行显示来源订阅或 DIRECT。

4. 实时速度  
   总速为主，上下行为辅。

5. 流量  
   总量为主，上下行为辅。

6. Switch  
   开启为代理节点，关闭为本地直连，但仍注入本地代理端口。

7. 操作  
   启动、停止、测速、连通性测试、诊断、编辑、复制、删除。

交互：

- 点击规则行选中，右侧打开详情抽屉。
- 诊断建议使用右侧抽屉或弹窗。桌面端优先抽屉，移动端用全屏弹窗。
- 删除必须二次确认。
- 快速测试完成后在规则名下显示：`IP 91.x.x.x · Google OK · OpenAI OK`。

### 规则编辑弹窗

字段顺序：

1. 规则名
2. 软件路径
3. 启动参数
4. 节点选择
5. 启用 Switch

节点选择：

- 顶部搜索框。
- 按订阅来源分组。
- 筛选：全部、可用、未测速、高延迟。
- 每个节点显示：名称、类型、延迟、来源。

不要出现工作目录字段。

### 订阅页

结构：

- 顶部工具栏：新增订阅、刷新全部。
- 列表：订阅名、URL、启用状态、节点数、最近刷新状态、操作。
- 点击订阅后右侧或下方显示该订阅节点。

节点展示：

- 使用紧凑表格或分组列表。
- 支持搜索、地区快速筛选。
- 延迟用数字 + 状态颜色，不用大 badge 堆满。

### 连接记录页

可作为下一阶段实现。

内容：

- 按规则筛选。
- 最近目标：域名、方法、上传、下载、时间、是否活动。
- 支持清空记录。

### 设置页

内容：

- HTTP API 开关、端口、token 复制。
- Core 路径只在高级设置中显示。
- 测试 URL 列表：出口 IP、Google、OpenAI。
- 端口范围。

## 组件重构建议

建议目录：

```text
src/
  components/
    layout/
      AppShell.vue
      TopBar.vue
      SideNav.vue
      StatusPane.vue
    common/
      Button.vue
      IconButton.vue
      Switch.vue
      Badge.vue
      Dialog.vue
      Drawer.vue
      Toolbar.vue
      EmptyState.vue
      Metric.vue
    rules/
      RulesTable.vue
      RuleRow.vue
      RuleDialog.vue
      RuleDiagnosticsPanel.vue
      NodePicker.vue
    subscriptions/
      SubscriptionList.vue
      SubscriptionDialog.vue
      SubscriptionNodes.vue
  views/
    RulesView.vue
    SubscriptionsView.vue
    ConnectionsView.vue
    SettingsView.vue
```

`App.vue` 只保留：

- active view
- 轮询状态
- 全局错误
- command 分发

样式建议：

- 先保留全局 CSS，不急着引入 UI 库。
- 建立 token + utility class + 组件局部样式。
- 避免每个组件重新定义颜色。

## 关键组件规格

### Button

类型：

- `primary`
- `secondary`
- `ghost`
- `danger`

规则：

- 图标按钮必须有 `title`。
- 主要操作每个页面最多一个 primary。
- 危险操作不使用大面积红底，默认用 ghost danger，确认弹窗中再使用 danger。

### Switch

状态文本必须明确：

- 全局：`节点模式` / `本地直连`
- 规则：`代理节点` / `本地直连`
- 订阅：`启用` / `停用`

### Badge

类型：

- Proxy
- Direct
- Running
- Stopped
- Error
- Testing

不要用太多不同颜色。Badge 以浅底色为主。

### Table

要求：

- 表头固定风格。
- 行 hover 有浅背景。
- 选中行有左侧 3px brand border。
- 操作按钮只在 hover 或选中时增强显示，但基础操作仍可见。
- 长文本截断，hover title 显示完整值。

## 规则诊断体验

诊断面板字段：

- 全局代理状态
- 规则 Switch 状态
- 实际网络模式
- 本地计量端口
- mihomo 上游端口
- 选中节点
- 生效节点
- 最近错误
- 最近访问目标
- 连通性测试结果

连通性测试结果建议：

```text
出口 IP    OK    91.199.84.47    916ms
Google     OK    204             420ms
OpenAI     OK    401             680ms
```

说明：

- OpenAI 返回 401/403 也视为链路可达。
- 只有 DNS 失败、连接失败、超时、代理失败才视为失败。

## 避免 AI 感的设计约束

必须避免：

- 紫蓝渐变背景
- 毛玻璃卡片
- 装饰光球、bokeh、抽象网格背景
- 大圆角卡片嵌套卡片
- 过度阴影
- 大面积插画
- “智能”“下一代”“极致体验”等空洞宣传语
- 所有区域都做成卡片

应该使用：

- 清楚的表格
- 稳定的分隔线
- 一致的间距
- 实际状态和数据
- 低调 icon
- 真实可操作的 toolbar

## 分阶段执行计划

### Phase 1: Design Token 与基础样式

- 重写 `src/style.css` 顶部 token。
- 替换硬编码颜色为 token。
- 统一按钮、输入框、select、textarea、switch。
- 删除渐变背景和过重阴影。
- 确认 `npm run build` 通过。

### Phase 2: App Shell

- 新增 `components/layout/AppShell.vue`。
- 新增 `TopBar.vue`、`SideNav.vue`、`StatusPane.vue`。
- `App.vue` 改为使用 AppShell。
- 保持现有 command 调用逻辑不变。
- 确认左侧不再显示 Core 路径、Config 路径。

### Phase 3: 规则页重构

- 拆出 `RulesTable.vue`、`RuleRow.vue`。
- 表格替代大卡片。
- 快速操作按钮保持 icon + title。
- 快速测试结果在行内显示摘要。
- 诊断入口保留。

### Phase 4: 规则诊断面板

- 将 `RuleDiagnosticsDialog.vue` 改为更像详情面板。
- 展示连通性测试明细。
- 最近目标列表固定高度，可滚动。
- 错误状态使用浅红背景 + 明确错误文案。

### Phase 5: 订阅页重构

- 订阅列表紧凑化。
- 节点查看区域表格化。
- 搜索、筛选、分组样式统一。

### Phase 6: 响应式与可访问性

- 断点：720、960、1280。
- 320px 宽度不丢功能。
- 200% 文本缩放不裁切主要操作。
- 所有 icon button 都有 title。
- 表单 label 必须可见。
- 键盘 Tab 顺序符合视觉顺序。

### Phase 7: 视觉验收

- 用浏览器截图检查 1280x720、1440x900、1920x1080。
- 检查文字不溢出、不重叠。
- 检查按钮 hover、disabled、focus 状态。
- 检查规则很多时表格滚动表现。

## 验收标准

- `npm run build` 通过。
- `rg "#[0-9a-fA-F]{3,8}" src/style.css src/components src/views` 后，除 token 区外不应大量出现硬编码颜色。
- 第一屏能完成：查看状态、切换全局代理、进入规则、启动软件、测试连通性。
- 规则列表信息完整但不拥挤。
- 订阅页能清楚看到订阅与节点来源关系。
- 诊断面板能解释“为什么当前走代理/为什么直连/哪里失败”。
- 视觉上不出现 AI 模板感。

## 建议 commit message

```text
重构前端视觉方案
```
