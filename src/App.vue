<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { open, message, ask } from '@tauri-apps/api/dialog'
import { invoke } from '@tauri-apps/api/tauri'
import { appDataDir, join } from '@tauri-apps/api/path'
import { exists, createDir, readTextFile, writeTextFile } from '@tauri-apps/api/fs'
import { Command } from '@tauri-apps/api/shell'

interface ProjectConfig {
  id: string
  name: string
  path: string
  projectPath: string  // 项目目录
  buildCommand?: string // 打包命令（可选）
  environment: 'prod' | 'test'
  version: string
  previousVersion?: string
  lastDeployTime?: string
  lastRollbackTime?: string
  serverInfo: {
    host: string
    username: string
    password: string
    remotePath: string
  }
}

const projects = ref<ProjectConfig[]>([])
const showAddDialog = ref(false)
const deployingProjects = ref<Map<string, boolean>>(new Map());
const deployStatusMap = ref<Map<string, DeployStatus>>(new Map());

// 添加辅助函数来管理项目状态
function setProjectDeploying(projectId: string, isDeploying: boolean) {
  deployingProjects.value.set(projectId, isDeploying);
  if (!isDeploying) {
    deployStatusMap.value.delete(projectId);
  }
}

function isProjectDeploying(projectId: string): boolean {
  return deployingProjects.value.get(projectId) || false;
}

// 新项目表单数据
const newProject = ref<ProjectConfig>({
  id: '',
  name: '',
  path: '',
  projectPath: '',
  buildCommand: undefined,  // 改为 undefined
  environment: 'prod',
  version: '1.0.0',
  previousVersion: undefined,
  lastDeployTime: undefined,
  lastRollbackTime: undefined,
  serverInfo: {
    host: '',
    username: '',
    password: '',
    remotePath: ''
  }
})

// 添加编辑状态变量
const showEditDialog = ref(false)
const editingProject = ref<ProjectConfig | null>(null)

// 打开编辑对话框
function openEditDialog(project: ProjectConfig) {
  editingProject.value = { ...project }  // 复制项目数据
  showEditDialog.value = true
}

// 保存编辑
async function saveEdit() {
  if (!editingProject.value) return;
  
  try {
    const appDir = await invoke('get_app_dir');
    const configPath = await join(appDir as string, 'config', 'projects.json');

    // 更新项目列表
    const index = projects.value.findIndex(p => p.id === editingProject.value!.id);
    if (index !== -1) {
      projects.value[index] = { ...editingProject.value };
    }
    
    // 保存到文件
    await writeTextFile(configPath, JSON.stringify(projects.value, null, 2));
    showEditDialog.value = false;
    editingProject.value = null;
    await message('修改成功！', { type: 'info' });
  } catch (error) {
    await message('修改失败：' + error, { type: 'error' });
  }
}

// 加载项目列表
async function loadProjects() {
  try {
    // 获取应用安装目录
    const appDir = await invoke('get_app_dir');
    const configPath = await join(appDir as string, 'config', 'projects.json');
    const configDir = await join(appDir as string, 'config');
    
    // 确保配置目录存在
    const dirExists = await exists(configDir);
    if (!dirExists) {
      await createDir(configDir, { recursive: true });
    }
    
    const fileExists = await exists(configPath);
    if (!fileExists) {
      await writeTextFile(configPath, JSON.stringify([], null, 2));
      projects.value = [];
      return;
    }

    const content = await readTextFile(configPath);
    projects.value = JSON.parse(content);
  } catch (error) {
    console.error('加载项目列表失败:', error);
    await message('加载项目列表失败：' + error, { type: 'error' });
    projects.value = [];
  }
}

// 选择目录
async function selectDirectory() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择打包目录'
    });
    
    if (selected) {
      // 验证目录是否可访问
      try {
        await exists(selected as string);
        newProject.value.path = selected as string;
      } catch (error) {
        await message('无法访问选择的目录，请确保有足够的权限', { type: 'error' });
      }
    }
  } catch (error) {
    console.error('选择目录失败:', error);
    await message('选择目录失败：' + error, { type: 'error' });
  }
}

