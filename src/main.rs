use std::process::ExitCode;

use kido::cli::{self, Command};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match cli::parse(args) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("kido: {message}");
            eprintln!();
            print_help();
            return ExitCode::from(2);
        }
    };

    match command {
        Command::Help => {
            print_help();
            ExitCode::SUCCESS
        }
        Command::Version => {
            println!("kido {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::Where { data } => {
            let path = data.unwrap_or_else(kido::store::default_data_path);
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Command::Install => report(kido::install_to_path()),
        Command::Uninstall { keep_data } => report(kido::uninstall(keep_data)),
        Command::Run { data } => match kido::run(data) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("kido: {err}");
                ExitCode::FAILURE
            }
        },
    }
}

fn report(result: std::io::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("kido: {err}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "\
kido —— 终端待办

用法:
  kido                    打开待办
  kido --data <路径>      指定数据文件（也可用环境变量 KIDO_HOME 指定目录）
  kido --where            打印当前使用的数据文件路径
  kido --install          把所在目录加入用户 PATH
  kido --uninstall        卸载：移出 PATH、删程序和数据
  kido --uninstall -k     卸载但保留 todo.txt
  kido --help             显示这段帮助
  kido --version          显示版本

数据文件默认为 exe 同目录的 todo.txt；装到 %LOCALAPPDATA%\\kido 时整台机器
只多出这一个目录，删掉即彻底干净。

快捷键（程序内按 ? 也能看）:
  j/k 上下移动 · Space 勾选 · e/Enter 编辑备注 · n 新建 · r 改名
  d 删除 · u 撤销 · t 新建页签 · T 改名页签 · D 删除页签
  Tab/h/l 切换页签 · o 打开数据文件 · q/Esc 退出
  鼠标: 左键点行 = 勾选，右键 = 备注，点页签 = 切换，滚轮 = 移动

数据格式（纯文本，可手改）:
  >> 工作
  [ ] 写周报
      #note: 周三前发，附上数据
  [x] 回邮件
"
    );
}
