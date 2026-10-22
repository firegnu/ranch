use super::{Bundle, Paths, Result, command, hash, probe, version_of};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(crate) struct Process {
    pub parent: u32,
    pub program: PathBuf,
}

pub(crate) struct System {
    pub macos: bool,
    pub process: fn(i32) -> Result<Process>,
    pub launch: fn(&[String]) -> Result<()>,
}

impl System {
    pub fn native() -> Self {
        Self {
            macos: cfg!(target_os = "macos"),
            process,
            launch: |args| {
                let output =
                    command::run(Path::new("/bin/launchctl"), args, Duration::from_secs(30))?;
                if output.status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "launchctl kickstart 失败：{} {}",
                        output.status,
                        String::from_utf8_lossy(&output.stderr)
                    ))
                }
            },
        }
    }
}

#[cfg(target_os = "macos")]
fn process(pid: i32) -> Result<Process> {
    use std::os::unix::ffi::OsStrExt;
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as i32;
    if unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    } != size
    {
        return Err(format!(
            "无法查询 pony pid {pid} 的父进程：{}",
            std::io::Error::last_os_error()
        ));
    }
    let mut path = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let len = unsafe { libc::proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
    if len <= 0 {
        return Err(format!(
            "无法查询 pony pid {pid} 的真实程序：{}",
            std::io::Error::last_os_error()
        ));
    }
    let end = path.iter().position(|b| *b == 0).unwrap_or(path.len());
    Ok(Process {
        parent: info.pbi_ppid,
        program: PathBuf::from(std::ffi::OsStr::from_bytes(&path[..end])),
    })
}

#[cfg(not(target_os = "macos"))]
fn process(_: i32) -> Result<Process> {
    Err("pony 非 macOS 平台，跳过重启".into())
}

pub(super) struct Plan {
    program: PathBuf,
    target_hash: String,
    pub action: Option<&'static str>,
    before: Value,
    recovery_before: Option<Value>,
    service_status: String,
    current_program: Option<PathBuf>,
    pub note: String,
    verify_running: bool,
    logs: PathBuf,
}

fn status(program: &Path, timeout: Duration) -> Result<Value> {
    let output = command::run(program, &["status".into()], timeout)?;
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("pony status 输出不是 JSON：{e}"))?;
    if !output.status.success()
        || value["ok"] != true
        || !value["running"].is_boolean()
        || (value["running"] == true && pid(&value).is_none())
    {
        return Err(format!("pony status 检查失败：{value}"));
    }
    // Deployment records need process/connection state, not channel or workspace data.
    let chats: Vec<_> = value["chats"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|chat| json!({"kind":chat["kind"],"connection":chat["connection"]}))
        .collect();
    Ok(
        json!({"service":value["service"],"running":value["running"],"pid":value["pid"],"last_exit":value["last_exit"],"chats":chats}),
    )
}

fn pid(value: &Value) -> Option<i32> {
    value["pid"]
        .as_i64()
        .and_then(|pid| i32::try_from(pid).ok())
        .filter(|pid| *pid > 0)
}