// 验证打包命令
function validateBuildCommand(command: string | undefined): boolean {
  // 如果命令为空或未定义，返回 true
  if (!command) {
    return true;
  }
  // 只允许 npm run xxx 格式的命令
  const regex = /^npm run [\w:\-]+$/;
  return regex.test(command);
}

// 保存项目时验证命令
async function saveProject() {
  if (!newProject.value.name || !newProject.value.path) {
    await message('请填写项目名称并选择目录', { type: 'error' });
    return;
  }

  // 如果有打包命令，则验证格式
  if (newProject.value.buildCommand && !validateBuildCommand(newProject.value.buildCommand)) {
    await message('打包命令格式不正确，请使用 npm run xxx 格式', { type: 'error' });
    return;
  }

  try {
    const appDir = await invoke('get_app_dir');
    const configPath = await join(appDir as string, 'config', 'projects.json');
    const configDir = await join(appDir as string, 'config');

    const dirExists = await exists(configDir);
    if (!dirExists) {
      await createDir(configDir, { recursive: true });
    }

    newProject.value.id = Date.now().toString();
    projects.value.push({ ...newProject.value });
    await writeTextFile(configPath, JSON.stringify(projects.value, null, 2));
    showAddDialog.value = false;
    newProject.value = {
      id: '',
      name: '',
      path: '',
      projectPath: '',
      buildCommand: undefined,  // 改为 undefined
      environment: 'prod',
      version: '1.0.0',
      serverInfo: {
        host: '',
        username: '',
        password: '',
        remotePath: ''
      }
    };
    await message('保存成功！', { type: 'info' });
  } catch (error) {
    await message('保存失败：' + error, { type: 'error' });
  }
}

// 修改状态管理接口
interface DeployStatus {
  isDeploying: boolean;
  status: string;
}

// 修改发布函数中的状态更新
async function deployProject(project: ProjectConfig) {
  try {
    if (!project.serverInfo.remotePath) {
      await message('远程目录路径不能为空', { type: 'error' });
      return;
    }

    setProjectDeploying(project.id, true);
    deployStatusMap.value.set(project.id, { isDeploying: true, status: '准备发布...' });

    if (project.buildCommand) {
      try {
        deployStatusMap.value.set(project.id, { isDeploying: true, status: '正在执行打包命令...' });
        const isWindows = navigator.platform.includes('Win');
        const npmCmd = isWindows ? 'npm-windows' : 'npm';
        
        const output = await new Command(npmCmd, ['run', project.buildCommand.replace('npm run ', '')], {
          cwd: project.projectPath
        }).execute();

        if (output.code !== 0) {
          throw new Error(output.stderr || '打包失败');
        }
      } catch (error) {
        throw new Error(`执行打包命令失败：${error}`);
      }
    }

    deployStatusMap.value.set(project.id, { isDeploying: true, status: '正在准备文件...' });
    // 更新版本号
    const versionParts = project.version.split('.');
    project.previousVersion = project.version;  // 保存当前版本号
    
    // 正确处理版本号
    const major = parseInt(versionParts[0] || '1');
    const minor = parseInt(versionParts[1] || '0');
    const patch = parseInt(versionParts[2] || '0') + 1;
    project.version = `${major}.${minor}.${patch}`;

    // 创建 version.json 内容
    const versionInfo = {
      version: project.version,
      previousVersion: project.previousVersion,
      updateTime: new Date().toISOString(),
      projectName: project.name,
      environment: project.environment
    };

    // 在本地项目目录下创建 version.json
    try {
      const versionPath = await join(project.path, 'version.json');
      await writeTextFile(versionPath, JSON.stringify(versionInfo, null, 2));

      // 保存到本地配置
      const appData = await appDataDir();
      const configDir = await join(appData, 'config');
      const configPath = await join(configDir, 'projects.json');
      await writeTextFile(configPath, JSON.stringify(projects.value, null, 2));
    } catch (error) {
      console.error('创建版本文件失败:', error);
      // 继续执行，不断发布程
    }

    deployStatusMap.value.set(project.id, { isDeploying: true, status: '正在上传文件...' });
    // 保远程径以 / 开头
    const remotePath = project.serverInfo.remotePath.startsWith('/')
      ? project.serverInfo.remotePath
      : '/' + project.serverInfo.remotePath;

    // 更新发布时间
    project.lastDeployTime = new Date().toISOString();

    // 调用后发布
    await invoke('deploy_project', {
      projectName: project.name,
      path: project.path,
      env: project.environment,
      host: project.serverInfo.host,
      username: project.serverInfo.username,
      password: project.serverInfo.password,
      remotePath: remotePath
    });

    deployStatusMap.value.set(project.id, { isDeploying: true, status: '发布完成' });
  } catch (error) {
    await message(`发布失败：${error}`, { type: 'error' });
    if (project.previousVersion) {
      project.version = project.previousVersion;
    }
    deployStatusMap.value.set(project.id, { isDeploying: true, status: '发布失败' });
  } finally {
    setProjectDeploying(project.id, false);
  }
}

