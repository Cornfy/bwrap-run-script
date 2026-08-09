use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

// ==========================================
// 🧱 核心结构体定义 (Core Data Structures)
// ==========================================

/// 挂载描述规范实体：对齐 bwrap-winer 的挂载管线设计
pub struct MountSpecification {
    pub path_buf_representing_host_source: PathBuf,
    pub path_buf_representing_container_destination: PathBuf,
    pub boolean_flag_indicating_readonly: bool,
    pub boolean_flag_indicating_device: bool,
    pub boolean_flag_indicating_try_only: bool,
}

const STRING_SLICE_REPRESENTING_SANDBOX_DEFAULT_PATH_ENV: &str = "/usr/local/bin:/usr/bin:/bin";

// ==========================================
// 🚀 XDG 与宿主环境基础路径推导
// ==========================================

fn get_user_home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .expect("致命错误: HOME 环境变量未设置")
}

fn resolve_host_environment_base_paths() -> (PathBuf, PathBuf) {
    let path_buf_representing_host_home_directory = get_user_home();
    let path_buf_representing_sandbox_data_root_directory =
        path_buf_representing_host_home_directory.join(".sandbox_data");

    (
        path_buf_representing_host_home_directory,
        path_buf_representing_sandbox_data_root_directory,
    )
}

/// 100% 安全获取当前进程 UID（利用 /proc/self 元数据，无需 unsafe 与 libc）
fn resolve_current_user_id_safely() -> u32 {
    if let Ok(metadata_representing_proc_self) = fs::metadata("/proc/self") {
        return metadata_representing_proc_self.uid();
    }
    // 兜底策略：解析 XDG_RUNTIME_DIR (/run/user/1000)
    if let Ok(string_representing_xdg_runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        if let Some(string_slice_representing_uid) =
            string_representing_xdg_runtime_dir.strip_prefix("/run/user/")
        {
            if let Ok(unsigned_32_bit_integer_representing_uid) =
                string_slice_representing_uid.parse::<u32>()
            {
                return unsigned_32_bit_integer_representing_uid;
            }
        }
    }
    1000
}

// ==========================================
// 💡 命令行 UI 交互与打印函数
// ==========================================

fn print_bwrap_run_help_information_and_exit() -> ! {
    let string_representing_program_name = env::args()
        .next()
        .unwrap_or_else(|| "./bwrap-run".to_string());
    println!("用法:");
    println!("  1. 运行命令/应用 (交互式选择沙箱ID):");
    println!(
        "      {} <可执行文件/命令> [参数...]",
        string_representing_program_name
    );
    println!("      例如: {} firefox", string_representing_program_name);
    println!();
    println!("  2. 运行命令/应用 (指定沙箱ID):");
    println!(
        "      {} --id <沙箱名> <可执行文件/命令> [参数...]",
        string_representing_program_name
    );
    println!(
        "      例如: {} --id browser_work firefox",
        string_representing_program_name
    );
    println!();
    println!("  3. 进入沙箱执行 Shell (指定沙箱ID):");
    println!("      {} --id <沙箱名>", string_representing_program_name);
    println!(
        "      例如: {} --id browser_work",
        string_representing_program_name
    );
    println!();
    println!("  4. 管理和信息:");
    println!(
        "      {} --list             # 列出所有已创建的沙箱",
        string_representing_program_name
    );
    println!(
        "      {} --help             # 打印此帮助信息",
        string_representing_program_name
    );
    exit(1);
}

