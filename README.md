# bwrap-run-script
A simple and universal Bubblewrap sandbox packaging tool that supports interactive configuration and multi-environment management. | 一个简易的 Bubblewrap 通用沙箱封装工具，支持交互式配置、多环境管理。

# 测试
```shell
elysia@archlinux ~ 
> bwrap-run
用法:
  1. 运行命令/应用 (交互式选择沙箱ID):
      /home/elysia/.local/bin/bwrap-run <可执行文件/命令> [参数...]
      例如: /home/elysia/.local/bin/bwrap-run firefox

  2. 运行命令/应用 (指定沙箱ID):
      /home/elysia/.local/bin/bwrap-run --id <沙箱名> <可执行文件/命令> [参数...]
      例如: /home/elysia/.local/bin/bwrap-run --id browser_work firefox

  3. 进入沙箱执行 Shell (指定沙箱ID):
      /home/elysia/.local/bin/bwrap-run --id <沙箱名>
      例如: /home/elysia/.local/bin/bwrap-run --id browser_work

  4. 管理和信息:
      /home/elysia/.local/bin/bwrap-run --list             # 列出所有已创建的沙箱
      /home/elysia/.local/bin/bwrap-run --help             # 打印此帮助信息
elysia@archlinux ~ [1]
> bwrap-run firefox
--- 交互式沙箱 ID 确认 ---
请输入沙箱 ID (留空使用默认: firefox): test_sandbox                
✅ 确认沙箱 ID: test_sandbox
---------------------------
ℹ️ 命令位于核心系统目录，将依赖 $PATH 查找执行: firefox
宿主数据目录 '/home/elysia/.sandbox_data/test_sandbox' 不存在，正在创建...
🎯 目标命令: firefox
📂 使用沙箱路径: /home/elysia (主机路径: /home/elysia/.sandbox_data/test_sandbox)
启用 Wayland 支持...
绑定 Vulkan 配置 (USR_SHARE 路径)...
绑定输入设备和 FUSE...
启用 AT-SPI (辅助功能) 支持...
启用 GVFS 支持...
绑定系统和用户自定义字体...
正在沙箱内运行: test_sandbox...
DEBUG: FULL_PATH_OF_COMMAND_ON_SYSTEM: /usr/sbin/firefox
✅ 策略：统一加载沙箱环境 HOME (/home/elysia)。
DEBUG: host_command_path: /usr/sbin/firefox
ℹ️ 策略：命令依赖于已全局绑定的系统目录 (/usr, /bin 等)。
^C⏎ 
elysia@archlinux ~ [130]
> bwrap-run --list
📦 已创建的沙箱列表 (持久化目录):
  - faugus-launcher
  - test_sandbox
  - wine
elysia@archlinux ~ 
> bwrap-run --id test_sandbox
ℹ️ 命令位于核心系统目录，将依赖 $PATH 查找执行: bash
🎯 目标命令: bash
📂 使用沙箱路径: /home/elysia (主机路径: /home/elysia/.sandbox_data/test_sandbox)
启用 Wayland 支持...
绑定 Vulkan 配置 (USR_SHARE 路径)...
绑定输入设备和 FUSE...
启用 AT-SPI (辅助功能) 支持...
启用 GVFS 支持...
绑定系统和用户自定义字体...
正在沙箱内运行: test_sandbox...
DEBUG: FULL_PATH_OF_COMMAND_ON_SYSTEM: /usr/bin/bash
✅ 策略：统一加载沙箱环境 HOME (/home/elysia)。
DEBUG: host_command_path: /usr/bin/bash
ℹ️ 策略：命令依赖于已全局绑定的系统目录 (/usr, /bin 等)。
[elysia@archlinux ~]$ ls /
bin  dev  etc  home  lib  lib64  proc  run  sbin  sys  tmp  usr
[elysia@archlinux ~]$ ls -la
总计 0
drwxr-xr-x 1 elysia elysia 60 12月10日 23:33 .
drwx------ 3 elysia elysia 60 12月10日 23:34 ..
drwxr-xr-x 1 elysia elysia 80 12月10日 23:33 .cache
drwxr-xr-x 1 elysia elysia 14 12月10日 23:33 .config
drwxr-xr-x 1 elysia elysia  0 12月10日 23:33 Downloads
drwx------ 1 elysia elysia 34 12月10日 23:33 .mozilla
[elysia@archlinux ~]$ ^C
[elysia@archlinux ~]$ 
exit
elysia@archlinux ~ [130]
> ./Desktop/hello
hello!
elysia@archlinux ~ 
> bwrap-run --id test_sandbox ./Desktop/hello
ℹ️ 命令位于 Home 目录，将使用绝对路径执行: /home/elysia/Desktop/hello
ℹ️ 正在沙箱中创建命令的父目录结构: /home/elysia/.sandbox_data/test_sandbox/Desktop
🎯 目标命令: hello
📂 使用沙箱路径: /home/elysia (主机路径: /home/elysia/.sandbox_data/test_sandbox)
启用 Wayland 支持...
绑定 Vulkan 配置 (USR_SHARE 路径)...
绑定输入设备和 FUSE...
启用 AT-SPI (辅助功能) 支持...
启用 GVFS 支持...
绑定系统和用户自定义字体...
正在沙箱内运行: test_sandbox...
DEBUG: FULL_PATH_OF_COMMAND_ON_SYSTEM: /home/elysia/Desktop/hello
✅ 策略：统一加载沙箱环境 HOME (/home/elysia)。
DEBUG: host_command_path: /home/elysia/Desktop/hello
✅ 策略：Home 目录命令，在环境上进行文件级绑定 (Overlay)。
hello!
```
