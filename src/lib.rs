use chrono::{DateTime, Local};
use std::thread::sleep;
use std::time::Duration;
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    process::{Command, Output},
    sync::{Arc, Mutex},
    thread,
};

pub enum CommandStatus {
    InQueue,
    Running,
    Finished,
}

impl fmt::Display for CommandStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let res = match *self {
            CommandStatus::InQueue => "In Queue",
            CommandStatus::Running => "Running",
            CommandStatus::Finished => "Finished",
        };
        write!(f, "{res}")
    }
}

pub struct Task {
    pub command: String,
    pub start_time: DateTime<Local>,
    pub finish_time: Option<DateTime<Local>>,
    pub status: CommandStatus,
    pub output: Option<Output>,
}

#[derive(Default)]
pub struct Tasks {
    queue: Arc<Mutex<VecDeque<(usize, String)>>>,
    currently_running: Arc<Mutex<HashMap<usize, Task>>>,
    finished: Arc<Mutex<HashMap<usize, Task>>>,
}

impl Tasks {
    pub fn get_currently_running(&self) -> Arc<Mutex<HashMap<usize, Task>>> {
        self.currently_running.clone()
    }

    pub fn get_finished(&self) -> Arc<Mutex<HashMap<usize, Task>>> {
        self.finished.clone()
    }

    pub fn push_queue(&self, id: usize, cmd: String) {
        self.queue.lock().unwrap().push_back((id, cmd));
    }
}

impl Clone for Tasks {
    fn clone(&self) -> Self {
        Tasks {
            queue: Arc::clone(&self.queue),
            currently_running: Arc::clone(&self.currently_running),
            finished: Arc::clone(&self.finished),
        }
    }
}

fn worker_loop(tasks: Tasks) {
    loop {
        let tasks_clone = tasks.clone();
        if !tasks.queue.lock().unwrap().is_empty() {
            let (id, command) = tasks.queue.lock().unwrap().pop_front().unwrap();
            run_command(id, command.as_str(), tasks_clone);
        }
        sleep(Duration::from_millis(10));
    }
}

pub fn spawn_threads(num_threads: usize, tasks: Tasks) {
    for _ in 0..num_threads {
        let tasks_clone = tasks.clone();
        thread::spawn(|| worker_loop(tasks_clone));
    }
}

pub fn get_output(id: usize, tasks: Tasks) -> Option<Output> {
    tasks
        .finished
        .lock()
        .unwrap()
        .get(&id)
        .unwrap()
        .output
        .clone()
}

fn parse_command(cmd: &str) -> Command {
    let mut split = cmd.split(' ');
    let mut command = Command::new(split.next().unwrap());
    command.args(split);
    command
}

// Takes a Command object and execute it to completion
pub fn run_command(id: usize, cmd: &str, tasks: Tasks) {
    let mut command = parse_command(cmd);
    let start_time = Local::now();
    tasks.currently_running.lock().unwrap().insert(
        id,
        Task {
            command: cmd.to_string(),
            start_time,
            finish_time: None,
            status: CommandStatus::Running,
            output: None,
        },
    );

    let res = command.output();

    if let Ok(res) = res {
        tasks.finished.lock().unwrap().insert(
            id,
            Task {
                command: cmd.to_string(),
                start_time,
                finish_time: Some(Local::now()),
                status: CommandStatus::Finished,
                output: Some(res),
            },
        );
    }
    tasks.currently_running.lock().unwrap().remove(&id);
}