// 回滚版本
async function rollbackVersion(project: ProjectConfig) {
  const confirmed = await ask('确定要回滚到上一个版本吗？', {
    title: '确认回滚',
    type: 'warning'
  });

  if (!confirmed) return;

  try {
    setProjectDeploying(project.id, true);
    deployStatusMap.value.set(project.id, { isDeploying: true, status: '准备回滚...' });

    if (!project.previousVersion) {
      throw new Error('没有可回滚的版本');
    }

    const currentVersion = project.version;
    
    // 检查上一个版本号的格式是否正确
    const prevVersionParts = project.previousVersion.split('.');
    if (prevVersionParts.length !== 3) {
      throw new Error('版本号格式错误');
    }

    // 确保版本号各部分都是整数
    const major = parseInt(prevVersionParts[0] || '1');
    const minor = parseInt(prevVersionParts[1] || '0');
    const patch = parseInt(prevVersionParts[2] || '0');

    if (isNaN(major) || isNaN(minor) || isNaN(patch)) {
      throw new Error('版本号格式错误');
    }


    project.version = `${major}.${minor}.${patch}`;
    project.previousVersion = currentVersion;

    // 创建 version.json 内容
    const versionInfo = {
      version: project.version,
      previousVersion: project.previousVersion,
      updateTime: new Date().toISOString(),
      projectName: project.name,
      environment: project.environment
    };

    // 本地项目目录下更新 version.json
    try {
      const versionPath = await join(project.path, 'version.json');
      await writeTextFile(versionPath, JSON.stringify(versionInfo, null, 2));

      // 保存到本地配置
      const appData = await appDataDir();
      const configDir = await join(appData, 'config');
      const configPath = await join(configDir, 'projects.json');
      await writeTextFile(configPath, JSON.stringify(projects.value, null, 2));
    } catch (error) {
      console.error('更新版本文件失败:', error);
      // 继续执行，不中断回滚流程
    }

    deployStatusMap.value.set(project.id, { isDeploying: true, status: '正在回滚文件...' });
    // 确保远程路径以 / 开头
    const remotePath = project.serverInfo.remotePath.startsWith('/')
      ? project.serverInfo.remotePath
      : '/' + project.serverInfo.remotePath;

    await invoke('rollback_project', {
      projectName: project.name,
      path: project.path,
      env: project.environment,
      host: project.serverInfo.host,
      username: project.serverInfo.username,
      password: project.serverInfo.password,
      remotePath: remotePath
    });

    // 更新回滚时间
    project.lastRollbackTime = new Date().toISOString();

    // 保存到本地配置
    const appData = await appDataDir();
    const configDir = await join(appData, 'config');
    const configPath = await join(configDir, 'projects.json');
    await writeTextFile(configPath, JSON.stringify(projects.value, null, 2));

    deployStatusMap.value.set(project.id, { isDeploying: true, status: '回滚完成' });
  } catch (error) {
    await message(`回滚失败：${error}`, { type: 'error' });
    if (project.previousVersion) {
      const tempVersion = project.version;
      project.version = project.previousVersion;
      project.previousVersion = tempVersion;
    }
    deployStatusMap.value.set(project.id, { isDeploying: true, status: '回滚失败' });
  } finally {
    setProjectDeploying(project.id, false);
  }
}