pub(super) fn plan(
    bundle: &Bundle,
    home: &Path,
    system: &System,
    unfinished: Option<&Value>,
) -> Result<Option<Plan>> {
    let Some(target_hash) = bundle.hashes.get("pony") else {
        return Ok(None);
    };
    let mut plan = Plan {
        program: bundle.dir.join("bin/pony"),
        target_hash: target_hash.clone(),
        action: None,
        before: Value::Null,
        recovery_before: None,
        service_status: String::new(),
        current_program: None,
        note: String::new(),
        verify_running: false,
        logs: home.join(".pony/logs"),
    };
    if !system.macos {
        plan.note = "pony 非 macOS 平台，跳过重启".into();
        return Ok(Some(plan));
    }
    plan.before = status(&plan.program, Duration::from_secs(5))?;
    if plan.before["service"] == "unsupported" {
        plan.note = "pony 服务 unsupported，跳过重启".into();
        return Ok(Some(plan));
    }
    // Pre-service pony versions always reported not_installed. Check presence
    // independently so rolling back cannot leave launchd running unsupported code.
    let plist = home.join("Library/LaunchAgents/local.ranch.pony.plist");
    let installed = match fs::symlink_metadata(&plist) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(super::error(&plist, e)),
    } || plan.before["service"] == "installed";
    if !installed {
        plan.note = "pony 服务没装，服务另装，deploy 不装".into();
        return Ok(Some(plan));
    }
    let service = probe(
        &plan.program,
        &["service".into(), "install".into(), "--dry-run".into()],
    )?
    .ok_or("目标 pony 不支持 service；先用当前版本卸服务再部署")?;
    if !matches!(service["status"].as_str(), Some("same" | "replace")) {
        return Err(format!("不认识的 pony 服务定义状态：{service}"));
    }
    plan.service_status = service["status"].as_str().unwrap().into();
    let skipped_definition = if plan.service_status == "replace" {
        format!(
            "；服务定义要换（replace），deploy 没换；要换时跑 {} service install --yes（会启动 pony）",
            super::quote(&plan.program)
        )
    } else {
        String::new()
    };
    plan.recovery_before = unfinished
        .filter(|record| {
            record["pony"]["before"]["running"] == true
                && record["steps"].as_array().is_some_and(|steps| {
                    steps
                        .iter()
                        .any(|step| step["step"] == "pony_service" && step["result"] == "failed")
                })
        })
        .map(|record| record["pony"]["before"].clone());
    if plan.before["running"] == false {
        if plan.recovery_before.is_some() {
            plan.action = Some("pony_service");
            plan.verify_running = true;
            plan.note = "pony 原本在跑，上次部署的服务安装失败；本次恢复服务并核对启动".into();
            return Ok(Some(plan));
        }
        plan.note = format!(
            "pony 服务装着但没在跑，不重启；last_exit {}；launchd 下次启动会用新程序",
            plan.before["last_exit"]
        );
        plan.note.push_str(&skipped_definition);
        return Ok(Some(plan));
    }
    let current = (system.process)(pid(&plan.before).ok_or("pony 没有运行 pid")?)?;
    plan.current_program = Some(current.program.clone());
    if current.parent != 1 {
        plan.note = "pony 在跑，但父进程不是 launchd，不重启，请自行重启".into();
        if plan.service_status == "replace" {
            plan.note.push_str("；先停掉手动起的 pony");
        }
        plan.note.push_str(&skipped_definition);
        return Ok(Some(plan));
    }
    plan.verify_running = true;
    if service["status"] == "replace" || plan.recovery_before.is_some() {
        plan.action = Some("pony_service");
    } else if hash(&current.program)? != plan.target_hash {
        plan.action = Some("pony_restart");
    }
    plan.note = if service["status"] == "same" {
        "pony 服务定义 plist 没变（same）"
    } else {
        "pony 服务定义需替换（replace），安装后不另 kickstart"
    }
    .into();
    if plan.action.is_none() {
        plan.note.push_str("；正在跑的 pony 程序相同，不重启");
    }
    Ok(Some(plan))
}

