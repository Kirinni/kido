//! kido —— 终端待办：页签、备注、鼠标勾选。
//!
//! 单文件 exe，数据就是 exe 旁边的 `todo.txt`，删目录即卸载。

pub mod app;
pub mod cli;
pub mod editor;
pub mod store;
pub mod ui;

#[cfg(windows)]
mod path_env;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;

use app::App;
use store::Store;

pub fn run(data: Option<PathBuf>) -> io::Result<()> {
    let path = data.unwrap_or_else(store::default_data_path);
    let store = Store::load(&path)?;
    let mut app = App::new(store);

    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let result = event_loop(&mut terminal, &mut app);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        app.tick();
        terminal.draw(|frame| ui::draw(frame, app))?;
        if app.quit {
            return Ok(());
        }
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) => app.on_key(key),
                Event::Mouse(mouse) => app.on_mouse(mouse),
                _ => {}
            }
        }
    }
}

/// `kido --install`：把自己所在目录加进用户 PATH。
pub fn install_to_path() -> io::Result<()> {
    let dir = store::exe_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "定位不到 exe 所在目录"))?;

    #[cfg(windows)]
    {
        let added = path_env::add_dir(&dir)?;
        println!("程序目录: {}", dir.display());
        println!("数据文件: {}", store::default_data_path().display());
        println!(
            "PATH:     {}",
            if added {
                "已加入用户 PATH，新开终端后可直接输入 kido"
            } else {
                "已经在用户 PATH 里"
            }
        );
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "目前只实现了 Windows 的 PATH 处理",
        ))
    }
}

/// `kido --uninstall`：清 PATH、删数据、删自己。
pub fn uninstall(keep_data: bool) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let data = store::default_data_path();
    let is_install_dir = store::install_dir().is_some_and(|d| d == dir);

    println!("kido 卸载");
    println!("  程序: {}", exe.display());
    println!("  数据: {}", data.display());

    #[cfg(windows)]
    match path_env::remove_dir(&dir) {
        Ok(true) => println!("  PATH: 已移除 {}", dir.display()),
        Ok(false) => println!("  PATH: 没有相关条目"),
        Err(err) => println!("  PATH: 修改失败（{err}），可手动从环境变量里删"),
    }

    let data_inside_dir = data.starts_with(&dir);
    if keep_data {
        println!("  数据: 保留");
    } else if data.exists() {
        std::fs::remove_file(&data)?;
        println!("  数据: 已删除");
    }

    // 运行中的 exe 不能删自己，交给一个等待几秒的 cmd 收尾。
    // rmdir 只在「这确实是安装目录」时用，避免误删用户自己的文件夹。
    let mut script = String::from("ping -n 4 127.0.0.1 >nul");
    if !keep_data && is_install_dir {
        script.push_str(&format!(" & rmdir /s /q \"{}\"", dir.display()));
    } else {
        script.push_str(&format!(" & del /f /q \"{}\"", exe.display()));
        if !keep_data && data_inside_dir {
            script.push_str(&format!(" & rmdir /q \"{}\"", dir.display()));
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // raw_arg：cmd 不认 Rust 默认的 MSVC 风格引号转义，这里直接拼原样命令行
        std::process::Command::new("cmd")
            .arg("/C")
            .raw_arg(script)
            .spawn()?;
    }

    println!("  完成，程序和目录会在几秒后消失");
    Ok(())
}
