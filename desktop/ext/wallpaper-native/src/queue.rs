use anyhow::{Result, bail, ensure};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};
use tokio::sync::watch;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    Running,
    Paused,
    Cancelled,
    Committing,
}

static ACTIVE: LazyLock<Mutex<HashMap<String, watch::Sender<State>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn begin(date: &str) -> Result<()> {
    let _: chrono::NaiveDate = date.parse()?;
    let mut jobs = ACTIVE.lock().unwrap();
    ensure!(!jobs.contains_key(date), "该日期已有下载任务");
    jobs.insert(date.to_owned(), watch::channel(State::Running).0);
    Ok(())
}

pub fn control(date: &str) -> watch::Sender<State> {
    ACTIVE
        .lock()
        .unwrap()
        .get(date)
        .cloned()
        .unwrap_or_else(|| watch::channel(State::Running).0)
}

pub fn finish(date: &str) {
    ACTIVE.lock().unwrap().remove(date);
}

pub fn stop(date: &str, cancel: bool) -> Result<bool> {
    let jobs = ACTIVE.lock().unwrap();
    let Some(tx) = jobs.get(date) else {
        return Ok(false);
    };
    ensure!(
        tx.send_if_modified(|state| {
            if *state == State::Committing {
                return false;
            }
            *state = if cancel {
                State::Cancelled
            } else {
                State::Paused
            };
            true
        }),
        "文件已进入保存阶段"
    );
    Ok(true)
}

pub fn check(state: State) -> Result<()> {
    match state {
        State::Paused => bail!("queue:paused"),
        State::Cancelled => bail!("queue:cancelled"),
        _ => Ok(()),
    }
}

pub fn commit(tx: &watch::Sender<State>) -> Result<()> {
    tx.send_if_modified(|state| {
        if *state != State::Running {
            return false;
        }
        *state = State::Committing;
        true
    });
    check(*tx.borrow())
}