fn list_active_sandboxes_and_exit(
    path_buf_representing_sandbox_data_root_directory: &Path,
) -> ! {
    if !path_buf_representing_sandbox_data_root_directory.exists() {
        println!(
            "宿主沙箱根目录 '{}' 不存在。",
            path_buf_representing_sandbox_data_root_directory.display()
        );
        exit(0);
    }

    println!("📦 已创建的沙箱列表 (持久化目录):");
    let mut unsigned_integer_representing_sandbox_count = 0;
    if let Ok(read_dir_representing_entries) =
        fs::read_dir(path_buf_representing_sandbox_data_root_directory)
    {
        for dir_entry_result in read_dir_representing_entries.flatten() {
            if let Ok(file_type) = dir_entry_result.file_type() {
                if file_type.is_dir() {
                    println!("  - {}", dir_entry_result.file_name().to_string_lossy());
                    unsigned_integer_representing_sandbox_count += 1;
                }
            }
        }
    }

    if unsigned_integer_representing_sandbox_count == 0 {
        println!("  (没有已创建的沙箱)");
    }
    exit(0);
}

fn prompt_for_sandbox_identifier(
    string_slice_representing_default_identifier: &str,
) -> String {
    println!("--- 交互式沙箱 ID 确认 ---");
    print!(
        "请输入沙箱 ID (留空使用默认: {}): ",
        string_slice_representing_default_identifier
    );
    io::stdout().flush().unwrap();

    let mut string_representing_user_input_id = String::new();
    io::stdin()
        .read_line(&mut string_representing_user_input_id)
        .unwrap();
    let string_slice_representing_trimmed_id = string_representing_user_input_id.trim();

    let string_representing_final_sandbox_id = if string_slice_representing_trimmed_id.is_empty()
    {
        string_slice_representing_default_identifier.to_string()
    } else {
        string_slice_representing_trimmed_id.to_string()
    };

    println!("✅ 确认沙箱 ID: {}", string_representing_final_sandbox_id);
    println!("---------------------------");
    string_representing_final_sandbox_id
}

// ==========================================
// 🔍 物理探针与路径工具
// ==========================================

fn resolve_target_command_absolute_path(
    os_str_representing_command_name: &OsStr,
) -> Option<PathBuf> {
    let path_slice_representing_command = Path::new(os_str_representing_command_name);

    if os_str_representing_command_name
        .as_bytes()
        .contains(&b'/')
        || path_slice_representing_command.is_absolute()
    {
        if path_slice_representing_command.exists() {
            return path_slice_representing_command
                .canonicalize()
                .ok()
                .or_else(|| Some(path_slice_representing_command.to_path_buf()));
        }
    } else if let Some(os_string_representing_path_env) = env::var_os("PATH") {
        for path_buf_representing_search_dir in env::split_paths(&os_string_representing_path_env)
        {
            let path_buf_representing_candidate =
                path_buf_representing_search_dir.join(os_str_representing_command_name);
            if path_buf_representing_candidate.exists() {
                return path_buf_representing_candidate
                    .canonicalize()
                    .ok()
                    .or_else(|| Some(path_buf_representing_candidate));
            }
        }
    } else if let Ok(path_buf_representing_cwd) = env::current_dir() {
        let path_buf_representing_candidate =
            path_buf_representing_cwd.join(os_str_representing_command_name);
        if path_buf_representing_candidate.exists() {
            return path_buf_representing_candidate
                .canonicalize()
                .ok()
                .or_else(|| Some(path_buf_representing_candidate));
        }
    }
    None
}

/// 区分 文件 vs 目录，安全建立 Home 下的父级目录结构
fn prepare_host_home_command_directory_structure(
    path_slice_representing_host_command_path: &Path,
    path_slice_representing_sandbox_persistence_directory: &Path,
) {
    let path_buf_representing_host_home_directory = get_user_home();
    if path_slice_representing_host_command_path
        .starts_with(&path_buf_representing_host_home_directory)
    {
        if let Ok(path_slice_representing_relative_subpath) =
            path_slice_representing_host_command_path
                .strip_prefix(&path_buf_representing_host_home_directory)
        {
            let option_representing_target_dir_in_persistence =
                if path_slice_representing_host_command_path.exists()
                    && !path_slice_representing_host_command_path.is_dir()
                {
                    path_slice_representing_relative_subpath
                        .parent()
                        .map(|parent| path_slice_representing_sandbox_persistence_directory.join(parent))
                } else {
                    Some(
                        path_slice_representing_sandbox_persistence_directory
                            .join(path_slice_representing_relative_subpath),
                    )
                };

            if let Some(path_buf_representing_target_dir) =
                option_representing_target_dir_in_persistence
            {
                if !path_buf_representing_target_dir.exists() {
                    println!(
                        "ℹ️ 正在沙箱中创建命令的父目录结构: {}",
                        path_buf_representing_target_dir.display()
                    );
                    if let Err(error_representing_failed_mkdir) =
                        fs::create_dir_all(&path_buf_representing_target_dir)
                    {
                        eprintln!(
                            "错误: 无法创建沙箱内目录结构: {}",
                            error_representing_failed_mkdir
                        );
                        exit(1);
                    }
                }
            }
        }
    }
}

