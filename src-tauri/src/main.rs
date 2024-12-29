// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::command;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::{Read, Write};
use ssh2::Session;
use std::net::TcpStream;

// 获取备份目录
#[command]
fn get_backup_dir(project_name: &str, env: &str) -> Result<String, String> {
    let app_dir = get_app_dir();
    let backup_dir = Path::new(&app_dir)
        .join("backups")
        .join(project_name)
        .join(env);
    
    // 确保备份目录存在
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("创建备份目录失败: {}", e))?;
    
    Ok(backup_dir.to_str()
        .ok_or_else(|| "路径转换失败".to_string())?
        .to_string())
}

// 从远程服务器下载文件到本地备份
fn download_for_backup(
    sftp: &ssh2::Sftp,
    remote_path: &Path,
    backup_path: &Path,
) -> Result<(), String> {
    // 确保备份目录存在
    fs::create_dir_all(backup_path)
        .map_err(|e| format!("创建备份目录失败: {}", e))?;

    // 读取远程目录内容
    let remote_dir = match sftp.readdir(remote_path) {
        Ok(dir) => dir,
        Err(_) => {
            // 如果目录不存在，直接返回
            return Ok(());
        }
    };

    // 遍历并下载每个文件
    for (path, stat) in remote_dir {
        let file_name = path.file_name()
            .ok_or_else(|| "无效的文件名".to_string())?
            .to_str()
            .ok_or_else(|| "文件名编码错误".to_string())?;
        
        let remote_file_path = remote_path.join(file_name);
        let backup_file_path = backup_path.join(file_name);

        if stat.is_dir() {
            // 如果是目录递归下载
            download_for_backup(sftp, &remote_file_path, &backup_file_path)?;
        } else {
            // 如果是文件，直接下载
            let mut remote_file = sftp.open(&remote_file_path)
                .map_err(|e| format!("打开远程文件失败 {}: {}", file_name, e))?;
            
            let mut contents = Vec::new();
            remote_file.read_to_end(&mut contents)
                .map_err(|e| format!("读取远程文件失败 {}: {}", file_name, e))?;

            fs::write(&backup_file_path, contents)
                .map_err(|e| format!("写入备份文件失败 {}: {}", file_name, e))?;
        }
    }

    Ok(())
}