// 删除项目
async function deleteProject(project: ProjectConfig) {
  const confirmed = await ask('确定要删除该项目吗？', {
    title: '确认删除',
    type: 'warning'
  });

  if (!confirmed) return;

  try {
    const appDir = await invoke('get_app_dir');
    const configPath = await join(appDir as string, 'config', 'projects.json');

    projects.value = projects.value.filter(p => p.id !== project.id);
    await writeTextFile(configPath, JSON.stringify(projects.value, null, 2));
    await message('删除成功！', { type: 'info' });
  } catch (error) {
    await message(`删除失败：${error}`, { type: 'error' });
  }
}

// 添加时间格式化函数
function formatTime(timeString: string): string {
  return new Date(timeString).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  });
}

// 选择项目目录
async function selectProjectDirectory() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择项目目录'
    });
    
    if (selected) {
      try {
        await exists(selected as string);
        newProject.value.projectPath = selected as string;
      } catch (error) {
        await message('无法访问选择的目录，请确保有足够的权限', { type: 'error' });
      }
    }
  } catch (error) {
    console.error('选择目录失败:', error);
    await message('选择目录失败：' + error, { type: 'error' });
  }
}

// 编辑时选择项目目录
async function selectEditProjectDirectory() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择项目目录'
    });
    
    if (selected && editingProject.value) {
      try {
        await exists(selected as string);
        editingProject.value.projectPath = selected as string;
      } catch (error) {
        await message('无法访问选择的目录，请确保有足够的权限', { type: 'error' });
      }
    }
  } catch (error) {
    console.error('选择目录失败:', error);
    await message('选择目录失败：' + error, { type: 'error' });
  }
}

// 编辑时选择打包目录
async function selectEditDirectory() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择打包目录'
    });
    
    if (selected && editingProject.value) {
      try {
        await exists(selected as string);
        editingProject.value.path = selected as string;
      } catch (error) {
        await message('无法访问选择的目录，请确保有足够的权限', { type: 'error' });
      }
    }
  } catch (error) {
    console.error('选择目录失败:', error);
    await message('选择目录失败：' + error, { type: 'error' });
  }
}

onMounted(() => {
  loadProjects()
})
</script>