// ==========================================
// ⚙️ 统一挂载描述符收集器 (Data-Driven Pipeline)
// ==========================================

fn collect_runtime_mount_specifications(
    path_buf_representing_host_home_directory: &Path,
    path_buf_representing_sandbox_persistence_directory: &Path,
    path_buf_representing_host_command_absolute_path: &Path,
) -> Vec<MountSpecification> {
    let mut vector_of_mount_specifications: Vec<MountSpecification> = Vec::new();

    // 辅助闭包：快速添加辅助挂载描述符
    let mut helper_add_mount_spec = |host_source: PathBuf,
                                     container_dest: PathBuf,
                                     readonly: bool,
                                     device: bool,
                                     try_only: bool| {
        vector_of_mount_specifications.push(MountSpecification {
            path_buf_representing_host_source: host_source,
            path_buf_representing_container_destination: container_dest,
            boolean_flag_indicating_readonly: readonly,
            boolean_flag_indicating_device: device,
            boolean_flag_indicating_try_only: try_only,
        });
    };

    // 1. 系统核心只读目录挂载 (Mandatory Core System Paths)
    let array_of_strings_representing_system_core_paths =
        ["/usr", "/etc", "/sys", "/bin", "/sbin", "/lib", "/lib64"];
    for string_slice_representing_sys_path in array_of_strings_representing_system_core_paths {
        let path_buf_representing_sys_path = PathBuf::from(string_slice_representing_sys_path);
        if path_buf_representing_sys_path.exists() {
            helper_add_mount_spec(
                path_buf_representing_sys_path.clone(),
                path_buf_representing_sys_path,
                true,
                false,
                false,
            );
        }
    }

    // 2. GPU 显卡加速节点 (DRI & NVIDIA Nodes)
    if Path::new("/dev/dri").exists() {
        helper_add_mount_spec(
            PathBuf::from("/dev/dri"),
            PathBuf::from("/dev/dri"),
            false,
            true,
            true,
        );
    }
    for unsigned_integer_index in 0..10 {
        let path_buf_representing_nvidia_node =
            PathBuf::from(format!("/dev/nvidia{}", unsigned_integer_index));
        if path_buf_representing_nvidia_node.exists() {
            helper_add_mount_spec(
                path_buf_representing_nvidia_node.clone(),
                path_buf_representing_nvidia_node,
                false,
                true,
                true,
            );
        }
    }
    let array_of_strings_representing_nvidia_controls = [
        "/dev/nvidiactl",
        "/dev/nvidia-modeset",
        "/dev/nvidia-uvm",
        "/dev/nvidia-uvm-tools",
    ];
    for string_slice_representing_ctrl_node in array_of_strings_representing_nvidia_controls {
        let path_buf_representing_ctrl_node = PathBuf::from(string_slice_representing_ctrl_node);
        if path_buf_representing_ctrl_node.exists() {
            helper_add_mount_spec(
                path_buf_representing_ctrl_node.clone(),
                path_buf_representing_ctrl_node,
                false,
                true,
                true,
            );
        }
    }

    // 3. GUI、Wayland、X11、Audio 运行时 Socket 挂载
    let unsigned_32_bit_integer_representing_uid = resolve_current_user_id_safely();

    if let Ok(string_representing_wayland_display) = env::var("WAYLAND_DISPLAY") {
        let path_buf_representing_wayland_socket = PathBuf::from(format!(
            "/run/user/{}/{}",
            unsigned_32_bit_integer_representing_uid, string_representing_wayland_display
        ));
        if path_buf_representing_wayland_socket.exists() {
            println!("启用 Wayland 支持...");
            helper_add_mount_spec(
                path_buf_representing_wayland_socket.clone(),
                path_buf_representing_wayland_socket,
                false,
                false,
                true,
            );
        }
    }

    if env::var("DISPLAY").is_ok() && Path::new("/tmp/.X11-unix").exists() {
        println!("启用 X11 支持...");
        helper_add_mount_spec(
            PathBuf::from("/tmp/.X11-unix"),
            PathBuf::from("/tmp/.X11-unix"),
            false,
            false,
            true,
        );
    }

    if let Ok(string_representing_xdg_runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        if Path::new(&string_representing_xdg_runtime_dir).is_dir() {
            let path_buf_representing_pipewire_socket = PathBuf::from(format!(
                "/run/user/{}/pipewire-0",
                unsigned_32_bit_integer_representing_uid
            ));
            if path_buf_representing_pipewire_socket.exists() {
                println!("启用 PipeWire 音频支持...");
                helper_add_mount_spec(
                    path_buf_representing_pipewire_socket.clone(),
                    path_buf_representing_pipewire_socket,
                    false,
                    false,
                    true,
                );
            }
            let path_buf_representing_pulse_socket = PathBuf::from(format!(
                "/run/user/{}/pulse",
                unsigned_32_bit_integer_representing_uid
            ));
            if path_buf_representing_pulse_socket.exists() {
                println!("启用 PulseAudio 兼容层支持...");
                helper_add_mount_spec(
                    path_buf_representing_pulse_socket.clone(),
                    path_buf_representing_pulse_socket,
                    false,
                    false,
                    true,
                );
            }
        }
    }

    // 4. Vulkan 配置
    if Path::new("/usr/share/vulkan").is_dir() {
        println!("绑定 Vulkan 配置 (USR_SHARE 路径)...");
        helper_add_mount_spec(
            PathBuf::from("/usr/share/vulkan"),
            PathBuf::from("/usr/share/vulkan"),
            true,
            false,
            true,
        );
    } else if Path::new("/etc/vulkan").is_dir() {
        println!("绑定 Vulkan 配置 (ETC 路径)...");
        helper_add_mount_spec(
            PathBuf::from("/etc/vulkan"),
            PathBuf::from("/etc/vulkan"),
            true,
            false,
            true,
        );
    }

    // 5. 输入设备、FUSE 与 udev 硬件数据库
    println!("绑定输入设备和 FUSE...");
    if Path::new("/dev/input").exists() {
        helper_add_mount_spec(
            PathBuf::from("/dev/input"),
            PathBuf::from("/dev/input"),
            false,
            true,
            true,
        );
    }
    if Path::new("/dev/fuse").exists() {
        helper_add_mount_spec(
            PathBuf::from("/dev/fuse"),
            PathBuf::from("/dev/fuse"),
            false,
            true,
            true,
        );
    }
    let array_of_strings_representing_udev_paths = ["/run/udev", "/etc/udev"];
    for string_slice_representing_udev_path in array_of_strings_representing_udev_paths {
        let path_buf_representing_udev_path = PathBuf::from(string_slice_representing_udev_path);
        if path_buf_representing_udev_path.exists() {
            helper_add_mount_spec(
                path_buf_representing_udev_path.clone(),
                path_buf_representing_udev_path,
                true,
                false,
                true,
            );
        }
    }

    // 6. DBus、AT-SPI 与 GVFS
    let path_buf_representing_dbus_socket = PathBuf::from(format!(
        "/run/user/{}/bus",
        unsigned_32_bit_integer_representing_uid
    ));
    if path_buf_representing_dbus_socket.exists() {
        helper_add_mount_spec(
            path_buf_representing_dbus_socket.clone(),
            path_buf_representing_dbus_socket,
            false,
            false,
            true,
        );
    }

    let path_buf_representing_at_spi_dir = PathBuf::from(format!(
        "/run/user/{}/at-spi",
        unsigned_32_bit_integer_representing_uid
    ));
    if path_buf_representing_at_spi_dir.is_dir() {
        println!("启用 AT-SPI (辅助功能) 支持...");
        helper_add_mount_spec(
            path_buf_representing_at_spi_dir.clone(),
            path_buf_representing_at_spi_dir,
            false,
            false,
            true,
        );
    }

    let path_buf_representing_gvfs_dir = PathBuf::from(format!(
        "/run/user/{}/gvfs",
        unsigned_32_bit_integer_representing_uid
    ));
    if path_buf_representing_gvfs_dir.is_dir() {
        println!("启用 GVFS 支持...");
        helper_add_mount_spec(
            path_buf_representing_gvfs_dir.clone(),
            path_buf_representing_gvfs_dir,
            false,
            false,
            true,
        );
    }

    // 7. 系统与用户自定义字体
    println!("绑定系统和用户自定义字体...");
    let array_of_strings_representing_font_paths =
        ["/etc/fonts", "/usr/share/fonts", "/usr/local/share/fonts"];
    for string_slice_representing_font_path in array_of_strings_representing_font_paths {
        let path_buf_representing_font_path = PathBuf::from(string_slice_representing_font_path);
        if path_buf_representing_font_path.exists() {
            helper_add_mount_spec(
                path_buf_representing_font_path.clone(),
                path_buf_representing_font_path,
                true,
                false,
                true,
            );
        }
    }
    let path_buf_representing_user_fonts_dir =
        path_buf_representing_host_home_directory.join(".local/share/fonts");
    if path_buf_representing_user_fonts_dir.is_dir() {
        helper_add_mount_spec(
            path_buf_representing_user_fonts_dir.clone(),
            path_buf_representing_user_fonts_dir,
            true,
            false,
            true,
        );
    }

    // 8. 动态 DNS 网络解析目录挂载
    let array_of_strings_representing_dns_paths =
        ["/run/systemd/resolve", "/run/NetworkManager", "/run/resolvconf"];
    for string_slice_representing_dns_path in array_of_strings_representing_dns_paths {
        let path_buf_representing_dns_path = PathBuf::from(string_slice_representing_dns_path);
        if path_buf_representing_dns_path.exists() {
            helper_add_mount_spec(
                path_buf_representing_dns_path.clone(),
                path_buf_representing_dns_path,
                true,
                false,
                true,
            );
        }
    }

    // 9. 沙箱持久化主 HOME 目录绑定
    helper_add_mount_spec(
        path_buf_representing_sandbox_persistence_directory.to_path_buf(),
        path_buf_representing_host_home_directory.to_path_buf(),
        false,
        false,
        false,
    );
    println!(
        "✅ 策略：统一加载沙箱环境 HOME ({})。",
        path_buf_representing_host_home_directory.display()
    );

    // 10. 本次要执行的可执行文件物理 Overlay 绑定
    if path_buf_representing_host_command_absolute_path
        .starts_with(path_buf_representing_host_home_directory)
    {
        helper_add_mount_spec(
            path_buf_representing_host_command_absolute_path.to_path_buf(),
            path_buf_representing_host_command_absolute_path.to_path_buf(),
            true,
            false,
            true,
        );
        println!("✅ 策略：Home 目录命令，在环境上进行文件级绑定 (Overlay)。");
    } else {
        let bytes_slice_representing_path =
            path_buf_representing_host_command_absolute_path.as_os_str().as_bytes();
        if !bytes_slice_representing_path.starts_with(b"/usr/")
            && !bytes_slice_representing_path.starts_with(b"/bin")
            && !bytes_slice_representing_path.starts_with(b"/sbin")
            && !bytes_slice_representing_path.starts_with(b"/lib")
            && !bytes_slice_representing_path.starts_with(b"/lib64")
        {
            if let Some(path_slice_representing_command_dir) =
                path_buf_representing_host_command_absolute_path.parent()
            {
                helper_add_mount_spec(
                    path_slice_representing_command_dir.to_path_buf(),
                    path_slice_representing_command_dir.to_path_buf(),
                    true,
                    false,
                    true,
                );
                println!(
                    "⚠️ 策略：命令位于非核心系统路径 ({})。已绑定其父目录。",
                    path_slice_representing_command_dir.display()
                );
            }
        } else {
            println!("ℹ️ 策略：命令依赖于已全局绑定的系统目录 (/usr, /bin 等)。");
        }
    }

    vector_of_mount_specifications
}

