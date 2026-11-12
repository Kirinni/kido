# kido

终端里的待办清单：页签、备注、鼠标点一下就勾选。**一个 exe，一个 todo.txt，删目录即卸载。**

- 左键点整行 = 勾选（不用瞄准复选框），右键 = 写备注
- 页签随你增删改名，每页各管各的
- 数据是纯文本 `todo.txt`，随时可以用编辑器批量改
- 静态链接，目标机器不需要 VC++ 运行库；不写注册表（只往用户 PATH 加一行）

## 安装

```powershell
irm https://raw.githubusercontent.com/Kirinni/kido/main/install.ps1 | iex
```

装完**完全退出并重开一次终端**（Windows Terminal / VS Code 在启动时就固定了 PATH），然后：

```powershell
kido
```

安装位置与数据文件：

```
%LOCALAPPDATA%\kido\
├─ kido.exe
└─ todo.txt
```

## 卸载

```powershell
kido --uninstall        # 移出 PATH、删程序和 todo.txt
kido --uninstall -k     # 保留 todo.txt
```

删完目录就什么都不剩了。

## 快捷键

| 键 | 作用 | 键 | 作用 |
|---|---|---|---|
| `j` `↓` | 下移 | `k` `↑` | 上移 |
| `Space` | 勾选 / 取消 | `e` `Enter` | 编辑备注 |
| `n` | 新建任务 | `r` | 重命名任务 |
| `d` | 删除任务 | `u` `Ctrl+Z` | 撤销 |
| `K` `Alt+↑` | 任务上移一位 | `J` `Alt+↓` | 任务下移一位 |
| `Ctrl+Home` | 任务移到首位 | `Ctrl+End` | 任务移到末尾 |
| `t` | 新建页签 | `T` | 重命名页签 |
| `D` | 删除页签 | `Tab` `h` `l` | 切换页签 |
| `o` | 用默认程序打开数据文件 | `?` | 帮助 |
| `q` `Esc` | 退出 | | |

鼠标：

| 操作 | 效果 |
|---|---|
| 左键点任务行任意位置 | 勾选 / 取消（不用瞄准复选框） |
| 右键点任务行 | 编辑备注 |
| 空白处双击 / 右键 | 新建任务 |
| 左键点页签 | 切换页签 |
| 右键点页签 | 删除页签（二次确认） |
| 滚轮 | 移动选中行 |
| 弹窗外任意处点击 | 关闭弹窗（备注照常保存） |

有备注的任务会在标题后面跟一段灰字摘要（`· 备注首行`），太长会以省略号截断。

## 配色

kido **只用终端的 16 色 ANSI 调色板**，不写死 RGB。所以 Windows Terminal 里换「配色方案」时，kido 会跟着变：

- 六个页签依次取 green / blue / magenta / cyan / yellow / bright cyan
- 正文用方案里的默认前景色，灰字/备注/分隔线用 bright black（槽位 8，多数方案专门留给灰字）
- 状态提示：成功绿、警告黄、错误红、普通信息青
- 选中行用**反显**而不是固定底色，任何方案下都有对比度

换配色方案：Windows Terminal → 设置 → 外观 → 配色方案 → 选一个（例如 One Half Dark / Campbell / Solarized），新开的标签页立刻生效。

> 代价是不再能自定义十六进制颜色。如果你想要"方案之外再叠一层自定义色"，可以改 [src/theme.rs](src/theme.rs) 里的 `ACCENTS`，把 `Color::Green` 换成 `Color::Rgb(r, g, b)` —— 但那样这些元素就不再跟随终端方案了。

## 数据格式

```text
>> 工作
[ ] 写周报
    #note: 周三前发，附上上周数据
[x] 回邮件

>> 生活
[ ] 买菜
```

- `>> 名字` 开一个页签
- `[ ] ` / `[x] ` 是任务，`- ` `* ` `+ ` 也认（视为未完成）
- `    #note: ` 承接上一条任务的备注，可以写多行
- `#` 开头的其它行是注释

改完存盘，kido 会在几百毫秒内自动重新载入；反过来在 kido 里操作也会立刻写回文件。
备注保存时会去掉每行首尾空白。同一份 `todo.txt` 也能被文本编辑器、Rainmeter 皮肤之类的工具读取。

## 数据文件位置

按顺序取第一个可用的：

1. 环境变量 `KIDO_HOME` 指向的目录下的 `todo.txt`
2. **exe 所在目录**的 `todo.txt`（默认情况，绿色软件）
3. `%LOCALAPPDATA%\kido\todo.txt`

也可以显式指定：`kido --data D:\notes\todo.txt`，`kido --where` 会打印当前用的是哪个文件。

## 从源码构建

需要 Rust stable，以及能够链接 MSVC 的构建工具（`cl` / `link`）。

```powershell
cargo build --release
# 产物：target\release\kido.exe
```

`.cargo/config.toml` 里开了 `+crt-static`，所以产物是静态链接的，拷到别的 Windows 机器直接能跑。

构建缓存默认在项目内的 `target\`；如果 C 盘紧张，可以把它指到别处：

```powershell
$env:CARGO_TARGET_DIR = 'D:\cache\kido-target'
cargo build --release
```

## 发布新版本

```powershell
git tag v0.1.0
git push origin v0.1.0
```

tag 推上去后 GitHub Actions 会自动构建、算 sha256、发布 Release；
`install.ps1` 拉的就是 `releases/latest/download/kido-x86_64-pc-windows-msvc.exe`。

## 许可

MIT
