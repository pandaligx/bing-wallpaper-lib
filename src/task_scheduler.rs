//! Windows 任务计划：用户登录时执行一次，并按指定分钟间隔重复执行。

use anyhow::{Context, Result};
use chrono::{Duration as ChronoDuration, Local};
use windows::core::{Interface, BSTR, VARIANT};
use windows::Win32::Foundation::VARIANT_BOOL;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::TaskScheduler::{
    IExecAction, ILogonTrigger, ITaskFolder, ITaskService, TaskScheduler, TASK_ACTION_EXEC,
    TASK_CREATE_OR_UPDATE, TASK_INSTANCES_IGNORE_NEW, TASK_LOGON_INTERACTIVE_TOKEN,
    TASK_RUNLEVEL_HIGHEST, TASK_TRIGGER_LOGON, TASK_TRIGGER_TIME,
};

const TASK_NAME: &str = "BingWallpaperLibPeriodicWallpaper";
const RPC_E_CHANGED_MODE: windows::core::HRESULT = windows::core::HRESULT(0x8001_0106u32 as i32);

struct ComApartment {
    initialized: bool,
}

impl ComApartment {
    fn init() -> Result<Self> {
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result.is_ok() {
            Ok(Self { initialized: true })
        } else if result == RPC_E_CHANGED_MODE {
            Ok(Self { initialized: false })
        } else {
            Err(windows::core::Error::from(result)).context("初始化 Windows 任务计划 COM 失败")
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

struct TaskSchedulerClient {
    root: ITaskFolder,
    service: ITaskService,
    account: String,
    _apartment: ComApartment,
}

impl TaskSchedulerClient {
    fn connect() -> Result<Self> {
        let apartment = ComApartment::init()?;
        let service: ITaskService = unsafe {
            CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)
                .context("创建 Windows 任务计划服务失败")?
        };
        let empty = VARIANT::default();
        unsafe {
            service
                .Connect(&empty, &empty, &empty, &empty)
                .context("连接 Windows 任务计划服务失败")?;
        }

        let user = unsafe { service.ConnectedUser() }
            .context("读取任务计划当前用户名失败")?
            .to_string();
        let domain = unsafe { service.ConnectedDomain() }
            .context("读取任务计划当前用户域失败")?
            .to_string();
        let account = if domain.trim().is_empty() {
            user
        } else {
            format!(r"{domain}\{user}")
        };
        let root_path = BSTR::from("\\");
        let root = unsafe { service.GetFolder(&root_path) }.context("打开任务计划根目录失败")?;

        Ok(Self {
            root,
            service,
            account,
            _apartment: apartment,
        })
    }
}

fn normalized_interval(interval_minutes: u16) -> u16 {
    interval_minutes.clamp(
        crate::settings::AppSettings::MIN_PERIODIC_INTERVAL_MINUTES,
        crate::settings::AppSettings::MAX_PERIODIC_INTERVAL_MINUTES,
    )
}

fn repetition_interval(interval_minutes: u16) -> String {
    let total = normalized_interval(interval_minutes);
    let hours = total / 60;
    let minutes = total % 60;
    match (hours, minutes) {
        (0, minutes) => format!("PT{minutes}M"),
        (hours, 0) => format!("PT{hours}H"),
        (hours, minutes) => format!("PT{hours}H{minutes}M"),
    }
}

/// 创建或更新周期壁纸任务。任务在用户登录时立即运行，并从注册时刻后的一个周期开始重复。
pub fn register(interval_minutes: u16) -> Result<()> {
    let client = TaskSchedulerClient::connect()?;
    let definition = unsafe { client.service.NewTask(0) }.context("创建任务定义失败")?;

    let registration = unsafe { definition.RegistrationInfo() }.context("读取任务注册信息失败")?;
    unsafe {
        registration
            .SetAuthor(&BSTR::from("Bing Wallpaper Library"))
            .context("设置任务作者失败")?;
        registration
            .SetDescription(&BSTR::from(
                "Change Bing wallpaper at logon and at the configured interval",
            ))
            .context("设置任务说明失败")?;
    }

    let principal = unsafe { definition.Principal() }.context("读取任务运行账户失败")?;
    unsafe {
        principal
            .SetUserId(&BSTR::from(client.account.as_str()))
            .context("设置任务运行用户失败")?;
        principal
            .SetLogonType(TASK_LOGON_INTERACTIVE_TOKEN)
            .context("设置任务登录类型失败")?;
        principal
            .SetRunLevel(TASK_RUNLEVEL_HIGHEST)
            .context("设置任务最高权限失败")?;
    }

    let triggers = unsafe { definition.Triggers() }.context("读取任务触发器失败")?;
    let logon_trigger: ILogonTrigger = unsafe { triggers.Create(TASK_TRIGGER_LOGON) }
        .context("创建用户登录触发器失败")?
        .cast()
        .context("转换用户登录触发器失败")?;
    unsafe {
        logon_trigger
            .SetId(&BSTR::from("AtUserLogon"))
            .context("设置用户登录触发器标识失败")?;
        logon_trigger
            .SetUserId(&BSTR::from(client.account.as_str()))
            .context("设置登录触发用户失败")?;
    }

    let time_trigger =
        unsafe { triggers.Create(TASK_TRIGGER_TIME) }.context("创建周期触发器失败")?;
    let next_run =
        Local::now() + ChronoDuration::minutes(i64::from(normalized_interval(interval_minutes)));
    let start_boundary = next_run.format("%Y-%m-%dT%H:%M:%S").to_string();
    unsafe {
        time_trigger
            .SetId(&BSTR::from("PeriodicWallpaper"))
            .context("设置周期触发器标识失败")?;
        time_trigger
            .SetStartBoundary(&BSTR::from(start_boundary.as_str()))
            .context("设置周期任务开始时间失败")?;
        let repetition = time_trigger.Repetition().context("读取任务重复规则失败")?;
        repetition
            .SetInterval(&BSTR::from(repetition_interval(interval_minutes)))
            .context("设置任务重复间隔失败")?;
        repetition
            .SetStopAtDurationEnd(VARIANT_BOOL::from(false))
            .context("设置任务持续重复失败")?;
    }

    let settings = unsafe { definition.Settings() }.context("读取任务设置失败")?;
    unsafe {
        settings
            .SetAllowDemandStart(VARIANT_BOOL::from(true))
            .context("设置允许手动运行任务失败")?;
        settings
            .SetMultipleInstances(TASK_INSTANCES_IGNORE_NEW)
            .context("设置任务单实例策略失败")?;
        settings
            .SetStopIfGoingOnBatteries(VARIANT_BOOL::from(false))
            .context("设置电池供电行为失败")?;
        settings
            .SetDisallowStartIfOnBatteries(VARIANT_BOOL::from(false))
            .context("设置电池供电启动行为失败")?;
        settings
            .SetStartWhenAvailable(VARIANT_BOOL::from(true))
            .context("设置错过任务后补执行失败")?;
        settings
            .SetRunOnlyIfNetworkAvailable(VARIANT_BOOL::from(false))
            .context("设置任务网络要求失败")?;
        settings
            .SetExecutionTimeLimit(&BSTR::from("PT15M"))
            .context("设置任务执行超时失败")?;
        settings
            .SetEnabled(VARIANT_BOOL::from(true))
            .context("启用任务失败")?;
        settings
            .SetHidden(VARIANT_BOOL::from(false))
            .context("设置任务可见性失败")?;
        settings
            .SetRunOnlyIfIdle(VARIANT_BOOL::from(false))
            .context("设置任务空闲条件失败")?;
        settings
            .SetWakeToRun(VARIANT_BOOL::from(false))
            .context("设置任务不唤醒电脑失败")?;
    }

    let actions = unsafe { definition.Actions() }.context("读取任务操作失败")?;
    let action: IExecAction = unsafe { actions.Create(TASK_ACTION_EXEC) }
        .context("创建任务程序操作失败")?
        .cast()
        .context("转换任务程序操作失败")?;
    let exe = std::env::current_exe().context("读取当前程序路径失败")?;
    unsafe {
        action
            .SetPath(&BSTR::from(exe.to_string_lossy().as_ref()))
            .context("设置任务程序路径失败")?;
        action
            .SetArguments(&BSTR::from("--scheduled-wallpaper"))
            .context("设置任务程序参数失败")?;
        if let Some(parent) = exe.parent() {
            action
                .SetWorkingDirectory(&BSTR::from(parent.to_string_lossy().as_ref()))
                .context("设置任务工作目录失败")?;
        }
    }

    let user = VARIANT::from(client.account.as_str());
    let empty = VARIANT::default();
    unsafe {
        client
            .root
            .RegisterTaskDefinition(
                &BSTR::from(TASK_NAME),
                &definition,
                TASK_CREATE_OR_UPDATE.0,
                &user,
                &empty,
                TASK_LOGON_INTERACTIVE_TOKEN,
                &empty,
            )
            .context("注册 Windows 周期壁纸任务失败")?;
    }
    Ok(())
}

/// 删除周期壁纸任务；任务不存在时同样视为成功。
pub fn unregister() -> Result<()> {
    let client = TaskSchedulerClient::connect()?;
    if unsafe { client.root.GetTask(&BSTR::from(TASK_NAME)) }.is_err() {
        return Ok(());
    }
    unsafe { client.root.DeleteTask(&BSTR::from(TASK_NAME), 0) }
        .context("删除 Windows 周期壁纸任务失败")
}

/// 查询当前用户的周期壁纸任务是否存在且启用。
pub fn is_enabled() -> bool {
    TaskSchedulerClient::connect()
        .and_then(|client| {
            let task = unsafe { client.root.GetTask(&BSTR::from(TASK_NAME)) }
                .context("读取 Windows 周期壁纸任务失败")?;
            let enabled = unsafe { task.Enabled() }.context("读取周期壁纸任务状态失败")?;
            Ok(bool::from(enabled))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_task_scheduler_repetition_intervals() {
        assert_eq!(repetition_interval(0), "PT1M");
        assert_eq!(repetition_interval(1), "PT1M");
        assert_eq!(repetition_interval(60), "PT1H");
        assert_eq!(repetition_interval(90), "PT1H30M");
        assert_eq!(repetition_interval(1439), "PT23H59M");
        assert_eq!(repetition_interval(u16::MAX), "PT23H59M");
    }
}
