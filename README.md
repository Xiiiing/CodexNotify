# Codex Notify

让 Codex 的任务完成、等待输入和权限请求，通过 Bark 到达 iPhone 与 Apple Watch。

这是一个可移动的 Windows Python 项目，不需要打包 EXE，也不需要在每个代码项目中放置 `.codex`。窗口关闭后默认驻留系统托盘；右击托盘图标可以打开、启停通知或彻底退出。

## 主要功能

- `Stop` 通知：任务回合结束、等待输入或执行异常。
- `PermissionRequest` 通知：Codex 等待本地命令等权限时立即提醒；只提醒，不自动批准或拒绝。
- 项目名优先：当前工作目录或自定义显示名作为 Bark 主标题。
- 发送队列：事件先写入本地 SQLite，再发送；网络失败按退避策略自动重试。
- 去重：同一个 Hook 事件不会因重复回调而重复推送。
- 通知历史：查看已发送、等待重试、失败和被安静策略抑制的事件，并可手动重试失败项。
- 隐私保护：可自动隐藏疑似 Token、密码、邮箱和带查询参数的链接。
- 安静时段：支持静默发送、仅重要事件或暂停全部通知，并支持跨午夜时段。
- Bark 高级参数：声音、分组、通知级别、图标、点击链接、超时和重试次数。
- 可选端到端加密：AES-128-CBC 或 AES-256-CBC，每条消息使用随机 IV。
- 首次设置向导、实时通知预览、环境诊断、Hook 安装/卸载、Windows 登录自启动。
- 单实例保护：重复双击不会产生多个托盘进程，并会尝试唤回已有窗口。
- Bark Key 和可选 AES 密钥都使用 Windows DPAPI 加密保存。

## 数据流

```text
Codex Stop / PermissionRequest Hook
                ↓ JSON 标准输入
           hook_runner.py
                ↓ 去重、脱敏、安静策略
       data/events.sqlite 本地队列
                ↓ HTTPS POST / 失败重试
          Bark → iPhone → Apple Watch
```

