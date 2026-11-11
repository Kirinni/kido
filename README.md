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
| `t` | 新建页签 | `T` | 重命名页签 |
| `D` | 删除页签 | `Tab` `h` `l` | 切换页签 |
| `o` | 用默认程序打开数据文件 | `?` | 帮助 |
| `q` `Esc` | 退出 | | |

鼠标：**左键点行 = 勾选**，**右键 = 备注**，点页签 = 切换，滚轮 = 移动选中行。

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