<template>
  <div class="container">
    <div class="header">
      <h1 class="title">RollWin</h1>
      <button class="add-btn" @click="showAddDialog = true">新增项目</button>
    </div>

    <!-- 项目列表 -->
    <div class="project-list">
      <div v-for="project in projects" :key="project.id" class="project-item">
        <div class="project-info">
          <div class="project-header">
            <h3>{{ project.name }}</h3>
            <div class="project-actions">
              <button class="edit-btn" @click="openEditDialog(project)">修改</button>
              <button 
                class="deploy-btn" 
                :disabled="isProjectDeploying(project.id)" 
                @click="deployProject(project)"
              >
                {{ isProjectDeploying(project.id) ? '发布中...' : '发布' }}
              </button>
              <button 
                class="rollback-btn" 
                :disabled="isProjectDeploying(project.id)" 
                @click="rollbackVersion(project)"
              >
                回滚
              </button>
              <button class="delete-btn" @click="deleteProject(project)">删除</button>
            </div>
          </div>
          <p class="path">{{ project.path }}</p>
          
          <!-- 添加状态显示区域 -->
          <div v-if="isProjectDeploying(project.id)" class="deploy-status">
            <div class="status-indicator"></div>
            {{ deployStatusMap.get(project.id)?.status || '处理中...' }}
          </div>

          <div class="project-meta">
            <div class="meta-left">
              <span class="environment" :class="project.environment">
                {{ project.environment === 'prod' ? '正式环境' : '测试环境' }}
              </span>
              <span class="version">v{{ project.version }}</span>
            </div>
            <div class="meta-right">
              <span v-if="project.lastDeployTime" class="time-info deploy-time">
                <span class="time-label">上次发布：</span>
                {{ formatTime(project.lastDeployTime) }}
              </span>
              <span v-if="project.lastRollbackTime" class="time-info rollback-time">
                <span class="time-label">上次回滚：</span>
                {{ formatTime(project.lastRollbackTime) }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 新增项目对话框 -->
    <div v-if="showAddDialog" class="dialog-overlay">
      <div class="dialog">
        <h2>新增项目</h2>
        <div class="form-group">
          <label>项目名称：</label>
          <div class="input-wrapper">
            <input 
              v-model="newProject.name"
              type="text"
              placeholder="请输入项目名称"
            >
          </div>
        </div>

        <div class="form-group">
          <label>项目目录：</label>
          <div class="input-wrapper">
            <input 
              :value="newProject.projectPath"
              type="text"
              readonly
              placeholder="请选择项目目录"
            >
            <button class="select-btn" @click="selectProjectDirectory">选择目录</button>
          </div>
        </div>

        <div class="form-group">
          <label>打包目录：</label>
          <div class="input-wrapper">
            <input 
              :value="newProject.path"
              type="text"
              readonly
              placeholder="请选择打包目录"
            >
            <button class="select-btn" @click="selectDirectory">选择目录</button>
          </div>
        </div>

        <div class="form-group">
          <label>打包命令：</label>
          <div class="input-wrapper">
            <input 
              v-model="newProject.buildCommand"
              type="text"
              placeholder="请输入打包命令，如：npm run build:prod"
            >
          </div>
        </div>

        <div class="form-group">
          <label>发布环境：</label>
          <div class="environment-switch">
            <button 
              :class="{ active: newProject.environment === 'test' }"
              @click="newProject.environment = 'test'"
            >测试环境</button>
            <button 
              :class="{ active: newProject.environment === 'prod' }"
              @click="newProject.environment = 'prod'"
            >正式环境</button>
          </div>
        </div>

        <div class="form-group">
          <label>服务器配置</label>
          <div class="server-config">
            <div class="input-row">
              <input 
                v-model="newProject.serverInfo.host"
                type="text"
                placeholder="服务器IP地址"
              >
            </div>
            <div class="input-row">
              <input 
                v-model="newProject.serverInfo.username"
                type="text"
                placeholder="用户名"
              >
            </div>
            <div class="input-row">
              <input 
                v-model="newProject.serverInfo.password"
                type="password"
                placeholder="密码"
              >
            </div>
            <div class="input-row">
              <input 
                v-model="newProject.serverInfo.remotePath"
                type="text"
                placeholder="远程目录路径"
              >
            </div>
          </div>
        </div>

        <div class="dialog-actions">
          <button class="cancel-btn" @click="showAddDialog = false">取消</button>
          <button class="save-btn" @click="saveProject">保存</button>
        </div>
      </div>
    </div>

    <!-- 添加编辑对话框 -->
    <div v-if="showEditDialog && editingProject" class="dialog-overlay">
      <div class="dialog">
        <h2>修改项目</h2>
        <div class="form-group">
          <label>项目名称：</label>
          <div class="input-wrapper">
            <input 
              v-model="editingProject.name"
              type="text"
              placeholder="请输入项目名称"
            >
          </div>
        </div>

        <div class="form-group">
          <label>项目目录：</label>
          <div class="input-wrapper">
            <input 
              :value="editingProject.projectPath"
              type="text"
              readonly
              placeholder="请选择项目目录"
            >
            <button class="select-btn" @click="selectEditProjectDirectory">选择目录</button>
          </div>
        </div>

        <div class="form-group">
          <label>打包目录：</label>
          <div class="input-wrapper">
            <input 
              :value="editingProject.path"
              type="text"
              readonly
              placeholder="请选择打包目录"
            >
            <button class="select-btn" @click="selectEditDirectory">选择目录</button>
          </div>
        </div>

        <div class="form-group">
          <label>打包命令：</label>
          <div class="input-wrapper">
            <input 
              v-model="editingProject.buildCommand"
              type="text"
              placeholder="请输入打包命令，如：npm run build:prod（可选）"
            >
          </div>
        </div>

        <div class="form-group">
          <label>发布环境：</label>
          <div class="environment-switch">
            <button 
              :class="{ active: editingProject.environment === 'test' }"
              @click="editingProject.environment = 'test'"
            >测试环境</button>
            <button 
              :class="{ active: editingProject.environment === 'prod' }"
              @click="editingProject.environment = 'prod'"
            >正式环境</button>
          </div>
        </div>

        <div class="form-group">
          <label>服务器配置：</label>
          <div class="server-config">
            <div class="input-row">
              <input 
                v-model="editingProject.serverInfo.host"
                type="text"
                placeholder="服务器IP地址"
              >
            </div>
            <div class="input-row">
              <input 
                v-model="editingProject.serverInfo.username"
                type="text"
                placeholder="用户名"
              >
            </div>
            <div class="input-row">
              <input 
                v-model="editingProject.serverInfo.password"
                type="password"
                placeholder="密码"
              >
            </div>
            <div class="input-row">
              <input 
                v-model="editingProject.serverInfo.remotePath"
                type="text"
                placeholder="远程目录路径"
              >
            </div>
          </div>
        </div>

        <div class="form-group">
          <label>操作记录：</label>
          <div class="time-records">
            <div v-if="editingProject.lastDeployTime" class="time-record">
              <span class="time-label">上次发布：</span>
              <span>{{ formatTime(editingProject.lastDeployTime) }}</span>
            </div>
            <div v-if="editingProject.lastRollbackTime" class="time-record">
              <span class="time-label">上次回滚：</span>
              <span>{{ formatTime(editingProject.lastRollbackTime) }}</span>
            </div>
          </div>
        </div>

        <div class="dialog-actions">
          <button class="cancel-btn" @click="showEditDialog = false">取消</button>
          <button class="save-btn" @click="saveEdit">保存</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
html, body {
  margin: 0;
  padding: 0;
  height: 100vh;
  width: 100vw;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  overflow: hidden;
}

#app {
  height: 100vh;
  width: 100vw;
  background-color: #f8f9fa;
  overflow: hidden;
  color: #333;
  
}
</style>