// ==========================================
// 🚀 终局进程替换引擎 (Process Replacement Engine)
// ==========================================

fn execute_process_replacement_via_bubblewrap(
    string_representing_derived_sandbox_identifier: String,
    path_buf_representing_host_home_directory: PathBuf,
    path_buf_representing_sandbox_persistence_directory: PathBuf,
    path_buf_representing_host_command_absolute_path: PathBuf,
    os_string_representing_command_to_run_inside_sandbox: PathBuf,
    vector_of_os_strings_representing_target_arguments: Vec<OsString>,
) -> ! {
    let mut vector_of_os_strings_representing_bubblewrap_arguments: Vec<OsString> = Vec::new();

    // 1. 基础隔离标志 (按 bwrap-winer 规范保留 IPC 以保障 GPU/Vulkan 加速)
    vector_of_os_strings_representing_bubblewrap_arguments.extend(vec![
        OsString::from("--die-with-parent"),
        OsString::from("--proc"),
        OsString::from("/proc"),
        OsString::from("--dev"),
        OsString::from("/dev"),
        OsString::from("--tmpfs"),
        OsString::from("/tmp"),
        OsString::from("--tmpfs"),
        OsString::from("/run"),
    ]);

    // 2. 收集通用挂载规范
    let vector_of_mount_specifications = collect_runtime_mount_specifications(
        &path_buf_representing_host_home_directory,
        &path_buf_representing_sandbox_persistence_directory,
        &path_buf_representing_host_command_absolute_path,
    );

    // 3. 统一合并与数据驱动推导循环 (挂载去重核心)
    for mount_spec in vector_of_mount_specifications {
        let string_slice_representing_bwrap_flag = match (
            mount_spec.boolean_flag_indicating_device,
            mount_spec.boolean_flag_indicating_readonly,
            mount_spec.boolean_flag_indicating_try_only,
        ) {
            (true, _, true) => "--dev-bind-try",
            (true, _, false) => "--dev-bind",
            (false, true, true) => "--ro-bind-try",
            (false, true, false) => "--ro-bind",
            (false, false, true) => "--bind-try",
            (false, false, false) => "--bind",
        };

        vector_of_os_strings_representing_bubblewrap_arguments
            .push(OsString::from(string_slice_representing_bwrap_flag));
        vector_of_os_strings_representing_bubblewrap_arguments.push(
            mount_spec
                .path_buf_representing_host_source
                .into_os_string(),
        );
        vector_of_os_strings_representing_bubblewrap_arguments.push(
            mount_spec
                .path_buf_representing_container_destination
                .into_os_string(),
        );
    }

    // 4. 环境变量与工作目录
    if let Ok(string_representing_wayland_display) = env::var("WAYLAND_DISPLAY") {
        vector_of_os_strings_representing_bubblewrap_arguments.push(OsString::from("--setenv"));
        vector_of_os_strings_representing_bubblewrap_arguments
            .push(OsString::from("WAYLAND_DISPLAY"));
        vector_of_os_strings_representing_bubblewrap_arguments
            .push(OsString::from(string_representing_wayland_display));
    }

    if let Ok(string_representing_display) = env::var("DISPLAY") {
        vector_of_os_strings_representing_bubblewrap_arguments.push(OsString::from("--setenv"));
        vector_of_os_strings_representing_bubblewrap_arguments.push(OsString::from("DISPLAY"));
        vector_of_os_strings_representing_bubblewrap_arguments
            .push(OsString::from(string_representing_display));
    }

    vector_of_os_strings_representing_bubblewrap_arguments.extend(vec![
        OsString::from("--chdir"),
        path_buf_representing_host_home_directory
            .as_os_str()
            .into(),
        OsString::from("--setenv"),
        OsString::from("HOME"),
        path_buf_representing_host_home_directory
            .as_os_str()
            .into(),
        OsString::from("--setenv"),
        OsString::from("PATH"),
        OsString::from(STRING_SLICE_REPRESENTING_SANDBOX_DEFAULT_PATH_ENV),
    ]);

    // 5. 直通可执行文件与参数 (取消 /bin/bash -c 包装，防范 Shell 注入)
    vector_of_os_strings_representing_bubblewrap_arguments.push(OsString::from("--"));
    vector_of_os_strings_representing_bubblewrap_arguments
        .push(os_string_representing_command_to_run_inside_sandbox.into_os_string());
    vector_of_os_strings_representing_bubblewrap_arguments
        .extend(vector_of_os_strings_representing_target_arguments);

    // 6. 系统 Execve 接管进程
    println!(
        "正在沙箱内运行: {}...",
        string_representing_derived_sandbox_identifier
    );

    let error_indicating_exec_failure = Command::new("bwrap")
        .args(&vector_of_os_strings_representing_bubblewrap_arguments)
        .exec();

    eprintln!(
        "致命错误: 无法启动 bubblewrap (bwrap): {}",
        error_indicating_exec_failure
    );
    exit(1);
}

