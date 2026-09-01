import i18n from "i18next";
import { initReactI18next } from "react-i18next";

const en = {
  brandSub:"Codex delivery console", overview:"Overview", notifications:"Notifications", projects:"Projects", activity:"Activity", system:"System",
  overviewDesc:"Delivery health and recent Codex activity", delivery:"Push", deliveryDesc:"Bark connection and notification content", rules:"Rules", rulesDesc:"Project scope, quiet hours and privacy", systemDesc:"Hook integration and desktop preferences",
  allConnected:"Hook and Bark connected", finishSetup:"Complete the required connection", deliveryStatus:"Delivery status", deliveryReady:"Delivery ready", setupRequired:"Action required", deliveryReadyBody:"Codex events are ready to reach Bark with reliable retry.", setupRequiredBody:"Connect Bark and finish the Codex Hook review to start receiving events.", resolve:"Resolve",
  codexHook:"Codex Hook", connected:"Connected", notInstalled:"Not installed", barkConnection:"Bark", pendingQueue:"Pending", delivered:"Delivered", testSent:"Test notification sent.", retryQueued:"Retry queued.", historyCleared:"History cleared.", records:"records", noEventsHint:"New Codex events will appear here.",
  connection:"Connection", barkServerHint:"Official or self-hosted Bark endpoint", message:"Message", fixedMessage:"Fixed message", previewBody:"Codex has completed the task. Open your workstation to review it.", secretSafety:"Secrets stay in the operating system credential store.",
  projectRules:"Project rules", allProjectsEnabled:"All projects are enabled", addRulesHint:"Add a rule only when a project needs a custom alias or filter.", contentSafety:"Content safety", contentSafetyHint:"Approval alerts remain notification-only. Sensitive-looking values are removed before storage and delivery.",
  diagnosticsIdle:"Diagnostics are ready", diagnosticsHint:"Run only when you need to inspect the local integration.", resourceProfile:"Resource profile", adaptiveIdle:"Adaptive idle mode", adaptiveIdleHint:"The UI pauses refreshes while hidden. The Rust worker sleeps until a retry is due and performs no continuous animation or network polling.",
  save:"Save changes", saved:"Changes saved", test:"Send test", enabled:"Notifications enabled", hook:"Hook", bark:"Bark", queue:"Pending", sent:"Sent",
  healthy:"Ready", needsAttention:"Needs attention", recentActivity:"Recent activity", noEvents:"No notification activity yet.", notificationDelivery:"Notification delivery",
  barkServer:"Bark server", deviceKey:"Device key", encryptionKey:"Encryption key", configured:"Configured", notConfigured:"Not configured", saveSecret:"Save secret",
  titleTemplate:"Title template", group:"Group", sound:"Sound", level:"Level", bodyMode:"Message body", preview:"Preview", requestTimeout:"Request timeout", retryLimit:"Retry limit",
  encryption:"Bark encryption", algorithm:"Algorithm", permissionAlerts:"Approval request alerts", privacy:"Redact sensitive content", projectScope:"Project scope",
  allProjects:"All projects", includeProjects:"Only listed projects", excludeProjects:"All except listed projects", addProject:"Add project", path:"Path", displayName:"Display name", remove:"Remove",
  status:"Status", event:"Event", attempts:"Attempts", retry:"Retry", retryAll:"Retry failed", clear:"Clear finished", all:"All", failed:"Failed", suppressed:"Suppressed",
  quietHours:"Quiet hours", quietStart:"Starts", quietEnd:"Ends", quietAction:"Action", silent:"Deliver silently", pause:"Pause all", importantOnly:"Important only",
  appearance:"Appearance", language:"Language", theme:"Theme", systemDefault:"System default", light:"Light", dark:"Dark", autostart:"Launch at login",
  hookManagement:"Codex Hook", installRepair:"Install / repair", installing:"Installing…", uninstall:"Uninstall", uninstalling:"Uninstalling…", checkTrust:"Check trust", checkingTrust:"Checking…", diagnostics:"Diagnostics", runDiagnostics:"Run diagnostics", binary:"Hook binary",
  installedStatus:"Installed", enabledStatus:"Enabled", trustStatus:"Trust", hookInstalled:"Hook installed/repaired. Review its trust status below.", hookUninstalled:"Hook uninstalled.", hookTrusted:"Both Codex Hooks are trusted and active.", hookNeedsTrust:"Hook review is required in Codex.", trusted:"Trusted", untrusted:"Needs review", modified:"Changed — review again", unknownTrust:"Unable to verify", disabledStatus:"Disabled", hookTrustHelp:"Open Codex, enter /hooks, select PermissionRequest and Stop, then press T to trust them. Return here and click Check trust.",
  setupTitle:"Set up CodexNotify", setupBody:"Connect Bark, send a test, then install the Codex Hook. You can change every option later.", continue:"Open setup", completeSetup:"Finish setup",
  unknownError:"The operation failed", online:"Online", offline:"Not ready", eventQueued:"Queued", eventSending:"Sending", eventRetrying:"Retrying", eventSent:"Sent", eventFailed:"Failed", eventSuppressed:"Suppressed",
  errors:{ioError:"A local file operation failed.",invalidJson:"A local configuration file contains invalid JSON.",databaseError:"The notification database is unavailable.",credentialError:"The system credential store is unavailable.",invalidConfig:"One or more settings are invalid.",networkError:"Bark could not be reached.",hookConfigError:"The Codex Hook configuration could not be updated.",autostartError:"The login startup setting could not be updated.",invalidSecretKind:"The requested secret type is invalid."},
};
const zh = {
  brandSub:"Codex 通知控制台", overview:"概览", notifications:"通知", projects:"项目", activity:"活动", system:"系统",
  overviewDesc:"查看发送状态和最近的 Codex 活动", delivery:"推送", deliveryDesc:"管理 Bark 连接和通知内容", rules:"规则", rulesDesc:"项目范围、安静时段与隐私", systemDesc:"Hook 集成和桌面偏好设置",
  allConnected:"Hook 与 Bark 已连接", finishSetup:"完成必要的连接配置", deliveryStatus:"发送状态", deliveryReady:"推送已就绪", setupRequired:"需要处理", deliveryReadyBody:"Codex 事件可通过可靠重试发送到 Bark。", setupRequiredBody:"连接 Bark 并完成 Codex Hook 审核后即可接收事件。", resolve:"去处理",
  codexHook:"Codex Hook", connected:"已连接", notInstalled:"未安装", barkConnection:"Bark", pendingQueue:"待发送", delivered:"已送达", testSent:"测试通知已发送。", retryQueued:"已加入重试队列。", historyCleared:"历史记录已清理。", records:"条记录", noEventsHint:"新的 Codex 事件会显示在这里。",
  connection:"连接", barkServerHint:"支持官方或自托管 Bark 地址", message:"通知内容", fixedMessage:"固定正文", previewBody:"Codex 已完成任务，请回到工作站查看结果。", secretSafety:"密钥只保存在操作系统凭据库中。",
  projectRules:"项目规则", allProjectsEnabled:"当前允许所有项目", addRulesHint:"仅在项目需要别名或筛选时添加规则。", contentSafety:"内容安全", contentSafetyHint:"审批请求只发送提醒，不会自动允许或拒绝；疑似敏感内容会在存储和发送前脱敏。",
  diagnosticsIdle:"环境诊断已就绪", diagnosticsHint:"仅在需要排查本地集成时运行。", resourceProfile:"资源策略", adaptiveIdle:"自适应空闲模式", adaptiveIdleHint:"窗口隐藏后暂停界面刷新；Rust 后台仅在重试到期时唤醒，不持续执行动画或网络轮询。",
  save:"保存更改", saved:"已保存", test:"发送测试", enabled:"启用通知", hook:"Hook", bark:"Bark", queue:"待处理", sent:"已发送",
  healthy:"正常", needsAttention:"需要处理", recentActivity:"最近活动", noEvents:"暂无通知记录。", notificationDelivery:"通知发送",
  barkServer:"Bark 服务器", deviceKey:"设备 Key", encryptionKey:"加密密钥", configured:"已配置", notConfigured:"未配置", saveSecret:"保存密钥",
  titleTemplate:"标题模板", group:"通知分组", sound:"声音", level:"通知级别", bodyMode:"正文内容", preview:"实时预览", requestTimeout:"请求超时", retryLimit:"最大尝试次数",
  encryption:"Bark 加密", algorithm:"算法", permissionAlerts:"审批请求提醒", privacy:"自动隐藏敏感内容", projectScope:"项目范围",
  allProjects:"所有项目", includeProjects:"仅列表项目", excludeProjects:"排除列表项目", addProject:"添加项目", path:"路径", displayName:"显示名称", remove:"删除",
  status:"状态", event:"事件", attempts:"尝试次数", retry:"重试", retryAll:"重试失败项", clear:"清理已完成", all:"全部", failed:"失败", suppressed:"已抑制",
  quietHours:"安静时段", quietStart:"开始", quietEnd:"结束", quietAction:"策略", silent:"静默发送", pause:"暂停全部", importantOnly:"仅重要事件",
  appearance:"外观", language:"语言", theme:"主题", systemDefault:"跟随系统", light:"浅色", dark:"深色", autostart:"登录时启动",
  hookManagement:"Codex Hook", installRepair:"安装 / 修复", installing:"正在安装…", uninstall:"卸载", uninstalling:"正在卸载…", checkTrust:"检查信任", checkingTrust:"正在检查…", diagnostics:"环境诊断", runDiagnostics:"运行诊断", binary:"Hook 程序",
  installedStatus:"安装状态", enabledStatus:"启用状态", trustStatus:"信任状态", hookInstalled:"Hook 已安装/修复，请检查下方信任状态。", hookUninstalled:"Hook 已卸载。", hookTrusted:"两个 Codex Hook 均已信任并启用。", hookNeedsTrust:"需要在 Codex 中审核 Hook。", trusted:"已信任", untrusted:"待审核", modified:"配置已变化，需重新审核", unknownTrust:"无法确认", disabledStatus:"已禁用", hookTrustHelp:"打开 Codex，输入 /hooks，分别选择 PermissionRequest 和 Stop，然后按 T 信任。完成后回到此处点击“检查信任”。",
  setupTitle:"设置 CodexNotify", setupBody:"连接 Bark、发送测试通知，然后安装 Codex Hook。所有选项之后都可以修改。", continue:"开始设置", completeSetup:"完成设置",
  unknownError:"操作失败", online:"正常", offline:"未就绪", eventQueued:"排队中", eventSending:"发送中", eventRetrying:"等待重试", eventSent:"已发送", eventFailed:"失败", eventSuppressed:"已抑制",
  errors:{ioError:"本地文件操作失败。",invalidJson:"本地配置文件包含无效 JSON。",databaseError:"通知数据库不可用。",credentialError:"系统凭据库不可用。",invalidConfig:"一项或多项设置无效。",networkError:"无法连接 Bark。",hookConfigError:"无法更新 Codex Hook 配置。",autostartError:"无法更新登录自启动设置。",invalidSecretKind:"密钥类型无效。"},
};

const systemLanguage = navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
i18n.use(initReactI18next).init({ resources:{en:{translation:en},zh:{translation:zh}}, lng:localStorage.getItem("language")||systemLanguage, fallbackLng:"en", interpolation:{escapeValue:false} });
export default i18n;
