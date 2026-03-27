use std::sync::LazyLock;

use chrono::Local;
use regex::Regex;
pub const TASK_PREFIX: &str = "chen-lab-task";

pub const TASK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"chen-lab-task.(?<TaskId>[a-zA-Z_\d\-]+)@(?<StartTime>[a-zA-Z_\d\-:+]+).service")
        .unwrap()
});

pub fn gen_task(task_name: &str) -> String {
    let time = Local::now().format("%Y-%m-%d_%H:%M:%S").to_string();
    format!("{TASK_PREFIX}.{task_name}@{time}.service")
}

#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub task: String,
    pub time: String,
}

pub fn get_task_information(id: &str) -> Option<TaskInfo> {
    let caps = TASK_REGEX.captures(id)?;
    Some(TaskInfo {
        task: caps["TaskId"].to_string(),
        time: caps["StartTime"].to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Local;
    #[test]
    fn it_works() {
        let time = Local::now().format("%Y-%m-%d_%H:%M:%S").to_string();
        let task = format!("{TASK_PREFIX}.sleep@{time}.service");
        let caps = TASK_REGEX.captures(&task).unwrap();
        assert_eq!("sleep", &caps["TaskId"]);
    }
}
