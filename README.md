# RollWin

RollWin 是一个基于 Tauri + Vue3 + TypeScript 开发的跨平台项目发布工具。它可以帮助开发者轻松管理和发布多个项目到不同的服务器环境。

[查看项目源码](https://github.com/SuperDraven/RollWin)

## 功能特性

- 🚀 多项目管理
- 🔄 版本控制与回滚
- 📦 自动化打包部署
- 🌍 支持多环境(测试环境/正式环境)
- 🔒 安全的 SSH 连接
- 📝 详细的操作日志
- ⏱️ 发布历史记录
- 💾 本地配置持久化

## 技术栈

- [Tauri](https://tauri.app/) - 跨平台应用框架
- [Vue 3](https://vuejs.org/) - 前端框架
- [TypeScript](https://www.typescriptlang.org/) - 类型安全
- [SSH2](https://docs.rs/ssh2/) - SSH 连接
- [Rust](https://www.rust-lang.org/) - 后端开发

## 安装使用

### 开发环境要求

- Node.js >= 16
- Rust >= 1.60
- Visual Studio Build Tools (Windows)
- WebView2 (Windows)

### 安装依赖

```bash
# 克隆项目
git clone https://github.com/SuperDraven/RollWin.git

# 安装依赖
npm install

# 启动开发服务器
npm run tauri dev

# 构建应用
npm run tauri build
```

## 使用说明

1. 新增项目
   - 填写项目基本信息
   - 配置项目目录和打包目录
   - 设置打包命令(可选)
   - 配置服务器信息

2. 发布项目
   - 选择要发布的项目
   - 点击发布按钮
   - 等待打包和上传完成

3. 版本回滚
   - 查看项目历史版本
   - 点击回滚按钮
   - 确认回滚操作

## 配置说明

项目配置文件保存在应用安装目录的 `config` 文件夹下:
- `config/projects.json` - 项目配置信息
- `version.json` - 项目版本信息(位于每个项目的打包目录)

## 贡献指南

1. Fork [本仓库](https://github.com/SuperDraven/RollWin)
2. 创建新的功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交你的更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 提交 Pull Request

## 开源协议

本项目采用 [MIT](LICENSE) 协议。

## 联系方式

如果你有任何问题或建议，欢迎提交 [Issue](https://github.com/SuperDraven/RollWin/issues)。