官方参考：[Codex Hooks](https://learn.chatgpt.com/docs/hooks)、[Bark 使用说明](https://github.com/Finb/Bark/blob/master/docs/en-us/tutorial.md)、[Bark 推送加密](https://github.com/Finb/Bark/blob/master/docs/en-us/encryption.md)。

## 系统与启动

当前项目位于：

```text
C:\Users\31908\Desktop\画图\CodexBarkNotifier
```

程序固定使用你的非 base Conda 环境：

```text
E:\conda\envs\clam_latest\python.exe
```

双击 `start.bat` 即可启动。脚本只检查并向 `clam_latest` 安装 `requirements.txt` 中缺少的包，不使用 base。正常启动后可以直接关闭主窗口，程序会继续在系统托盘运行。要彻底关闭，请右击托盘图标并选择退出。

若选择“登录 Windows 后自动在托盘启动”，程序只会创建当前用户的启动脚本：

```text
%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\CodexBarkNotifier.cmd
```

自启动默认关闭，可以随时在“高级设置”中撤销。

## 首次配置

首次打开会显示五步设置向导：

1. 填写 Bark Server，默认是 `https://api.day.app`。
2. 填写设备 Key；也可以直接粘贴 `https://api.day.app/你的Key`。
3. 发送测试通知，确认电脑到 Bark、iPhone和手表的链路。
4. 选择项目通知范围，或保持“所有项目”。
5. 安装 Hook，并按 Codex 的提示审查和信任本地命令。

设置页右侧会实时展示标题、状态和正文。推荐标题模板保留 `{project}`，这样每条通知的主标题就是清晰的项目名称。

## 页面说明

- `概览`：当前项目、Hook 真实调用状态、Bark 配置、队列和发送统计。
- `项目管理`：所有项目、仅选择项目、排除选择项目；每个目录可设置显示名称或单独禁用。
- `通知设置`：Bark Server、设备 Key、标题模板、正文模式、分组、级别和声音，并实时预览。
- `通知历史`：显示最近事件状态，可重试失败项或清理已完成历史。
- `高级设置`：审批提醒、脱敏、安静时段、图标、点击链接、超时、重试、AES 加密和开机自启动。
- `环境诊断`：检查 Key、Conda、Hook、功能开关和真实回调；可安装、修复、卸载或重新审核 Hook。

## `.codex` 到底修改什么

本程序只会修改用户级文件：

```text
C:\Users\31908\.codex\hooks.json
```

每次修改前都会在同一目录创建备份，例如：

```text
C:\Users\31908\.codex\hooks.json.bak.20260901-173824
```

安装器会保留现有的其他 Hook，只替换带 `--codex-bark-notifier` 标记的本程序处理器。目前安装两个异步命令 Hook：

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"E:\\conda\\envs\\clam_latest\\python.exe\" \"C:\\Users\\31908\\Desktop\\画图\\CodexBarkNotifier\\hook_runner.py\" --codex-bark-notifier",
            "timeout": 30,
            "async": true,
            "statusMessage": "正在记录 Codex 通知"
          }
        ]
      }
    ],
    "PermissionRequest": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"E:\\conda\\envs\\clam_latest\\python.exe\" \"C:\\Users\\31908\\Desktop\\画图\\CodexBarkNotifier\\hook_runner.py\" --codex-bark-notifier",
            "timeout": 30,
            "async": true,
            "statusMessage": "正在记录 Codex 通知"
          }
        ]
      }
    ]
  }
}
```

程序不会把以下内容写进 `.codex`：

- Bark Key 或 AES 密钥。
- Python 源码、SQLite 历史或日志。
- 项目范围、标题模板和其他应用设置。
- 任何项目内 `.codex` 文件。

程序不会修改 `config.toml` 的 `notify` 或自动开启 Hook 功能。如果诊断发现 `[features] hooks = false`，需要你确认后自行改为 `true` 或删除该禁用项。

## 为什么必须重新信任 Hook

Codex 会对非托管 Hook 进行审查。新增事件、修改命令或 Hook 文件哈希变化后，它会再次显示类似：

```text
发现新的 Hook
Review hooks
Trust this hook
允许运行本地命令
```

这是预期行为。请核对命令只指向上面列出的 `clam_latest\python.exe` 和本项目 `hook_runner.py`，再选择信任。程序不会自动信任 Hook，也不会自动批准任何权限请求。

如果没有安装独立 CLI，可在“环境诊断”点击“审核 / 信任 Hook”，或双击 `review_hooks.bat`。脚本会寻找 Codex 桌面版内置的 `codex.exe`，不要求系统 `PATH` 中存在 `codex` 命令。

## 本地数据与隐私

运行数据全部留在项目目录：

```text
data/settings.json          非敏感设置
data/secret.dat             DPAPI 加密的 Bark Key
data/encryption_secret.dat  DPAPI 加密的可选 AES 密钥
data/events.sqlite          通知队列与历史
data/hook_health.json       最近一次真实 Hook 回调
logs/notifier.log           轮转日志，不记录密钥
```

这些运行数据都已加入 `.gitignore`。DPAPI 文件通常只能由创建它们的同一个 Windows 用户解密；迁移到其他电脑或账户时请重新输入密钥。

通知正文模式包括固定短句、200 字、500 字和完整正文。敏感项目建议使用固定短句或最短模式，并保持“自动隐藏敏感内容”开启。

## Bark AES 加密

AES 默认关闭。开启前必须在 Bark App 的“推送加密”中选择同样的算法并设置相同密钥：

- AES-128-CBC：16 个 ASCII 字符。
- AES-256-CBC：32 个 ASCII 字符。

可以在高级设置中生成密钥。若手机端算法或密钥不一致，Bark 可能收到消息但无法正确解密内容。先保存两端配置，再使用“发送测试通知”验证。

## 测试与故障排查

测试 Bark 本身：在界面点击“发送测试通知”。

模拟完整 Hook：

```powershell
cd "C:\Users\31908\Desktop\画图\CodexBarkNotifier"
& "E:\conda\envs\clam_latest\python.exe" test_hook.py
```

正常输出为 `stdout: {}` 和退出码 0。模拟测试不会冒充 Hook 已被 Codex 信任；诊断页只有收到真实 Codex 回调后才显示“真实调用已验证”。

自动化测试：

```powershell
& "E:\conda\envs\clam_latest\python.exe" -m unittest discover -v
& "E:\conda\envs\clam_latest\python.exe" -m compileall -q .
```

若测试通知成功但真实任务不通知，请依次检查：

1. 是否重启 Codex 并重新信任了最新 Hook。
2. 环境诊断是否显示两个事件：`Stop` 和 `PermissionRequest`。
3. `config.toml` 是否明确设置了 `hooks = false`。
4. 当前项目是否被项目范围排除或单独禁用。
5. 高级设置中的安静时段是否为“暂停全部”。
6. 通知历史中是否显示“等待重试”或“失败”，并查看 `logs/notifier.log`。

## 卸载、移动与恢复

卸载时在“环境诊断”点击“卸载 Hook”。程序仅删除带自身标记的两个处理器，其他 Hook 保持不变，并再次备份 `hooks.json`。

移动整个项目目录后，双击新位置的 `start.bat`，再点击“安装 / 修复 Hook”；Hook 使用绝对路径，所以必须更新。恢复配置时，可先退出 Codex，再将所需的 `hooks.json.bak.时间戳` 复制回 `hooks.json`。

## 安全边界

- 权限请求只发送提醒，不作允许或拒绝决定。
- 不自动信任 Hook，不绕过 Codex 审核。
- 修改 `hooks.json` 前创建备份，无效 JSON 时拒绝覆盖。
- 发送采用异步 Hook；网络异常进入本地重试队列，不阻塞 Codex 完成本轮。
- Hook 始终向 Codex 输出合法空对象，不让通知故障破坏正常任务。