#[command]
async fn deploy_project(
    project_name: String,
    path: String,
    env: String,
    host: String,
    username: String,
    password: String,
    remote_path: String,
) -> Result<(), String> {
    // 连接服务器
    let host_with_port = if !host.contains(":") {
        format!("{}:22", host)
    } else {
        host.clone()
    };

    // 添加重试逻辑
    let mut retries = 3;
    let mut last_error = None;
    
    while retries > 0 {
        match TcpStream::connect(&host_with_port) {
            Ok(tcp) => {
                let mut sess = Session::new()
                    .map_err(|e| format!("创建会话失败: {}", e))?;
                
                sess.set_tcp_stream(tcp);
                sess.handshake()
                    .map_err(|e| format!("握手失败: {}", e))?;

                sess.set_timeout(30000);  // 30秒

                sess.userauth_password(&username, &password)
                    .map_err(|e| format!("认证失败: {}", e))?;

                let sftp = sess.sftp()
                    .map_err(|e| format!("创建SFTP会话失败: {}", e))?;

                // 在上传文件之前先备份
                let backup_dir = get_backup_dir(&project_name, &env)?;
                let remote_path_buf = PathBuf::from(&remote_path);
                
                // 创建备份
                download_for_backup(&sftp, &remote_path_buf, &PathBuf::from(backup_dir))?;

                // 上传文件
                let local_path = Path::new(&path);
                let remote_path = Path::new(&remote_path);

                upload_dir(&sftp, local_path, remote_path)
                    .map_err(|e| format!("上传文件失败: {}", e))?;

                return Ok(());
            }
            Err(e) => {
                last_error = Some(e);
                retries -= 1;
                if retries > 0 {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        }
    }

    Err(format!("连接服务器失败: {}", last_error.unwrap()))
}

fn upload_dir(sftp: &ssh2::Sftp, local_path: &Path, remote_path: &Path) -> Result<(), String> {
    // 尝试创建当前目录，忽略已存在的错误
    let _ = sftp.mkdir(remote_path, 0o755);

    for entry in fs::read_dir(local_path)
        .map_err(|e| format!("读取本地目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let local_path = entry.path();
        let file_name = local_path.file_name()
            .ok_or_else(|| "无效的文件名".to_string())?
            .to_str()
            .ok_or_else(|| "文件名编码错误".to_string())?;
        let remote_path = remote_path.join(file_name);

        if local_path.is_dir() {
            let _ = sftp.mkdir(&remote_path, 0o755);
            upload_dir(sftp, &local_path, &remote_path)?;
        } else {
            let mut file = fs::File::open(&local_path)
                .map_err(|e| format!("打开本地文件失败 {}: {}", local_path.display(), e))?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| format!("读取文件内容失败 {}: {}", local_path.display(), e))?;

            let mut remote_file = sftp.create(&remote_path)
                .map_err(|e| format!("创建远程文件失败 {}: {}", remote_path.display(), e))?;
            remote_file.write(&contents)
                .map_err(|e| format!("写入远程文件失败 {}: {}", remote_path.display(), e))?;
        }
    }

    Ok(())
}

#[command]
async fn rollback_project(
    project_name: String,
    path: String,
    env: String,
    host: String,
    username: String,
    password: String,
    remote_path: String,
) -> Result<(), String> {
    // 连接服务器
    let host_with_port = if !host.contains(":") {
        format!("{}:22", host)
    } else {
        host
    };

    let tcp = TcpStream::connect(&host_with_port)
        .map_err(|e| format!("连接服务器失败: {}", e))?;
    
    let mut sess = Session::new()
        .map_err(|e| format!("创建会话失败: {}", e))?;
    
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("握手失败: {}", e))?;

    sess.userauth_password(&username, &password)
        .map_err(|e| format!("认证失败: {}", e))?;

    let sftp = sess.sftp()
        .map_err(|e| format!("创建SFTP会话失败: {}", e))?;

    // 获取最新的备份
    let backup_dir = get_backup_dir(&project_name, &env)?;
    let backup_path = PathBuf::from(backup_dir);

    // 检查备份是否存在
    if !backup_path.exists() {
        return Err("没有找到可用的备份".to_string());
    }

    // 上传 version.json
    let remote_version_path = Path::new(&remote_path).join("version.json");
    let local_version_path = Path::new(&path).join("version.json");

    // 读取本地 version.json
    let version_content = fs::read(&local_version_path)
        .map_err(|e| format!("读取版本文件失败: {}", e))?;

    // 上传 version.json
    let mut remote_file = sftp.create(&remote_version_path)
        .map_err(|e| format!("创建远程版本文件失败: {}", e))?;
    remote_file.write(&version_content)
        .map_err(|e| format!("写入远程版本文件失败: {}", e))?;

    // 上传备份文件到服务器
    let remote_path = Path::new(&remote_path);
    upload_dir(&sftp, &backup_path, remote_path)
        .map_err(|e| format!("回滚失败: {}", e))?;

    Ok(())
}

#[command]
fn get_app_dir() -> String {
    #[cfg(debug_assertions)]
    {
        // 开发模式下使用当前目录
        let current_dir = std::env::current_dir().unwrap();
        current_dir.to_str().unwrap().to_string()
    }

    #[cfg(not(debug_assertions))]
    {
        // 生产模式下使用应用安装目录
        let current_exe = std::env::current_exe().unwrap();
        let app_dir = current_exe.parent().unwrap();
        app_dir.to_str().unwrap().to_string()
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![deploy_project, rollback_project, get_app_dir, get_backup_dir])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