// ==========================================
// 🏁 程序入口 (Main Entry)
// ==========================================

fn main() {
    let vector_of_os_strings_representing_command_line_arguments: Vec<OsString> =
        env::args_os().collect();
    if vector_of_os_strings_representing_command_line_arguments.len() < 2 {
        print_bwrap_run_help_information_and_exit();
    }

    let (
        path_buf_representing_host_home_directory,
        path_buf_representing_sandbox_data_root_directory,
    ) = resolve_host_environment_base_paths();

    let string_slice_representing_first_argument =
        vector_of_os_strings_representing_command_line_arguments[1].to_string_lossy();

    // 表达式匹配，消除多余声明警告
    let (
        string_representing_derived_sandbox_identifier,
        os_string_representing_target_command,
        vector_of_os_strings_representing_target_arguments,
    ) = match string_slice_representing_first_argument.as_ref() {
        "--help" => print_bwrap_run_help_information_and_exit(),
        "--list" => {
            list_active_sandboxes_and_exit(&path_buf_representing_sandbox_data_root_directory)
        }
        "--id" => {
            if vector_of_os_strings_representing_command_line_arguments.len() < 3 {
                println!("错误：使用 --id 模式时，必须指定 <沙箱名>。");
                print_bwrap_run_help_information_and_exit();
            }
            let string_representing_id =
                vector_of_os_strings_representing_command_line_arguments[2]
                    .to_string_lossy()
                    .to_string();
            let (os_string_cmd, vector_args) =
                if vector_of_os_strings_representing_command_line_arguments.len() >= 4 {
                    (
                        vector_of_os_strings_representing_command_line_arguments[3].clone(),
                        vector_of_os_strings_representing_command_line_arguments[4..].to_vec(),
                    )
                } else {
                    (OsString::from("/bin/bash"), Vec::new())
                };
            (string_representing_id, os_string_cmd, vector_args)
        }
        _ => {
            let os_string_cmd =
                vector_of_os_strings_representing_command_line_arguments[1].clone();
            let vector_args =
                vector_of_os_strings_representing_command_line_arguments[2..].to_vec();

            let string_representing_id =
                if let Some(path_buf_resolved) = resolve_target_command_absolute_path(&os_string_cmd) {
                    let string_slice_binary_name = path_buf_resolved
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    prompt_for_sandbox_identifier(&string_slice_binary_name)
                } else {
                    println!(
                        "错误：无法找到可执行文件 '{}' 的完整路径。",
                        os_string_cmd.to_string_lossy()
                    );
                    print_bwrap_run_help_information_and_exit();
                };
            (string_representing_id, os_string_cmd, vector_args)
        }
    };

    // 探针校验宿主目标绝对路径
    let path_buf_representing_host_command_absolute_path =
        match resolve_target_command_absolute_path(&os_string_representing_target_command) {
            Some(path) => path,
            None => {
                println!(
                    "错误：无法找到目标命令 '{}' 的完整路径。",
                    os_string_representing_target_command.to_string_lossy()
                );
                exit(1);
            }
        };

    // 确立沙箱内要执行的命令形态
    let path_buf_representing_command_to_run_inside_sandbox: PathBuf =
        if path_buf_representing_host_command_absolute_path
            .starts_with(&path_buf_representing_host_home_directory)
        {
            println!(
                "ℹ️ 命令位于 Home 目录，将使用绝对路径执行: {}",
                path_buf_representing_host_command_absolute_path.display()
            );
            path_buf_representing_host_command_absolute_path.clone()
        } else {
            let bytes_slice_path = path_buf_representing_host_command_absolute_path
                .as_os_str()
                .as_bytes();
            if !bytes_slice_path.starts_with(b"/usr/")
                && !bytes_slice_path.starts_with(b"/bin")
                && !bytes_slice_path.starts_with(b"/sbin")
            {
                println!(
                    "ℹ️ 命令位于非核心系统目录，将使用绝对路径执行: {}",
                    path_buf_representing_host_command_absolute_path.display()
                );
                path_buf_representing_host_command_absolute_path.clone()
            } else {
                let os_str_base_name = path_buf_representing_host_command_absolute_path
                    .file_name()
                    .unwrap_or_default();
                println!(
                    "ℹ️ 命令位于核心系统目录，将依赖 $PATH 查找执行: {}",
                    Path::new(os_str_base_name).display()
                );
                PathBuf::from(os_str_base_name)
            }
        };

    let path_buf_representing_sandbox_persistence_directory =
        path_buf_representing_sandbox_data_root_directory
            .join(&string_representing_derived_sandbox_identifier);

    if !path_buf_representing_sandbox_persistence_directory.exists() {
        println!(
            "宿主数据目录 '{}' 不存在，正在创建...",
            path_buf_representing_sandbox_persistence_directory.display()
        );
        if let Err(error_representing_mkdir_failure) =
            fs::create_dir_all(&path_buf_representing_sandbox_persistence_directory)
        {
            eprintln!(
                "错误: 无法创建数据目录: {}",
                error_representing_mkdir_failure
            );
            exit(1);
        }
    }

    prepare_host_home_command_directory_structure(
        &path_buf_representing_host_command_absolute_path,
        &path_buf_representing_sandbox_persistence_directory,
    );

    if path_buf_representing_host_command_absolute_path == Path::new("/bin/bash")
        && vector_of_os_strings_representing_target_arguments.is_empty()
    {
        println!("🎯 目标命令: /bin/bash");
    } else {
        let string_slice_binary_name = path_buf_representing_host_command_absolute_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        println!("🎯 目标命令: {}", string_slice_binary_name);
    }

    println!(
        "📂 使用沙箱路径: {} (主机路径: {})",
        path_buf_representing_host_home_directory.display(),
        path_buf_representing_sandbox_persistence_directory.display()
    );

    // 启动终局替换引擎
    execute_process_replacement_via_bubblewrap(
        string_representing_derived_sandbox_identifier,
        path_buf_representing_host_home_directory,
        path_buf_representing_sandbox_persistence_directory,
        path_buf_representing_host_command_absolute_path,
        path_buf_representing_command_to_run_inside_sandbox,
        vector_of_os_strings_representing_target_arguments,
    );
}