<style scoped>
.container {
  width: 100%;
  height: 100vh;
  padding: 20px;
  box-sizing: border-box;
  overflow-y: auto;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 30px;
  padding: 0 20px;
  position: sticky;
  top: 0;

  z-index: 10;
  height: 60px;
 
}

.title {
  margin: 0;
  color: #2563eb;
  font-weight: 600;
}

.add-btn {
  padding: 10px 20px;
  background: #2563eb;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-weight: 600;
  transition: all 0.3s ease;
}

.add-btn:hover {
  background: #1d4ed8;
  transform: translateY(-1px);
}

.project-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 0 20px;
}

.project-item {
  background: #fff;
  border-radius: 8px;
  padding: 24px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
  border: 1px solid #e5e7eb;
  transition: all 0.3s ease;
  width: 100%;
}

.project-item:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(37, 99, 235, 0.1);
}

.project-info {
  width: 100%;
}

.project-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
  width: 100%;
}

.project-info h3 {
  font-size: 18px;
  margin: 0;
  flex-shrink: 0;
  margin-right: auto;  /* 让标题占据左侧所有空间 */
}

.path {
  margin: 8px 0;
  color: #64748b;
  font-size: 14px;
  text-align: left;
  width: 100%;
}

.project-meta {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 12px;
  width: 100%;
}

.meta-left {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-shrink: 0;
}

.meta-right {
  display: flex;
  gap: 16px;
  color: #64748b;
  font-size: 13px;
  margin-left: auto;  /* 让时间信息靠右 */
}

.time-info {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 12px;
}

.deploy-time {
  background: #f0fdf4;
  color: #166534;
}

.deploy-time .time-label {
  color: #15803d;
}