impl Plan {
    pub fn title(&self) -> &'static str {
        if self.recovery_before.is_some() {
            "恢复 pony 服务并启动"
        } else if self.action == Some("pony_service") {
            "更换 pony 服务定义并重启"
        } else {
            "重启 pony"
        }
    }

    pub fn description(&self, paths: &Paths) -> String {
        let mut description = if self.action == Some("pony_service") {
            format!(
                "{}：{} service install --yes",
                self.title(),
                super::quote(&self.program)
            )
        } else {
            format!(
                "重启 pony：launchctl kickstart -k gui/{}/local.ranch.pony",
                unsafe { libc::geteuid() },
            )
        };
        if let Some(program) = &self.current_program {
            let current =
                version_of(paths, program, "pony").unwrap_or_else(|| program.display().to_string());
            let target = version_of(paths, &self.program, "pony")
                .unwrap_or_else(|| self.program.display().to_string());
            description.push_str(&format!(
                "\n       正在跑的是 {current} 的 pony（pid {}），换成 {target}",
                self.before["pid"]
            ));
        }
        description
    }

    pub fn record(&self) -> Value {
        json!({"action":self.action.unwrap_or("skip"),"reason":self.note,"before":self.original_before()})
    }

    fn original_before(&self) -> &Value {
        self.recovery_before.as_ref().unwrap_or(&self.before)
    }

    pub fn service_recovery(&self) -> String {
        format!(
            "服务定义可能已卸下，launchd 不会自己再起 pony；找到原因后先跑 {} service install --yes",
            super::quote(&self.program)
        )
    }

    fn is_target(&self, value: &Value, system: &System) -> Result<bool> {
        if value["running"] != true {
            return Ok(false);
        }
        let process = (system.process)(pid(value).ok_or("pony 没有运行 pid")?)?;
        Ok(process.parent == 1 && hash(&process.program)? == self.target_hash)
    }

    pub fn execute(&self, system: &System, attempted: &mut bool) -> Result<Value> {
        let before = status(&self.program, Duration::from_secs(5))?;
        if before["pid"] != self.before["pid"] || before["running"] != self.before["running"] {
            return Err("pony 运行状态在计划后变了，请重跑".into());
        }
        if before["running"] == true
            && (system.process)(pid(&before).ok_or("pony 没有运行 pid")?)?.parent != 1
        {
            return Err("pony 父进程不是 launchd，不重启，请自行重启".into());
        }
        let service = probe(
            &self.program,
            &["service".into(), "install".into(), "--dry-run".into()],
        )?
        .ok_or("pony 不再支持 service，请重跑")?;
        if service["status"] != self.service_status {
            return Err("pony 服务定义在计划后变了，请重跑".into());
        }
        *attempted = true;
        if self.action == Some("pony_service") {
            // pony service may itself spend 30 seconds waiting for bootout.
            let output = command::run(
                &self.program,
                &["service".into(), "install".into(), "--yes".into()],
                Duration::from_secs(45),
            )?;
            let value: Value = serde_json::from_slice(&output.stdout)
                .map_err(|e| format!("pony service 输出不是 JSON：{e}"))?;
            if !output.status.success() || value["ok"] != true || value["applied"] != true {
                return Err(format!(
                    "pony service install 失败：{value} {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        } else {
            (system.launch)(&[
                "kickstart".into(),
                "-k".into(),
                format!("gui/{}/local.ranch.pony", unsafe { libc::geteuid() }),
            ])?;
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut last = String::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("30 秒内没等到新的 pony 起来：{last}"));
            }
            match status(&self.program, remaining.min(Duration::from_secs(5))) {
                Ok(value) => {
                    if value["pid"] != self.original_before()["pid"]
                        && self.is_target(&value, system).unwrap_or(false)
                    {
                        println!(
                            "pony 已起来（pid {}，{}）",
                            value["pid"],
                            connections(&value)
                        );
                        return Ok(value);
                    }
                    last = format!(
                        "running {}，pid {}，last_exit {}",
                        value["running"], value["pid"], value["last_exit"]
                    );
                }
                Err(e) => last = e,
            }
            std::thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(500)),
            );
        }
    }

    pub fn verify(&self, system: &System) -> Result<()> {
        if self.verify_running {
            let value = status(&self.program, Duration::from_secs(5))?;
            if !self.is_target(&value, system)?
                || (self.action.is_some() && value["pid"] == self.original_before()["pid"])
            {
                return Err("pony 核对失败：没有运行目标程序".into());
            }
            println!(
                "核对：pony 跑的是目标程序（pid {}，{}）。",
                value["pid"],
                connections(&value)
            );
        } else {
            println!("核对：{}。", self.note);
        }
        Ok(())
    }

    pub fn failure_status(&self, system: &System, service_failed: bool) -> Value {
        if !system.macos {
            return json!({"service":"unsupported"});
        }
        let value = match status(&self.program, Duration::from_secs(5)) {
            Ok(value) => {
                if value["running"] == false {
                    println!("pony 现在没在跑，last_exit {}。", value["last_exit"]);
                } else {
                    println!(
                        "pony 现在在跑（pid {}，{}），尚未确认部署成功。",
                        value["pid"],
                        connections(&value)
                    );
                }
                value
            }
            Err(error) => {
                println!("pony 现在的运行状态无法确认：{error}");
                json!({"error":error})
            }
        };
        if service_failed {
            println!("{}", self.service_recovery());
        }
        println!(
            "日志：{}、{}",
            self.logs.join("launchd.log").display(),
            self.logs.join("pony.log").display()
        );
        value
    }
}

fn connections(value: &Value) -> String {
    value["chats"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|chat| {
            format!(
                "{} {}",
                chat["kind"].as_str().unwrap_or("chat"),
                chat["connection"].as_str().unwrap_or("unknown")
            )
        })
        .collect::<Vec<_>>()
        .join("、")
}