.rollback-time {
  background: #faf5ff;
  color: #6b21a8;
}

.rollback-time .time-label {
  color: #7e22ce;
}

.time-label {
  color: #94a3b8;
}

.environment {
  display: inline-block;
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 600;
}

.environment.prod {
  background: #ecfdf5;
  color: #059669;
}

.environment.test {
  background: #eff6ff;
  color: #2563eb;
}

.version {
  font-size: 12px;
  color: #64748b;
  background: #f1f5f9;
  padding: 4px 12px;
  border-radius: 4px;
  margin-left: 8px;
}

.project-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
  margin-left: 20px;  /* 与标题保持一定距离 */
}

.edit-btn, .deploy-btn, .rollback-btn, .delete-btn {
  padding: 6px 12px;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-weight: 600;
  font-size: 13px;
  transition: all 0.2s ease;
}

.edit-btn {
  background: #2563eb;
  color: white;
}

.deploy-btn {
  background: #059669;
  color: white;
}

.rollback-btn {
  background: #7c3aed;
  color: white;
}

.delete-btn {
  background: #dc2626;
  color: white;
}

.dialog-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  justify-content: center;
  align-items: center;
  backdrop-filter: blur(4px);
}

.dialog {
  background: #fff;
  border-radius: 8px;
  padding: 30px;
  width: 80%;
  max-height: 90vh;
  overflow-y: auto;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.1);
}

.dialog h2 {
  margin: 0 0 20px 0;
  color: #1e293b;
}

.form-group {
  margin-bottom: 20px;
  display: flex;
  align-items: center;
  gap: 20px;
}

.form-group label {
  min-width: 100px;
  flex-shrink: 0;
  text-align: right;
  color: #1e293b;
  font-weight: 500;
}

.form-group .input-wrapper {
  flex: 1;
  display: flex;
  gap: 8px;
}

.input-group {
  display: flex;
  gap: 8px;
  width: 100%;
}

input {
  flex: 1;
  padding: 10px;
  border: 1px solid #e5e7eb;
  border-radius: 4px;
  font-size: 14px;
  color: #1e293b;
  background: #fff;
}

.select-btn {
  padding: 8px 16px;
  background: #2563eb;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-weight: 500;
  white-space: nowrap;
  transition: all 0.2s ease;
}

.select-btn:hover {
  background: #1d4ed8;
}

.environment-switch {
  display: flex;
  gap: 10px;
  flex: 1;
}

.server-config {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.input-row {
  display: flex;
  gap: 8px;
  width: 100%;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 30px;
}

.cancel-btn {
  padding: 10px 20px;
  background: #f1f5f9;
  color: #64748b;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.save-btn {
  padding: 10px 20px;
  background: #2563eb;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

/* 添加按钮悬停效果 */
.edit-btn:hover, .deploy-btn:hover, .rollback-btn:hover, .delete-btn:hover,
.save-btn:hover, .cancel-btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

/* 自定义滚动条 */
::-webkit-scrollbar {
  width: 8px;
}

::-webkit-scrollbar-track {
  background: #f1f5f9;
}

::-webkit-scrollbar-thumb {
  background: #cbd5e1;
  border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
  background: #94a3b8;
}

.time-records {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.time-record {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  color: #64748b;
}

.time-record .time-label {
  color: #94a3b8;
  font-weight: 500;
}

.deploy-status {
  margin: 12px 0;
  padding: 12px;
  background: #f0f9ff;
  color: #0369a1;
  border-radius: 6px;
  font-size: 14px;
  display: flex;
  align-items: center;
  gap: 8px;
  animation: fadeIn 0.3s ease;
}

.status-indicator {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #0ea5e9;
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0% {
    transform: scale(0.95);
    box-shadow: 0 0 0 0 rgba(14, 165, 233, 0.7);
  }
  70% {
    transform: scale(1);
    box-shadow: 0 0 0 6px rgba(14, 165, 233, 0);
  }
  100% {
    transform: scale(0.95);
    box-shadow: 0 0 0 0 rgba(14, 165, 233, 0);
  }
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
